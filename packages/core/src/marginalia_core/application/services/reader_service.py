"""Reader lifecycle orchestration."""

from __future__ import annotations

from uuid import uuid4

from marginalia_core.application.result import OperationResult
from marginalia_core.application.state_machine import InvalidTransitionError, ReaderStateMachine
from marginalia_core.domain.reading_session import ReaderState, ReadingPosition, ReadingSession
from marginalia_core.events.models import DomainEvent, EventName
from marginalia_core.ports.events import EventPublisher
from marginalia_core.ports.playback import PlaybackEngine
from marginalia_core.ports.storage import DocumentRepository, SessionRepository
from marginalia_core.ports.stt import CommandRecognizer
from marginalia_core.ports.tts import SpeechSynthesizer


class ReaderService:
    """Manage reading session lifecycle and cursor movement."""

    def __init__(
        self,
        *,
        document_repository: DocumentRepository,
        session_repository: SessionRepository,
        playback_engine: PlaybackEngine,
        speech_synthesizer: SpeechSynthesizer,
        event_publisher: EventPublisher,
        command_recognizer: CommandRecognizer,
    ) -> None:
        self._document_repository = document_repository
        self._session_repository = session_repository
        self._playback_engine = playback_engine
        self._speech_synthesizer = speech_synthesizer
        self._event_publisher = event_publisher
        self._command_recognizer = command_recognizer
        self._state_machine = ReaderStateMachine()

    def play(self, document_id: str | None) -> OperationResult:
        session = self._session_repository.get_active_session()
        target_document_id = document_id or (session.document_id if session else None)
        if target_document_id is None:
            return OperationResult.error("No active session and no document id was provided.")

        document = self._document_repository.get_document(target_document_id)
        if document is None:
            return OperationResult.error(
                f"Document '{target_document_id}' was not found in local storage."
            )

        if session is None or session.document_id != target_document_id:
            session = ReadingSession(
                session_id=str(uuid4()),
                document_id=target_document_id,
                position=ReadingPosition(),
            )

        try:
            target_state = ReaderState.READING if session.state is not ReaderState.READING else session.state
            self._state_machine.transition(session, target_state)
        except InvalidTransitionError as exc:
            return OperationResult.error(str(exc))

        self._playback_engine.start(document, session.position)
        self._speech_synthesizer.synthesize(
            document.sections[session.position.section_index].chunks[session.position.chunk_index].text
        )
        session.last_command = "play"
        session.touch()
        self._session_repository.save_session(session)
        self._publish(
            EventName.READING_STARTED,
            session_id=session.session_id,
            document_id=session.document_id,
            state=session.state.value,
        )
        return OperationResult.ok(
            "Reading session is active. Audio output is still backed by a fake adapter.",
            data={"session": session},
        )

    def pause(self) -> OperationResult:
        session = self._session_repository.get_active_session()
        if session is None:
            return OperationResult.error("No active reading session exists.")
        try:
            self._state_machine.transition(session, ReaderState.PAUSED)
        except InvalidTransitionError as exc:
            return OperationResult.error(str(exc))

        self._playback_engine.pause()
        session.last_command = "pause"
        self._session_repository.save_session(session)
        self._publish(
            EventName.PLAYBACK_PAUSED,
            session_id=session.session_id,
            document_id=session.document_id,
        )
        return OperationResult.ok("Active reading session paused.", data={"session": session})

    def resume(self) -> OperationResult:
        session = self._session_repository.get_active_session()
        if session is None:
            return OperationResult.error("No active reading session exists.")
        try:
            self._state_machine.transition(session, ReaderState.READING)
        except InvalidTransitionError as exc:
            return OperationResult.error(str(exc))

        self._playback_engine.resume()
        session.last_command = "resume"
        self._session_repository.save_session(session)
        self._publish(
            EventName.PLAYBACK_RESUMED,
            session_id=session.session_id,
            document_id=session.document_id,
        )
        return OperationResult.ok("Active reading session resumed.", data={"session": session})

    def repeat_current_chunk(self) -> OperationResult:
        session = self._session_repository.get_active_session()
        if session is None:
            return OperationResult.error("No active reading session exists.")

        document = self._document_repository.get_document(session.document_id)
        if document is None:
            return OperationResult.error("The active session references a missing document.")

        section = document.sections[session.position.section_index]
        chunk = section.chunks[session.position.chunk_index]
        return OperationResult.ok(
            "Current chunk located.",
            data={
                "session": session,
                "section_title": section.title,
                "chunk_text": chunk.text,
            },
        )

    def restart_chapter(self) -> OperationResult:
        session = self._session_repository.get_active_session()
        if session is None:
            return OperationResult.error("No active reading session exists.")

        session.position = ReadingPosition(section_index=session.position.section_index, chunk_index=0)
        session.last_command = "restart-chapter"
        self._playback_engine.seek(session.position)
        self._session_repository.save_session(session)
        self._publish(
            EventName.READING_POSITION_CHANGED,
            session_id=session.session_id,
            document_id=session.document_id,
            section_index=session.position.section_index,
            chunk_index=session.position.chunk_index,
        )
        return OperationResult.ok("Session moved to the start of the current chapter.", data={"session": session})

    def next_chapter(self) -> OperationResult:
        session = self._session_repository.get_active_session()
        if session is None:
            return OperationResult.error("No active reading session exists.")

        document = self._document_repository.get_document(session.document_id)
        if document is None:
            return OperationResult.error("The active session references a missing document.")

        next_section_index = session.position.section_index + 1
        if next_section_index >= len(document.sections):
            return OperationResult.error("Already at the final chapter.")

        session.position = ReadingPosition(section_index=next_section_index, chunk_index=0)
        session.last_command = "next-chapter"
        self._playback_engine.seek(session.position)
        self._session_repository.save_session(session)
        self._publish(
            EventName.READING_POSITION_CHANGED,
            session_id=session.session_id,
            document_id=session.document_id,
            section_index=session.position.section_index,
            chunk_index=session.position.chunk_index,
        )
        return OperationResult.ok("Session moved to the next chapter.", data={"session": session})

    def status(self) -> OperationResult:
        session = self._session_repository.get_active_session()
        if session is None:
            return OperationResult.ok(
                "No active session. Marginalia is idle.",
                data={"state": ReaderState.IDLE.value},
            )
        return OperationResult.ok("Active session located.", data={"session": session})

    def _publish(self, event_name: EventName, **payload: object) -> None:
        self._event_publisher.publish(DomainEvent(name=event_name, payload=payload))
