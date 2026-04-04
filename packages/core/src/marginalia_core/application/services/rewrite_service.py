"""Rewrite workflow orchestration."""

from __future__ import annotations

from uuid import uuid4

from marginalia_core.application.result import OperationResult
from marginalia_core.application.state_machine import InvalidTransitionError, ReaderStateMachine
from marginalia_core.domain.reading_session import ReaderState
from marginalia_core.domain.rewrite import RewriteDraft, RewriteStatus
from marginalia_core.events.models import DomainEvent, EventName
from marginalia_core.ports.events import EventPublisher
from marginalia_core.ports.llm import RewriteGenerator
from marginalia_core.ports.storage import (
    DocumentRepository,
    NoteRepository,
    RewriteDraftRepository,
    SessionRepository,
)


class RewriteService:
    """Create placeholder rewrite drafts informed by local notes."""

    def __init__(
        self,
        *,
        session_repository: SessionRepository,
        note_repository: NoteRepository,
        draft_repository: RewriteDraftRepository,
        document_repository: DocumentRepository,
        rewrite_generator: RewriteGenerator,
        event_publisher: EventPublisher,
    ) -> None:
        self._session_repository = session_repository
        self._note_repository = note_repository
        self._draft_repository = draft_repository
        self._document_repository = document_repository
        self._rewrite_generator = rewrite_generator
        self._event_publisher = event_publisher
        self._state_machine = ReaderStateMachine()

    def rewrite_current_section(self) -> OperationResult:
        session = self._session_repository.get_active_session()
        if session is None:
            return OperationResult.error("No active reading session exists.")

        document = self._document_repository.get_document(session.document_id)
        if document is None:
            return OperationResult.error("The active session references a missing document.")

        notes = self._note_repository.list_notes_for_document(session.document_id)
        section_notes = tuple(
            note.transcript
            for note in notes
            if note.position.section_index == session.position.section_index
        )
        if not section_notes:
            return OperationResult.planned(
                "Rewrite generation is wired, but the current section has no captured notes yet."
            )

        try:
            self._state_machine.transition(session, ReaderState.PROCESSING_REWRITE)
        except InvalidTransitionError as exc:
            return OperationResult.error(str(exc))

        section = document.sections[session.position.section_index]
        rewritten_text = self._rewrite_generator.rewrite_section(section.text, section_notes)
        draft = RewriteDraft(
            draft_id=str(uuid4()),
            document_id=document.document_id,
            section_index=section.index,
            source_excerpt=section.text[:600],
            note_transcripts=section_notes,
            rewritten_text=rewritten_text,
            status=RewriteStatus.GENERATED,
        )
        self._draft_repository.save_draft(draft)
        self._state_machine.transition(session, ReaderState.PAUSED)
        self._session_repository.save_session(session)
        self._event_publisher.publish(
            DomainEvent(
                name=EventName.REWRITE_COMPLETED,
                payload={
                    "session_id": session.session_id,
                    "document_id": session.document_id,
                    "draft_id": draft.draft_id,
                },
            )
        )
        return OperationResult.ok(
            "Placeholder rewrite draft generated through the fake provider.",
            data={"draft": draft},
        )
