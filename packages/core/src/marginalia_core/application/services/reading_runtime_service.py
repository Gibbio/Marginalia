"""Single supported runtime loop: read while listening for commands."""

from __future__ import annotations

import os
from pathlib import Path
from typing import Any

from marginalia_core.application.command_router import CommandLexicon
from marginalia_core.application.result import OperationResult, OperationStatus
from marginalia_core.domain.reading_session import PlaybackState, ReaderState, ReadingSession
from marginalia_core.ports.runtime import RuntimeSessionRecord, RuntimeSupervisor
from marginalia_core.ports.storage import DocumentRepository, SessionRepository
from marginalia_core.ports.stt import CommandRecognition, CommandRecognizer

from .document_ingestion_service import DocumentIngestionService
from .reader_service import ReaderService


class ReadingRuntimeService:
    """Run the only supported Alpha 0.1 interaction mode."""

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

    def play(self, target: str | None) -> OperationResult:
        """Resolve a file or document id, then run the continuous read+listen loop."""

        cleanup_details = self._cleanup_existing_runtime()
        resolved_target = self._resolve_document_target(target)
        if resolved_target.status is OperationStatus.ERROR:
            return resolved_target

        document_id = str(resolved_target.data["document_id"])
        startup_result = self._reader_service.play(document_id, command_source="runtime")
        if startup_result.status is OperationStatus.ERROR:
            self._apply_startup_cleanup_summary(cleanup_details)
            return startup_result

        session = self._session_repository.get_active_session()
        if session is None:
            return OperationResult.error("Reader runtime could not locate the active session.")

        self._mark_runtime_session(
            session,
            runtime_status="active",
            command_listening_active=True,
            startup_cleanup_summary=cleanup_details["summary"],
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

        handled_commands: list[dict[str, Any]] = []
        timeout_count = 0
        try:
            with self._command_recognizer.open_interrupt_monitor() as monitor:
                while True:
                    sync_result = self._reader_service.synchronize_active_session()
                    if sync_result.status is OperationStatus.ERROR:
                        return self._fail_runtime(
                            sync_result.message,
                            cleanup_details=cleanup_details,
                            handled_commands=handled_commands,
                            timeout_count=timeout_count,
                        )

                    active_session = self._session_repository.get_active_session()
                    if active_session is None:
                        return self._fail_runtime(
                            "The active session disappeared while the runtime loop was running.",
                            cleanup_details=cleanup_details,
                            handled_commands=handled_commands,
                            timeout_count=timeout_count,
                        )

                    playback = sync_result.data["playback"]
                    if active_session.state is ReaderState.IDLE:
                        return self._finalize_runtime(
                            outcome=active_session.runtime_status or "stopped",
                            cleanup_details=cleanup_details,
                            handled_commands=handled_commands,
                            timeout_count=timeout_count,
                            resolved_target=resolved_target.data,
                        )

                    if (
                        active_session.state is ReaderState.READING
                        and playback.state is PlaybackState.STOPPED
                    ):
                        advance_result = self._reader_service.advance_after_playback_completion()
                        if advance_result.status is OperationStatus.ERROR:
                            return self._fail_runtime(
                                advance_result.message,
                                cleanup_details=cleanup_details,
                                handled_commands=handled_commands,
                                timeout_count=timeout_count,
                            )
                        if advance_result.data["completed_document"]:
                            return self._finalize_runtime(
                                outcome="completed",
                                cleanup_details=cleanup_details,
                                handled_commands=handled_commands,
                                timeout_count=timeout_count,
                                resolved_target=resolved_target.data,
                            )
                        continue

                    capture = monitor.capture_next_interrupt()
                    if capture.timed_out and not capture.recognized_command:
                        timeout_count += 1
                        continue
                    if not capture.recognized_command:
                        continue

                    dispatch_result = self._reader_service.dispatch_recognized_command(
                        CommandRecognition(
                            command=capture.recognized_command,
                            provider_name=capture.provider_name,
                            raw_text=capture.raw_text or capture.recognized_command,
                        )
                    )
                    handled_commands.append(
                        {
                            "recognized_command": capture.recognized_command,
                            "status": dispatch_result.status.value,
                            "message": dispatch_result.message,
                            "handled_command": dispatch_result.data.get("handled_command")
                            if dispatch_result.data
                            else None,
                        }
                    )
                    if dispatch_result.status is OperationStatus.ERROR:
                        continue

                    active_session = self._session_repository.get_active_session()
                    if active_session is not None and active_session.state is ReaderState.IDLE:
                        return self._finalize_runtime(
                            outcome=active_session.runtime_status or "stopped",
                            cleanup_details=cleanup_details,
                            handled_commands=handled_commands,
                            timeout_count=timeout_count,
                            resolved_target=resolved_target.data,
                        )
        except Exception as exc:
            return self._fail_runtime(
                str(exc),
                cleanup_details=cleanup_details,
                handled_commands=handled_commands,
                timeout_count=timeout_count,
            )
        finally:
            self._runtime_supervisor.clear(process_id=os.getpid())

    def stop(self) -> OperationResult:
        """Stop playback plus the registered runtime process, if any."""

        cleanup = self._cleanup_existing_runtime()
        stop_result = self._reader_service.stop(command_source="cli")
        if stop_result.status is OperationStatus.ERROR and not cleanup["cleaned_up"]:
            return OperationResult.ok(
                "No active reading runtime was running.",
                data={
                    "cleanup": cleanup,
                    "stop_result": stop_result.to_dict(),
                    "session": self._session_repository.get_active_session(),
                },
            )

        session = self._session_repository.get_active_session()
        if session is not None:
            self._mark_runtime_session(
                session,
                runtime_status="stopped",
                command_listening_active=False,
                startup_cleanup_summary=session.startup_cleanup_summary,
                runtime_error=None,
                runtime_process_id=None,
            )

        return OperationResult.ok(
            "Active reading runtime stopped.",
            data={
                "cleanup": cleanup,
                "stop_result": stop_result.to_dict(),
                "session": session,
            },
        )

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

    def _finalize_runtime(
        self,
        *,
        outcome: str,
        cleanup_details: dict[str, Any],
        handled_commands: list[dict[str, Any]],
        timeout_count: int,
        resolved_target: dict[str, Any],
    ) -> OperationResult:
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
                    "handled_commands": handled_commands,
                    "handled_command_count": len(handled_commands),
                    "timeout_count": timeout_count,
                    "cleanup": cleanup_details,
                },
                "target": resolved_target,
            },
        )

    def _fail_runtime(
        self,
        error_message: str,
        *,
        cleanup_details: dict[str, Any],
        handled_commands: list[dict[str, Any]],
        timeout_count: int,
    ) -> OperationResult:
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
                    "handled_commands": handled_commands,
                    "handled_command_count": len(handled_commands),
                    "timeout_count": timeout_count,
                    "cleanup": cleanup_details,
                },
            },
        )

    def _mark_runtime_session(
        self,
        session: ReadingSession,
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
