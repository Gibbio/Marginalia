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
    DocumentChunkView,
    DocumentListItem,
    DocumentSectionView,
    DocumentView,
    NotesSnapshot,
    NoteView,
    SearchResultsSnapshot,
    SearchResultView,
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
        if command_name is FrontendCommandName.NEXT_CHUNK:
            result = self._container.reader_service.next_chunk(command_source="frontend")
            return self._response_from_result(request, result)
        if command_name is FrontendCommandName.RESTART_CHAPTER:
            result = self._container.reader_service.restart_chapter(command_source="frontend")
            return self._response_from_result(request, result)
        if command_name is FrontendCommandName.PREVIOUS_CHAPTER:
            result = self._container.reader_service.previous_chapter(command_source="frontend")
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
        if query_name is FrontendQueryName.GET_DOCUMENT_VIEW:
            document_view = self._build_document_view(request.payload)
            if document_view is None:
                return self._error_response(request, "No document is available for document view.")
            return self._ok_response(
                request,
                "Document view reported.",
                {"document": asdict(document_view)},
            )
        if query_name is FrontendQueryName.GET_SESSION_SNAPSHOT:
            session_snapshot = self._build_session_snapshot()
            payload = {
                "session": asdict(session_snapshot) if session_snapshot is not None else None
            }
            return self._ok_response(request, "Session snapshot reported.", payload)
        if query_name is FrontendQueryName.LIST_NOTES:
            notes_snapshot = self._build_notes_snapshot(request.payload)
            if notes_snapshot is None:
                return self._error_response(request, "No document is available for notes view.")
            return self._ok_response(
                request,
                "Notes snapshot reported.",
                {"notes": asdict(notes_snapshot)},
            )
        if query_name is FrontendQueryName.LIST_DOCUMENTS:
            documents = [asdict(document) for document in self._list_documents()]
            return self._ok_response(request, "Document list reported.", {"documents": documents})
        if query_name is FrontendQueryName.SEARCH_DOCUMENTS:
            query_text = str(request.payload.get("query", "")).strip()
            result = self._container.search_service.search_documents(query_text)
            if result.status is OperationStatus.ERROR:
                return self._error_response(request, result.message)
            return self._ok_response(
                request,
                "Document search reported.",
                {"search": asdict(self._search_snapshot_from_result(query_text, result))},
            )
        if query_name is FrontendQueryName.SEARCH_NOTES:
            query_text = str(request.payload.get("query", "")).strip()
            result = self._container.search_service.search_notes(query_text)
            if result.status is OperationStatus.ERROR:
                return self._error_response(request, result.message)
            return self._ok_response(
                request,
                "Note search reported.",
                {"search": asdict(self._search_snapshot_from_result(query_text, result))},
            )
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

    def _build_document_view(self, payload: dict[str, object]) -> DocumentView | None:
        document = self._resolve_document_from_payload(payload)
        if document is None:
            return None

        session_snapshot = self._build_session_snapshot()
        active_document = (
            session_snapshot is not None and session_snapshot.document_id == document.document_id
        )
        active_section_index = session_snapshot.section_index if active_document else None
        active_chunk_index = session_snapshot.chunk_index if active_document else None

        sections: list[DocumentSectionView] = []
        for section in document.sections:
            chunks: list[DocumentChunkView] = []
            for chunk in section.chunks:
                is_active = (
                    active_section_index == section.index and active_chunk_index == chunk.index
                )
                is_read = active_section_index is not None and (
                    section.index < active_section_index
                    or (
                        section.index == active_section_index
                        and active_chunk_index is not None
                        and chunk.index < active_chunk_index
                    )
                )
                chunks.append(
                    DocumentChunkView(
                        anchor=chunk.anchor,
                        char_end=chunk.char_end,
                        char_start=chunk.char_start,
                        index=chunk.index,
                        is_active=is_active,
                        is_read=is_read,
                        text=chunk.text,
                    )
                )
            sections.append(
                DocumentSectionView(
                    chunk_count=section.chunk_count,
                    chunks=tuple(chunks),
                    index=section.index,
                    source_anchor=section.source_anchor,
                    title=section.title,
                )
            )

        return DocumentView(
            active_chunk_index=active_chunk_index,
            active_section_index=active_section_index,
            chapter_count=document.chapter_count,
            chunk_count=document.total_chunk_count,
            document_id=document.document_id,
            sections=tuple(sections),
            source_path=str(document.source_path),
            title=document.title,
        )

    def _build_notes_snapshot(self, payload: dict[str, object]) -> NotesSnapshot | None:
        document = self._resolve_document_from_payload(payload)
        if document is None:
            return None

        notes = self._container.note_repository.list_notes_for_document(document.document_id)
        return NotesSnapshot(
            document_id=document.document_id,
            notes=tuple(
                NoteView(
                    anchor=note.anchor,
                    created_at=note.created_at,
                    document_id=note.document_id,
                    language=note.language,
                    note_id=note.note_id,
                    section_index=note.position.section_index,
                    chunk_index=note.position.chunk_index,
                    session_id=note.session_id,
                    transcript=note.transcript,
                    transcription_provider=note.transcription_provider,
                )
                for note in notes
            ),
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

    def _search_snapshot_from_result(
        self,
        query_text: str,
        result: OperationResult,
    ) -> SearchResultsSnapshot:
        return SearchResultsSnapshot(
            query=query_text,
            results=tuple(
                SearchResultView(
                    anchor=search_result.anchor,
                    entity_id=search_result.entity_id,
                    entity_kind=search_result.entity_kind,
                    excerpt=search_result.excerpt,
                    score=search_result.score,
                )
                for search_result in result.data.get("results", [])
            ),
        )

    def _resolve_document_from_payload(self, payload: dict[str, object]):
        document_id = str(payload.get("document_id", "")).strip()
        if document_id:
            return self._container.document_repository.get_document(document_id)

        session = self._container.session_repository.get_active_session()
        if session is not None:
            document = self._container.document_repository.get_document(session.document_id)
            if document is not None:
                return document

        documents = self._container.document_repository.list_documents()
        return documents[0] if documents else None

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
