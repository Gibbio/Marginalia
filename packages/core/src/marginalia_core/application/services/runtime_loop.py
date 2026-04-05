"""Step-driven runtime loop for the read-while-listening mode.

The loop can be driven by a CLI ``while`` loop, a GUI timer, or an async wrapper.
Each call to ``step()`` performs one non-blocking iteration and returns a status
that tells the caller whether to continue, or whether the loop completed/stopped/failed.
"""

from __future__ import annotations

import logging
import os
from enum import Enum
from pathlib import Path
from typing import Any

from marginalia_core.application.command_router import CommandLexicon
from marginalia_core.application.result import OperationResult, OperationStatus
from marginalia_core.domain.reading_session import PlaybackState, ReaderState
from marginalia_core.ports.runtime import RuntimeSessionRecord, RuntimeSupervisor
from marginalia_core.ports.storage import DocumentRepository, SessionRepository
from marginalia_core.ports.stt import CommandRecognition, CommandRecognizer, SpeechInterruptMonitor

from .document_ingestion_service import DocumentIngestionService
from .reader_service import ReaderService

logger = logging.getLogger(__name__)


class StepStatus(str, Enum):
    """Outcome of a single ``RuntimeLoop.step()`` call."""

    CONTINUE = "continue"
    COMPLETED = "completed"
    STOPPED = "stopped"
    ERROR = "error"


