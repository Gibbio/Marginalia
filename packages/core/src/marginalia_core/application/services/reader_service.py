"""Reader lifecycle orchestration."""

from __future__ import annotations

from uuid import uuid4

from marginalia_core.application.result import OperationResult
from marginalia_core.application.state_machine import InvalidTransitionError, ReaderStateMachine
from marginalia_core.domain.reading_session import ReaderState, ReadingPosition, ReadingSession
from marginalia_core.events.models import DomainEvent, EventName
from marginalia_core.ports.events import EventPublisher
from marginalia_core.ports.playback import PlaybackEngine, PlaybackSnapshot
from marginalia_core.ports.storage import DocumentRepository, SessionRepository
from marginalia_core.ports.stt import CommandRecognizer
from marginalia_core.ports.tts import SpeechSynthesizer, SynthesisRequest


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
        default_voice: str = "marginalia-default",
    ) -> None:
        self._document_repository = document_repository
        self._session_repository = session_repository
        self._playback_engine = playback_engine
        self._speech_synthesizer = speech_synthesizer
        self._event_publisher = event_publisher
        self._command_recognizer = command_recognizer
        self._state_machine = ReaderStateMachine()
        self._default_voice = default_voice

    def play(self, document_id: str | None) -> OperationResult:
        session = self._session_repository.get_active_session()
        latest_document = self._document_repository.list_documents()
        target_document_id = document_id or (session.document_id if session else None)
        if target_document_id is None and latest_document:
            target_document_id = latest_document[0].document_id
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
            target_state = (
                ReaderState.READING if session.state is not ReaderState.READING else session.state
            )
            self._state_machine.transition(session, target_state)
        except InvalidTransitionError as exc:
            return OperationResult.error(str(exc))

        current_chunk = document.get_chunk(
            session.position.section_index,
            session.position.chunk_index,
        )
        synthesis_result = self._speech_synthesizer.synthesize(
            SynthesisRequest(text=current_chunk.text, voice=self._default_voice)
        )
        playback_snapshot = self._playback_snapshot_for_session(
            session,
            self._playback_engine.start(
                document,
                session.position,
                synthesis=synthesis_result,
            ),
        )
        session.last_command = "play"
        session.touch()
        self._session_repository.save_session(session)
        self._publish(
            EventName.PLAYBACK_STARTED,
            session_id=session.session_id,
            document_id=session.document_id,
            anchor=session.position.anchor,
            audio_reference=synthesis_result.audio_reference,
            playback_state=playback_snapshot.state.value,
        )
        self._publish(
            EventName.READING_STARTED,
            session_id=session.session_id,
            document_id=session.document_id,
            state=session.state.value,
            anchor=session.position.anchor,
        )
        self._publish(
            EventName.READING_PROGRESSED,
            session_id=session.session_id,
            document_id=session.document_id,
            section_index=session.position.section_index,
            chunk_index=session.position.chunk_index,
            anchor=session.position.anchor,
        )
        return OperationResult.ok(
            "Reading session is active. Audio output is still backed by a fake adapter.",
            data={
                "session": session,
                "document_title": document.title,
                "current_chunk": current_chunk.text,
                "synthesis": synthesis_result,
                "playback": playback_snapshot,
            },
        )

    def pause(self) -> OperationResult:
        session = self._session_repository.get_active_session()
        if session is None:
            return OperationResult.error("No active reading session exists.")
        try:
            self._state_machine.transition(session, ReaderState.PAUSED)
        except InvalidTransitionError as exc:
            return OperationResult.error(str(exc))

        playback_snapshot = self._playback_snapshot_for_session(
            session,
            self._playback_engine.pause(),
        )
        session.last_command = "pause"
        self._session_repository.save_session(session)
        self._publish(
            EventName.READING_PAUSED,
            session_id=session.session_id,
            document_id=session.document_id,
            state=session.state.value,
        )
        self._publish(
            EventName.PLAYBACK_PAUSED,
            session_id=session.session_id,
            document_id=session.document_id,
            playback_state=playback_snapshot.state.value,
        )
        return OperationResult.ok(
            "Active reading session paused.",
            data={"session": session, "playback": playback_snapshot},
        )

    def resume(self) -> OperationResult:
        session = self._session_repository.get_active_session()
        if session is None:
            return OperationResult.error("No active reading session exists.")
        try:
            self._state_machine.transition(session, ReaderState.READING)
        except InvalidTransitionError as exc:
            return OperationResult.error(str(exc))

        playback_snapshot = self._playback_snapshot_for_session(
            session,
            self._playback_engine.resume(),
        )
        session.last_command = "resume"
        self._session_repository.save_session(session)
        self._publish(
            EventName.READING_RESUMED,
            session_id=session.session_id,
            document_id=session.document_id,
            state=session.state.value,
        )
        self._publish(
            EventName.PLAYBACK_RESUMED,
            session_id=session.session_id,
            document_id=session.document_id,
            playback_state=playback_snapshot.state.value,
        )
        return OperationResult.ok(
            "Active reading session resumed.",
            data={"session": session, "playback": playback_snapshot},
        )

    def repeat_current_chunk(self) -> OperationResult:
        session = self._session_repository.get_active_session()
        if session is None:
            return OperationResult.error("No active reading session exists.")

        document = self._document_repository.get_document(session.document_id)
        if document is None:
            return OperationResult.error("The active session references a missing document.")

        section = document.get_section(session.position.section_index)
        chunk = section.get_chunk(session.position.chunk_index)
        return OperationResult.ok(
            "Current chunk located.",
            data={
                "session": session,
                "anchor": session.position.anchor,
                "section_title": section.title,
                "chunk_text": chunk.text,
            },
        )

    def restart_chapter(self) -> OperationResult:
        session = self._session_repository.get_active_session()
        if session is None:
            return OperationResult.error("No active reading session exists.")

        session.position = ReadingPosition(
            section_index=session.position.section_index,
            chunk_index=0,
        )
        session.last_command = "restart-chapter"
        playback_snapshot = self._playback_snapshot_for_session(
            session,
            self._playback_engine.seek(session.position),
        )
        self._session_repository.save_session(session)
        self._publish(
            EventName.CHAPTER_RESTARTED,
            session_id=session.session_id,
            document_id=session.document_id,
            section_index=session.position.section_index,
            anchor=session.position.anchor,
        )
        self._publish(
            EventName.READING_PROGRESSED,
            session_id=session.session_id,
            document_id=session.document_id,
            section_index=session.position.section_index,
            chunk_index=session.position.chunk_index,
            anchor=session.position.anchor,
        )
        return OperationResult.ok(
            "Session moved to the start of the current chapter.",
            data={"session": session, "playback": playback_snapshot},
        )

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
        playback_snapshot = self._playback_snapshot_for_session(
            session,
            self._playback_engine.seek(session.position),
        )
        self._session_repository.save_session(session)
        self._publish(
            EventName.CHAPTER_ADVANCED,
            session_id=session.session_id,
            document_id=session.document_id,
            section_index=session.position.section_index,
            anchor=session.position.anchor,
        )
        self._publish(
            EventName.READING_PROGRESSED,
            session_id=session.session_id,
            document_id=session.document_id,
            section_index=session.position.section_index,
            chunk_index=session.position.chunk_index,
            anchor=session.position.anchor,
        )
        return OperationResult.ok(
            "Session moved to the next chapter.",
            data={"session": session, "playback": playback_snapshot},
        )

    def _publish(self, event_name: EventName, **payload: object) -> None:
        self._event_publisher.publish(DomainEvent(name=event_name, payload=payload))

    def _playback_snapshot_for_session(
        self,
        session: ReadingSession,
        snapshot: PlaybackSnapshot,
    ) -> PlaybackSnapshot:
        return PlaybackSnapshot(
            state=session.playback_state,
            last_action=snapshot.last_action,
            document_id=session.document_id,
            anchor=session.position.anchor,
            progress_units=snapshot.progress_units,
            audio_reference=snapshot.audio_reference,
        )
