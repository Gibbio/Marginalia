"""Query services for current session state."""

from __future__ import annotations

from marginalia_core.application.result import OperationResult
from marginalia_core.domain.reading_session import ReaderState
from marginalia_core.ports.storage import (
    DocumentRepository,
    NoteRepository,
    RewriteDraftRepository,
    SessionRepository,
)


class SessionQueryService:
    """Assemble the current session state for CLI-friendly reporting."""

    def __init__(
        self,
        *,
        session_repository: SessionRepository,
        document_repository: DocumentRepository,
        note_repository: NoteRepository,
        draft_repository: RewriteDraftRepository,
    ) -> None:
        self._session_repository = session_repository
        self._document_repository = document_repository
        self._note_repository = note_repository
        self._draft_repository = draft_repository

    def current_status(self) -> OperationResult:
        session = self._session_repository.get_active_session()
        documents = self._document_repository.list_documents()
        if session is None:
            return OperationResult.ok(
                "No active session. Marginalia is idle.",
                data={
                    "state": ReaderState.IDLE.value,
                    "document_count": len(documents),
                    "latest_document_id": documents[0].document_id if documents else None,
                },
            )

        document = self._document_repository.get_document(session.document_id)
        if document is None:
            return OperationResult.error("The active session references a missing document.")

        current_section = document.get_section(session.position.section_index)
        current_chunk = document.get_chunk(
            session.position.section_index,
            session.position.chunk_index,
        )
        note_count = len(self._note_repository.list_notes_for_document(document.document_id))
        draft_count = len(self._draft_repository.list_drafts_for_document(document.document_id))

        return OperationResult.ok(
            "Active session located.",
            data={
                "session": session,
                "document": {
                    "document_id": document.document_id,
                    "title": document.title,
                    "chapter_count": document.chapter_count,
                    "chunk_count": document.total_chunk_count,
                },
                "position": {
                    "anchor": session.position.anchor,
                    "section_title": current_section.title,
                    "chunk_text": current_chunk.text,
                },
                "counts": {
                    "notes": note_count,
                    "drafts": draft_count,
                },
            },
        )
