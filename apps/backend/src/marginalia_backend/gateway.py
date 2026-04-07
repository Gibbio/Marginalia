"""Frontend gateway implementation backed by local services."""

from __future__ import annotations

from dataclasses import asdict
from pathlib import Path

from marginalia_backend.bootstrap import BackendContainer
from marginalia_backend.runtime import BackendRuntimeManager
from marginalia_backend.serialization import to_transport_dict
from marginalia_core.application.frontend.capabilities import BackendCapabilities
from marginalia_core.application.frontend.commands import FrontendCommandName
from marginalia_core.application.frontend.envelopes import (
    FRONTEND_PROTOCOL_VERSION,
    FrontendRequest,
    FrontendResponse,
    FrontendResponseStatus,
)
from marginalia_core.application.frontend.events import FrontendEvent
from marginalia_core.application.frontend.gateway import FrontendGateway
from marginalia_core.application.frontend.queries import FrontendQueryName
from marginalia_core.application.frontend.snapshots import (
    AppSnapshot,
    DocumentListItem,
    SessionSnapshot,
)
from marginalia_core.application.result import OperationResult, OperationStatus
from marginalia_core.events.models import DomainEvent


class LocalFrontendGateway(FrontendGateway):
    """Backend gateway for local frontend clients."""

    def __init__(
        self,
        container: BackendContainer,
        runtime_manager: BackendRuntimeManager | None = None,
    ) -> None:
        self._container = container
        self._runtime_manager = runtime_manager or BackendRuntimeManager(container)

    def capabilities(self) -> BackendCapabilities:
        return BackendCapabilities(
            protocol_version=FRONTEND_PROTOCOL_VERSION,
            commands=tuple(command.value for command in FrontendCommandName),
            queries=tuple(query.value for query in FrontendQueryName),
            transports=("stdio-jsonl",),
            frontend_event_stream_supported=True,
            dictation_enabled=True,
            rewrite_enabled=True,
            summary_enabled=True,
        )

    def execute_command(self, request: FrontendRequest) -> FrontendResponse:
        try:
            command_name = FrontendCommandName(request.name)
        except ValueError:
            return self._error_response(request, f"Unknown command: {request.name}")

        payload = request.payload
        if command_name is FrontendCommandName.INGEST_DOCUMENT:
            target = str(payload.get("path", "")).strip()
            if not target:
                return self._error_response(request, "Command 'ingest_document' requires 'path'.")
            result = self._container.ingestion_service.ingest_text_file(Path(target).expanduser())
            return self._response_from_result(request, result)
        if command_name is FrontendCommandName.START_SESSION:
            target = str(payload.get("target", "")).strip() or None
            result = self._runtime_manager.start_session(target)
            return self._response_from_result(request, result)
        if command_name is FrontendCommandName.PAUSE_SESSION:
            result = self._container.reader_service.pause(command_source="frontend")
            return self._response_from_result(request, result)
        if command_name is FrontendCommandName.RESUME_SESSION:
            result = self._container.reader_service.resume(command_source="frontend")
            return self._response_from_result(request, result)
        if command_name is FrontendCommandName.STOP_SESSION:
            result = self._runtime_manager.stop_session()
            return self._response_from_result(request, result)
        if command_name is FrontendCommandName.REPEAT_CHUNK:
            result = self._container.reader_service.repeat_current_chunk(command_source="frontend")
            return self._response_from_result(request, result)
        if command_name is FrontendCommandName.PREVIOUS_CHUNK:
            result = self._container.reader_service.previous_chunk(command_source="frontend")
            return self._response_from_result(request, result)
        if command_name is FrontendCommandName.NEXT_CHAPTER:
            result = self._container.reader_service.next_chapter(command_source="frontend")
            return self._response_from_result(request, result)
        if command_name is FrontendCommandName.CREATE_NOTE:
            transcript = str(payload.get("text", "")).strip()
            if not transcript:
                return self._error_response(request, "Command 'create_note' requires 'text'.")
            start_result = self._container.note_service.start_note_capture()
            if start_result.status is OperationStatus.ERROR:
                return self._response_from_result(request, start_result)
            result = self._container.note_service.stop_note_capture(transcript=transcript)
            return self._response_from_result(request, result)

        return self._error_response(request, f"Unhandled command: {command_name.value}")

    def execute_query(self, request: FrontendRequest) -> FrontendResponse:
        try:
            query_name = FrontendQueryName(request.name)
        except ValueError:
            return self._error_response(request, f"Unknown query: {request.name}")

        if query_name is FrontendQueryName.GET_BACKEND_CAPABILITIES:
            return FrontendResponse(
                status=FrontendResponseStatus.OK,
                name=request.name,
                message="Backend capabilities reported.",
                payload=to_transport_dict(asdict(self.capabilities())),
                request_id=request.request_id,
            )
        if query_name is FrontendQueryName.GET_APP_SNAPSHOT:
            payload = {"app": asdict(self._build_app_snapshot())}
            return self._ok_response(request, "App snapshot reported.", payload)
        if query_name is FrontendQueryName.GET_SESSION_SNAPSHOT:
            session_snapshot = self._build_session_snapshot()
            payload = {
                "session": asdict(session_snapshot) if session_snapshot is not None else None
            }
            return self._ok_response(request, "Session snapshot reported.", payload)
        if query_name is FrontendQueryName.LIST_DOCUMENTS:
            documents = [asdict(document) for document in self._list_documents()]
            return self._ok_response(request, "Document list reported.", {"documents": documents})
        if query_name is FrontendQueryName.GET_DOCTOR_REPORT:
            return self._ok_response(request, "Doctor report reported.", self._doctor_report())

        return self._error_response(request, f"Unhandled query: {query_name.value}")

    def recent_events(self, limit: int = 50) -> tuple[FrontendEvent, ...]:
        events = self._container.event_bus.recent(limit=limit)
        return tuple(self._map_domain_event(event) for event in events)

    def _build_app_snapshot(self) -> AppSnapshot:
        result = self._container.session_query_service.current_status()
        data = result.data
        playback = data.get("playback")
        session = data.get("session")
        runtime = data.get("runtime", {})
        playback_state = None
        if playback is not None:
            playback_state = getattr(getattr(playback, "state", None), "value", None)
        return AppSnapshot(
            active_session_id=getattr(session, "session_id", None),
            document_count=len(self._container.document_repository.list_documents()),
            latest_document_id=data.get("latest_document_id"),
            playback_state=playback_state,
            runtime_status=runtime.get("runtime_status"),
            state=str(
                data.get("state") or getattr(getattr(session, "state", None), "value", "IDLE")
            ),
        )

    def _build_session_snapshot(self) -> SessionSnapshot | None:
        result = self._container.session_query_service.current_status()
        session = result.data.get("session") if result.data else None
        if session is None:
            return None

        progress = result.data.get("progress", {})
        position = result.data.get("position", {})
        counts = result.data.get("counts", {})
        providers = result.data.get("providers", {})
        playback = result.data.get("playback")
        playback_state = getattr(getattr(playback, "state", None), "value", "stopped")
        return SessionSnapshot(
            anchor=str(position.get("anchor", session.position.anchor)),
            chunk_index=int(progress.get("chunk_index", 0)),
            chunk_text=str(position.get("chunk_text", "")),
            command_listening_active=bool(
                result.data.get("runtime", {}).get("command_listening_active", False)
            ),
            command_stt_provider=providers.get("command_stt"),
            document_id=session.document_id,
            notes_count=int(counts.get("notes", 0)),
            playback_provider=providers.get("playback"),
            playback_state=playback_state,
            section_count=int(progress.get("section_count", 0)),
            section_index=int(progress.get("section_index", 0)),
            section_title=str(position.get("section_title", "")),
            session_id=session.session_id,
            state=getattr(session.state, "value", str(session.state)),
            tts_provider=providers.get("tts"),
            voice=providers.get("voice"),
        )

    def _list_documents(self) -> tuple[DocumentListItem, ...]:
        documents = self._container.document_repository.list_documents()
        return tuple(
            DocumentListItem(
                chapter_count=document.chapter_count,
                chunk_count=document.total_chunk_count,
                document_id=document.document_id,
                title=document.title,
            )
            for document in documents
        )

    def _doctor_report(self) -> dict[str, object]:
        report = self._container.settings.doctor_report()
        report["database"] = self._container.database.health_report()
        provider_capabilities = {
            "command_stt": self._container.command_stt.describe_capabilities(),
            "dictation_stt": self._container.dictation_stt.describe_capabilities(),
            "tts": self._container.speech_synthesizer.describe_capabilities(),
            "playback": self._container.playback_engine.describe_capabilities(),
            "rewrite": self._container.rewrite_provider.describe_capabilities(),
            "summary": self._container.summary_provider.describe_capabilities(),
        }
        report["provider_capabilities"] = provider_capabilities
        report["resolved_providers"] = {
            key: value.provider_name for key, value in provider_capabilities.items()
        }
        report["command_lexicon"] = {
            "language": self._container.command_lexicon.language,
            "source_path": self._container.command_lexicon.source_path,
            "phrases": self._container.command_lexicon.grammar,
        }
        report["runtime"] = {
            "active_runtime": self._container.runtime_supervisor.current_runtime(),
            "uses_default_audio_devices": True,
        }
        return to_transport_dict(report)

    def _map_domain_event(self, event: DomainEvent) -> FrontendEvent:
        return FrontendEvent(
            name=event.name.value,
            payload=to_transport_dict(event.payload),
            event_id=event.event_id,
            occurred_at=event.occurred_at,
        )

    def _ok_response(
        self,
        request: FrontendRequest,
        message: str,
        payload: dict[str, object],
    ) -> FrontendResponse:
        return FrontendResponse(
            status=FrontendResponseStatus.OK,
            name=request.name,
            message=message,
            payload=to_transport_dict(payload),
            request_id=request.request_id,
        )

    def _error_response(self, request: FrontendRequest, message: str) -> FrontendResponse:
        return FrontendResponse(
            status=FrontendResponseStatus.ERROR,
            name=request.name,
            message=message,
            request_id=request.request_id,
        )

    def _response_from_result(
        self,
        request: FrontendRequest,
        result: OperationResult,
    ) -> FrontendResponse:
        status = (
            FrontendResponseStatus.ERROR
            if result.status is OperationStatus.ERROR
            else FrontendResponseStatus.OK
        )
        return FrontendResponse(
            status=status,
            name=request.name,
            message=result.message,
            payload=to_transport_dict(result.data),
            request_id=request.request_id,
        )
