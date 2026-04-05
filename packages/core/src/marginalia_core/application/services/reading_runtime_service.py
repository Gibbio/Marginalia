"""Single supported runtime loop: read while listening for commands."""

from __future__ import annotations

import logging
import os
from typing import Any

from marginalia_core.application.command_router import CommandLexicon
from marginalia_core.application.result import OperationResult, OperationStatus
from marginalia_core.ports.runtime import RuntimeSupervisor
from marginalia_core.ports.storage import DocumentRepository, SessionRepository
from marginalia_core.ports.stt import CommandRecognizer

from .document_ingestion_service import DocumentIngestionService
from .reader_service import ReaderService
from .runtime_loop import RuntimeLoop, StepStatus

logger = logging.getLogger(__name__)


class ReadingRuntimeService:
    """Run the only supported Alpha 0.2 interaction mode."""

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

    def create_loop(self) -> RuntimeLoop:
        """Build a fresh ``RuntimeLoop`` for external callers (e.g. desktop)."""

        return RuntimeLoop(
            document_repository=self._document_repository,
            session_repository=self._session_repository,
            ingestion_service=self._ingestion_service,
            reader_service=self._reader_service,
            command_recognizer=self._command_recognizer,
            runtime_supervisor=self._runtime_supervisor,
            command_lexicon=self._command_lexicon,
        )

    def play(self, target: str | None) -> OperationResult:
        """Resolve a file or document id, then run the continuous read+listen loop."""

        loop = self.create_loop()
        start_result = loop.start(target)
        if start_result.status is OperationStatus.ERROR:
            return start_result

        with loop:
            while loop.step() is StepStatus.CONTINUE:
                pass

        return loop.finalize()

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
            session.command_listening_active = False
            session.command_language = self._command_lexicon.language
            session.runtime_status = "stopped"
            session.runtime_error = None
            session.runtime_process_id = None
            session.startup_cleanup_summary = session.startup_cleanup_summary
            session.touch()
            self._session_repository.save_session(session)

        return OperationResult.ok(
            "Active reading runtime stopped.",
            data={
                "cleanup": cleanup,
                "stop_result": stop_result.to_dict(),
                "session": session,
            },
        )

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