class RuntimeLoop:
    """Encapsulates the read-while-listening runtime as a step function.

    Lifecycle::

        loop = RuntimeLoop(...)
        result = loop.start(target)
        if result is error:
            return result
        with loop:
            while loop.step().status is StepStatus.CONTINUE:
                pass
        return loop.finalize()
    """

    def __init__(
        self,
        *,
        document_repository: DocumentRepository,
        session_repository: SessionRepository,
        ingestion_service: DocumentIngestionService,
        reader_service: ReaderService,
        command_recognizer: CommandRecognizer,
        runtime_supervisor: RuntimeSupervisor,
        command_lexicon: CommandLexicon,
    ) -> None:
        self._document_repository = document_repository
        self._session_repository = session_repository
        self._ingestion_service = ingestion_service
        self._reader_service = reader_service
        self._command_recognizer = command_recognizer
        self._runtime_supervisor = runtime_supervisor
        self._command_lexicon = command_lexicon

        self._monitor: SpeechInterruptMonitor | None = None
        self._cleanup_details: dict[str, Any] = {}
        self._resolved_target: dict[str, Any] = {}
        self._handled_commands: list[dict[str, Any]] = []
        self._timeout_count: int = 0
        self._shutdown_requested: bool = False
        self._started: bool = False
        self._last_step_status: StepStatus = StepStatus.CONTINUE
        self._final_result: OperationResult | None = None

    @property
    def shutdown_requested(self) -> bool:
        return self._shutdown_requested

    def request_shutdown(self) -> None:
        """Signal the loop to stop at the next ``step()`` call."""

        self._shutdown_requested = True
        logger.info("Shutdown requested for runtime loop")

    def start(self, target: str | None) -> OperationResult:
        """Initialize the runtime: resolve document, start playback, register supervisor."""

        self._cleanup_details = self._cleanup_existing_runtime()
        resolved_target = self._resolve_document_target(target)
        if resolved_target.status is OperationStatus.ERROR:
            return resolved_target

        document_id = str(resolved_target.data["document_id"])
        startup_result = self._reader_service.play(document_id, command_source="runtime")
        if startup_result.status is OperationStatus.ERROR:
            self._apply_startup_cleanup_summary(self._cleanup_details)
            return startup_result

        session = self._session_repository.get_active_session()
        if session is None:
            return OperationResult.error("Reader runtime could not locate the active session.")

        self._mark_runtime_session(
            session,
            runtime_status="active",
            command_listening_active=True,
            startup_cleanup_summary=self._cleanup_details["summary"],
            runtime_error=None,
            runtime_process_id=os.getpid(),
        )
        self._runtime_supervisor.activate(
            RuntimeSessionRecord(
                process_id=os.getpid(),
                session_id=session.session_id,
                document_id=session.document_id,
                command_language=self._command_lexicon.language,
                working_directory=Path.cwd(),
            )
        )
        self._resolved_target = resolved_target.data
        self._started = True
        logger.info(
            "Runtime started for document %s (session %s)",
            session.document_id,
            session.session_id,
        )
        return OperationResult.ok(
            "Runtime loop initialized.",
            data={"session": session, "target": resolved_target.data},
        )

    def step(self) -> StepStatus:
        """Execute one iteration of the runtime loop.

        Returns a ``StepStatus`` indicating whether the caller should continue.
        """

        if self._last_step_status is not StepStatus.CONTINUE:
            return self._last_step_status

        if self._shutdown_requested:
            self._reader_service.stop(command_source="signal")
            self._last_step_status = StepStatus.STOPPED
            self._final_result = self._finalize_runtime(outcome="stopped")
            logger.info("Runtime stopped by shutdown request")
            return self._last_step_status

        try:
            status = self._step_inner()
            self._last_step_status = status
            return status
        except Exception as exc:
            logger.exception("Runtime loop step failed")
            self._last_step_status = StepStatus.ERROR
            self._final_result = self._fail_runtime(str(exc))
            return StepStatus.ERROR

    def _step_inner(self) -> StepStatus:
        sync_result = self._reader_service.synchronize_active_session()
        if sync_result.status is OperationStatus.ERROR:
            self._final_result = self._fail_runtime(sync_result.message)
            return StepStatus.ERROR

        active_session = self._session_repository.get_active_session()
        if active_session is None:
            self._final_result = self._fail_runtime(
                "The active session disappeared while the runtime loop was running."
            )
            return StepStatus.ERROR

        playback = sync_result.data["playback"]

        if active_session.state is ReaderState.IDLE:
            self._final_result = self._finalize_runtime(
                outcome=active_session.runtime_status or "stopped"
            )
            return StepStatus.STOPPED

        if (
            active_session.state is ReaderState.READING
            and playback.state is PlaybackState.STOPPED
        ):
            advance_result = self._reader_service.advance_after_playback_completion()
            if advance_result.status is OperationStatus.ERROR:
                self._final_result = self._fail_runtime(advance_result.message)
                return StepStatus.ERROR
            if advance_result.data["completed_document"]:
                self._final_result = self._finalize_runtime(outcome="completed")
                logger.info("Document playback completed")
                return StepStatus.COMPLETED
            return StepStatus.CONTINUE

        if self._monitor is None:
            return StepStatus.CONTINUE

        capture = self._monitor.capture_next_interrupt()
        if capture.timed_out and not capture.recognized_command:
            self._timeout_count += 1
            return StepStatus.CONTINUE
        if not capture.recognized_command:
            return StepStatus.CONTINUE

        dispatch_result = self._reader_service.dispatch_recognized_command(
            CommandRecognition(
                command=capture.recognized_command,
                provider_name=capture.provider_name,
                raw_text=capture.raw_text or capture.recognized_command,
            )
        )
        self._handled_commands.append(
            {
                "recognized_command": capture.recognized_command,
                "status": dispatch_result.status.value,
                "message": dispatch_result.message,
                "handled_command": dispatch_result.data.get("handled_command")
                if dispatch_result.data
                else None,
            }
        )
        logger.info(
            "Voice command: %s -> %s",
            capture.recognized_command,
            dispatch_result.data.get("handled_command") if dispatch_result.data else "unknown",
        )
        if dispatch_result.status is OperationStatus.ERROR:
            return StepStatus.CONTINUE

        active_session = self._session_repository.get_active_session()
        if active_session is not None and active_session.state is ReaderState.IDLE:
            self._final_result = self._finalize_runtime(
                outcome=active_session.runtime_status or "stopped"
            )
            return StepStatus.STOPPED

        return StepStatus.CONTINUE

    def finalize(self) -> OperationResult:
        """Return the final result after the loop exits."""

        if self._final_result is not None:
            return self._final_result
        return self._finalize_runtime(outcome="stopped")

    def __enter__(self) -> RuntimeLoop:
        self._monitor = self._command_recognizer.open_interrupt_monitor()
        self._monitor.__enter__()
        return self

    def __exit__(self, exc_type: object, exc: object, tb: object) -> bool | None:
        try:
            if self._monitor is not None:
                self._monitor.__exit__(exc_type, exc, tb)
                self._monitor = None
        except Exception:
            logger.debug("Error closing interrupt monitor", exc_info=True)
        if self._last_step_status is StepStatus.CONTINUE:
            try:
                self._reader_service.stop(command_source="cleanup")
            except Exception:
                logger.debug("Error stopping reader during cleanup", exc_info=True)
        self._runtime_supervisor.clear(process_id=os.getpid())
        return None

    # ------------------------------------------------------------------
    # Internal helpers (carried over from the old ReadingRuntimeService)
    # ------------------------------------------------------------------

    def _resolve_document_target(self, target: str | None) -> OperationResult:
        if target:
            candidate_path = Path(target).expanduser()
            if candidate_path.exists() and candidate_path.is_file():
                ingest_result = self._ingestion_service.ingest_text_file(candidate_path)
                if ingest_result.status is OperationStatus.ERROR:
                    return ingest_result
                document = ingest_result.data["document"]
                return OperationResult.ok(
                    "Input file ingested for reading.",
                    data={
                        "document_id": document.document_id,
                        "source_path": document.source_path,
                        "ingested_now": True,
                    },
                )
            document = self._document_repository.get_document(target)
            if document is not None:
                return OperationResult.ok(
                    "Stored document selected for reading.",
                    data={
                        "document_id": document.document_id,
                        "source_path": document.source_path,
                        "ingested_now": False,
                    },
                )
            return OperationResult.error(
                f"Input '{target}' is neither a readable file nor a stored document id."
            )

        active_session = self._session_repository.get_active_session()
        if active_session is not None:
            return OperationResult.ok(
                "Resuming the active session document.",
                data={
                    "document_id": active_session.document_id,
                    "source_path": None,
                    "ingested_now": False,
                },
            )

        documents = self._document_repository.list_documents()
        if documents:
            return OperationResult.ok(
                "Using the latest ingested document.",
                data={
                    "document_id": documents[0].document_id,
                    "source_path": documents[0].source_path,
                    "ingested_now": False,
                },
            )
        return OperationResult.error("No document is available to read.")

    def _cleanup_existing_runtime(self) -> dict[str, Any]:
        cleanup_notes: list[str] = []
        stop_result = self._reader_service.stop(command_source="runtime-cleanup")
        if stop_result.status is OperationStatus.OK:
            cleanup_notes.append("Stopped the previously persisted reading session.")

        cleanup_report = self._runtime_supervisor.cleanup_existing_runtime(
            current_process_id=os.getpid()
        )
        cleanup_notes.extend(cleanup_report.notes)
        summary = "; ".join(cleanup_notes) if cleanup_notes else "No stale runtime was found."
        return {
            "cleaned_up": stop_result.status is OperationStatus.OK or cleanup_report.cleaned_up,
            "stop_result": stop_result.to_dict(),
            "runtime_report": cleanup_report,
            "summary": summary,
        }

    def _apply_startup_cleanup_summary(self, cleanup_details: dict[str, Any]) -> None:
        session = self._session_repository.get_active_session()
        if session is None:
            return
        session.startup_cleanup_summary = str(cleanup_details["summary"])
        session.touch()
        self._session_repository.save_session(session)

    def _finalize_runtime(self, *, outcome: str) -> OperationResult:
        session = self._session_repository.get_active_session()
        if session is not None:
            self._mark_runtime_session(
                session,
                runtime_status=outcome,
                command_listening_active=False,
                startup_cleanup_summary=session.startup_cleanup_summary,
                runtime_error=None,
                runtime_process_id=None,
            )
        return OperationResult.ok(
            (
                "Reading session completed."
                if outcome == "completed"
                else "Reading session stopped."
            ),
            data={
                "session": session,
                "runtime": {
                    "outcome": outcome,
                    "command_language": self._command_lexicon.language,
                    "handled_commands": self._handled_commands,
                    "handled_command_count": len(self._handled_commands),
                    "timeout_count": self._timeout_count,
                    "cleanup": self._cleanup_details,
                },
                "target": self._resolved_target,
            },
        )

    def _fail_runtime(self, error_message: str) -> OperationResult:
        session = self._session_repository.get_active_session()
        if session is not None:
            self._mark_runtime_session(
                session,
                runtime_status="failed",
                command_listening_active=False,
                startup_cleanup_summary=session.startup_cleanup_summary,
                runtime_error=error_message,
                runtime_process_id=None,
            )
        return OperationResult.error(
            error_message,
            data={
                "session": session,
                "runtime": {
                    "outcome": "failed",
                    "command_language": self._command_lexicon.language,
                    "handled_commands": self._handled_commands,
                    "handled_command_count": len(self._handled_commands),
                    "timeout_count": self._timeout_count,
                    "cleanup": self._cleanup_details,
                },
            },
        )

    def _mark_runtime_session(
        self,
        session: Any,
        *,
        runtime_status: str,
        command_listening_active: bool,
        startup_cleanup_summary: str | None,
        runtime_error: str | None,
        runtime_process_id: int | None,
    ) -> None:
        session.command_listening_active = command_listening_active
        session.command_language = self._command_lexicon.language
        session.runtime_status = runtime_status
        session.runtime_error = runtime_error
        session.runtime_process_id = runtime_process_id
        session.startup_cleanup_summary = startup_cleanup_summary
        if runtime_status in {"completed", "stopped"}:
            session.state = ReaderState.IDLE
            session.playback_state = PlaybackState.STOPPED
        session.touch()
        self._session_repository.save_session(session)
