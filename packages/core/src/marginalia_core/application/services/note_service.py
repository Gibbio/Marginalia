"""Voice note orchestration."""

from __future__ import annotations

from uuid import uuid4

from marginalia_core.application.result import OperationResult
from marginalia_core.application.state_machine import InvalidTransitionError, ReaderStateMachine
from marginalia_core.domain.note import VoiceNote
from marginalia_core.domain.reading_session import ReaderState
from marginalia_core.events.models import DomainEvent, EventName
from marginalia_core.ports.events import EventPublisher
from marginalia_core.ports.storage import NoteRepository, SessionRepository
from marginalia_core.ports.stt import DictationTranscriber, DictationTranscript


class NoteService:
    """Manage note recording lifecycle."""

    def __init__(
        self,
        *,
        session_repository: SessionRepository,
        note_repository: NoteRepository,
        dictation_transcriber: DictationTranscriber,
        event_publisher: EventPublisher,
    ) -> None:
        self._session_repository = session_repository
        self._note_repository = note_repository
        self._dictation_transcriber = dictation_transcriber
        self._event_publisher = event_publisher
        self._state_machine = ReaderStateMachine()

    def start_note_capture(self) -> OperationResult:
        session = self._session_repository.get_active_session()
        if session is None:
            return OperationResult.error("No active reading session exists.")

        try:
            self._state_machine.transition(session, ReaderState.RECORDING_NOTE)
        except InvalidTransitionError as exc:
            return OperationResult.error(str(exc))

        session.active_note_id = str(uuid4())
        session.last_command = "note-start"
        self._session_repository.save_session(session)
        self._event_publisher.publish(
            DomainEvent(
                name=EventName.NOTE_RECORDING_STARTED,
                payload={"session_id": session.session_id, "document_id": session.document_id},
            )
        )
        return OperationResult.ok(
            "Session moved into note recording mode.",
            data={"session": session},
        )

    def stop_note_capture(self, *, transcript: str | None = None) -> OperationResult:
        session = self._session_repository.get_active_session()
        if session is None:
            return OperationResult.error("No active reading session exists.")
        if session.active_note_id is None:
            return OperationResult.error("No note capture is currently active.")

        transcript_result = (
            self._manual_transcript(transcript)
            if transcript
            else self._dictation_transcriber.transcribe(
                session_id=session.session_id,
                note_id=session.active_note_id,
            )
        )
        if not transcript_result.text.strip():
            return OperationResult.error("Captured note text is empty.")
        note = VoiceNote(
            note_id=session.active_note_id,
            session_id=session.session_id,
            document_id=session.document_id,
            position=session.position,
            transcript=transcript_result.text,
            transcription_provider=transcript_result.provider_name,
            language=transcript_result.language,
        )
        self._note_repository.save_note(note)

        try:
            self._state_machine.transition(session, ReaderState.PAUSED)
        except InvalidTransitionError:
            session.state = ReaderState.PAUSED
            session.touch()

        self._event_publisher.publish(
            DomainEvent(
                name=EventName.NOTE_RECORDING_STOPPED,
                payload={
                    "session_id": session.session_id,
                    "document_id": session.document_id,
                    "note_id": note.note_id,
                    "provider_name": transcript_result.provider_name,
                },
            )
        )
        session.active_note_id = None
        session.last_command = "note-stop"
        self._session_repository.save_session(session)
        self._event_publisher.publish(
            DomainEvent(
                name=EventName.NOTE_SAVED,
                payload={
                    "session_id": session.session_id,
                    "document_id": session.document_id,
                    "note_id": note.note_id,
                    "anchor": note.anchor,
                    "language": note.language,
                },
            )
        )
        return OperationResult.ok(
            "Anchored voice note saved.",
            data={"note": note, "session": session, "transcript": transcript_result},
        )

    def _manual_transcript(self, transcript: str) -> DictationTranscript:
        cleaned = transcript.strip()
        return DictationTranscript(
            text=cleaned,
            provider_name="cli-manual",
            language="en",
            segments=(),
            raw_text=cleaned,
            is_final=True,
        )
