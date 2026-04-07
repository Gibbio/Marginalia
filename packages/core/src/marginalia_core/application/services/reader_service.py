"""Reader lifecycle orchestration."""

from __future__ import annotations

import logging
import threading
from collections.abc import Callable
from uuid import uuid4

from marginalia_core.application.command_router import (
    CommandLexicon,
    VoiceCommandIntent,
    resolve_voice_command,
)
from marginalia_core.application.result import OperationResult, OperationStatus
from marginalia_core.application.state_machine import InvalidTransitionError, ReaderStateMachine
from marginalia_core.domain.document import Document, DocumentChunk, DocumentSection
from marginalia_core.domain.reading_session import (
    PlaybackState,
    ReaderState,
    ReadingPosition,
    ReadingSession,
)
from marginalia_core.events.models import DomainEvent, EventName
from marginalia_core.ports.events import EventPublisher
from marginalia_core.ports.playback import PlaybackEngine, PlaybackSnapshot
from marginalia_core.ports.storage import DocumentRepository, SessionRepository
from marginalia_core.ports.stt import CommandRecognition, CommandRecognizer
from marginalia_core.ports.tts import SpeechSynthesizer, SynthesisRequest

logger = logging.getLogger(__name__)


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
        command_lexicon: CommandLexicon,
        default_voice: str = "marginalia-default",
    ) -> None:
        self._document_repository = document_repository
        self._session_repository = session_repository
        self._playback_engine = playback_engine
        self._speech_synthesizer = speech_synthesizer
        self._event_publisher = event_publisher
        self._command_recognizer = command_recognizer
        self._command_lexicon = command_lexicon
        self._state_machine = ReaderStateMachine()
        self._default_voice = default_voice
        self._tts_provider_name = speech_synthesizer.describe_capabilities().provider_name
        self._command_provider_name = command_recognizer.describe_capabilities().provider_name
        self._playback_provider_name = playback_engine.describe_capabilities().provider_name
        self._pre_synth_thread: threading.Thread | None = None

    def play(
        self,
        document_id: str | None,
        *,
        command_source: str = "cli",
        recognized_command: str | None = None,
    ) -> OperationResult:
        logger.info("Play requested for document %s (source=%s)", document_id, command_source)
        session = self._session_repository.get_active_session()
        latest_documents = self._document_repository.list_documents()
        target_document_id = document_id or (session.document_id if session else None)
        if target_document_id is None and latest_documents:
            target_document_id = latest_documents[0].document_id
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
        else:
            self._synchronize_session_playback(session)

        try:
            self._state_machine.transition(session, ReaderState.READING)
        except InvalidTransitionError as exc:
            return OperationResult.error(str(exc))

        return self._start_current_chunk(
            session,
            document,
            command_name="play",
            command_source=command_source,
            recognized_command=recognized_command,
            message=self._play_message(playback_scope="current-chunk"),
            publish_started=True,
        )

    def synchronize_active_session(self) -> OperationResult:
        """Return the active session plus a fresh playback snapshot."""

        session = self._session_repository.get_active_session()
        if session is None:
            return OperationResult.error("No active reading session exists.")

        document = self._document_repository.get_document(session.document_id)
        if document is None:
            return OperationResult.error("The active session references a missing document.")

        playback_snapshot = self._synchronize_session_playback(session)
        section, chunk = self._current_location(document, session)
        return OperationResult.ok(
            "Active session synchronized.",
            data={
                "session": session,
                "document": document,
                "position": {
                    "anchor": session.position.anchor,
                    "section_title": section.title,
                    "chunk_text": chunk.text,
                },
                "progress": self._reading_progress(document, session),
                "playback": playback_snapshot,
            },
        )

    def dispatch_recognized_command(self, recognition: CommandRecognition) -> OperationResult:
        """Dispatch a pre-recognized voice command while reading is active."""

        session = self._session_repository.get_active_session()
        if session is None:
            return OperationResult.error("No active reading session exists.")

        session.last_recognized_command = recognition.command
        session.command_stt_provider = recognition.provider_name
        intent = resolve_voice_command(recognition.command, self._command_lexicon)
        if intent is None:
            logger.warning("Unrecognized command phrase: %r", recognition.command)
            session.touch()
            self._session_repository.save_session(session)
            return OperationResult.error(
                "Recognized command is outside the supported vocabulary.",
                data={"session": session, "recognition": recognition},
            )

        result = self._dispatch_voice_intent(intent, recognition)
        self._publish(
            EventName.COMMAND_DISPATCHED,
            session_id=session.session_id,
            document_id=session.document_id,
            intent=intent.value,
            phrase=recognition.command,
            status=result.status.value,
        )
        response_factory = (
            OperationResult.error if result.status is OperationStatus.ERROR else OperationResult.ok
        )
        return response_factory(
            "Voice command recognized and dispatched.",
            data={
                "recognition": recognition,
                "handled_command": intent.value,
                "command_result": result.to_dict(),
            },
        )

    def advance_after_playback_completion(self) -> OperationResult:
        """Advance to the next chunk or mark the document complete."""

        session = self._session_repository.get_active_session()
        if session is None:
            return OperationResult.error("No active reading session exists.")

        document = self._document_repository.get_document(session.document_id)
        if document is None:
            return OperationResult.error("The active session references a missing document.")

        current_section = document.get_section(session.position.section_index)
        next_chunk_index = session.position.chunk_index + 1
        if next_chunk_index < current_section.chunk_count:
            session.position = ReadingPosition(
                section_index=session.position.section_index,
                chunk_index=next_chunk_index,
            )
            result = self._start_current_chunk(
                session,
                document,
                command_name="auto-advance",
                command_source="runtime",
                recognized_command=None,
                message="Reading advanced to the next chunk.",
                publish_started=False,
            )
            return OperationResult.ok(
                "Reading advanced to the next chunk.",
                data={**result.data, "completed_document": False},
            )

        next_section_index = session.position.section_index + 1
        if next_section_index < len(document.sections):
            session.position = ReadingPosition(section_index=next_section_index, chunk_index=0)
            logger.info(
                "Chapter boundary: advancing to section %d/%d (%s)",
                next_section_index + 1,
                document.chapter_count,
                document.get_section(next_section_index).title,
            )
            result = self._start_current_chunk(
                session,
                document,
                command_name="auto-advance",
                command_source="runtime",
                recognized_command=None,
                message="Reading advanced to the next chapter.",
                publish_started=False,
            )
            self._publish(
                EventName.CHAPTER_ADVANCED,
                session_id=session.session_id,
                document_id=session.document_id,
                section_index=session.position.section_index,
                anchor=session.position.anchor,
            )
            return OperationResult.ok(
                "Reading advanced to the next chapter.",
                data={**result.data, "completed_document": False},
            )

        playback_snapshot = self._playback_engine.stop()
        session.state = ReaderState.IDLE
        session.command_listening_active = False
        session.runtime_status = "completed"
        self._apply_playback_snapshot(session, playback_snapshot)
        self._mark_command(
            session,
            command_name="document-complete",
            command_source="runtime",
            recognized_command=None,
        )
        self._session_repository.save_session(session)
        logger.info(
            "Document %s completed at section %d",
            session.document_id,
            session.position.section_index,
        )
        self._publish(
            EventName.PLAYBACK_STOPPED,
            session_id=session.session_id,
            document_id=session.document_id,
            playback_state=playback_snapshot.state.value,
        )
        self._publish(
            EventName.READING_COMPLETED,
            session_id=session.session_id,
            document_id=session.document_id,
            final_anchor=session.position.anchor,
        )
        return OperationResult.ok(
            "Document playback completed.",
            data={"session": session, "playback": playback_snapshot, "completed_document": True},
        )

    def pause(
        self,
        *,
        command_source: str = "cli",
        recognized_command: str | None = None,
    ) -> OperationResult:
        session = self._session_repository.get_active_session()
        if session is None:
            return OperationResult.error("No active reading session exists.")
        self._synchronize_session_playback(session)
        try:
            self._state_machine.transition(session, ReaderState.PAUSED)
        except InvalidTransitionError as exc:
            return OperationResult.error(str(exc))

        playback_snapshot = self._playback_engine.pause()
        self._apply_playback_snapshot(session, playback_snapshot)
        self._mark_command(
            session,
            command_name="pause",
            command_source=command_source,
            recognized_command=recognized_command,
        )
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

    def resume(
        self,
        *,
        command_source: str = "cli",
        recognized_command: str | None = None,
    ) -> OperationResult:
        session = self._session_repository.get_active_session()
        if session is None:
            return OperationResult.error("No active reading session exists.")

        document = self._document_repository.get_document(session.document_id)
        if document is None:
            return OperationResult.error("The active session references a missing document.")

        self._synchronize_session_playback(session)
        try:
            self._state_machine.transition(session, ReaderState.READING)
        except InvalidTransitionError as exc:
            return OperationResult.error(str(exc))

        playback_snapshot = self._playback_engine.resume()
        if playback_snapshot.state is not PlaybackState.PLAYING:
            return self._start_current_chunk(
                session,
                document,
                command_name="resume",
                command_source=command_source,
                recognized_command=recognized_command,
                message="Active reading session resumed.",
                publish_started=False,
            )

        self._apply_playback_snapshot(session, playback_snapshot)
        self._mark_command(
            session,
            command_name="resume",
            command_source=command_source,
            recognized_command=recognized_command,
        )
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

    def repeat_current_chunk(
        self,
        *,
        command_source: str = "cli",
        recognized_command: str | None = None,
    ) -> OperationResult:
        session = self._session_repository.get_active_session()
        if session is None:
            return OperationResult.error("No active reading session exists.")

        document = self._document_repository.get_document(session.document_id)
        if document is None:
            return OperationResult.error("The active session references a missing document.")

        self._synchronize_session_playback(session)
        if session.state is not ReaderState.READING:
            try:
                self._state_machine.transition(session, ReaderState.READING)
            except InvalidTransitionError as exc:
                return OperationResult.error(str(exc))

        return self._start_current_chunk(
            session,
            document,
            command_name="repeat",
            command_source=command_source,
            recognized_command=recognized_command,
            message="Current chunk replayed.",
            publish_started=False,
        )

    def previous_chunk(
        self,
        *,
        command_source: str = "cli",
        recognized_command: str | None = None,
    ) -> OperationResult:
        """Go back one chunk, crossing into the previous section if needed."""

        session = self._session_repository.get_active_session()
        if session is None:
            return OperationResult.error("No active reading session exists.")

        document = self._document_repository.get_document(session.document_id)
        if document is None:
            return OperationResult.error("The active session references a missing document.")

        self._synchronize_session_playback(session)

        prev_chunk = session.position.chunk_index - 1
        if prev_chunk >= 0:
            session.position = ReadingPosition(
                section_index=session.position.section_index,
                chunk_index=prev_chunk,
            )
        elif session.position.section_index > 0:
            prev_section = session.position.section_index - 1
            last_chunk = document.get_section(prev_section).chunk_count - 1
            session.position = ReadingPosition(
                section_index=prev_section,
                chunk_index=last_chunk,
            )
        else:
            return OperationResult.error("Already at the beginning of the document.")

        if session.state is not ReaderState.READING:
            try:
                self._state_machine.transition(session, ReaderState.READING)
            except InvalidTransitionError as exc:
                return OperationResult.error(str(exc))

        return self._start_current_chunk(
            session,
            document,
            command_name="rewind",
            command_source=command_source,
            recognized_command=recognized_command,
            message="Moved to the previous chunk.",
            publish_started=False,
        )

    def next_chunk(
        self,
        *,
        command_source: str = "cli",
        recognized_command: str | None = None,
    ) -> OperationResult:
        """Advance one chunk, crossing into the next section if needed."""

        session = self._session_repository.get_active_session()
        if session is None:
            return OperationResult.error("No active reading session exists.")

        document = self._document_repository.get_document(session.document_id)
        if document is None:
            return OperationResult.error("The active session references a missing document.")

        self._synchronize_session_playback(session)

        current_section = document.get_section(session.position.section_index)
        next_chunk_index = session.position.chunk_index + 1
        crossed_chapter = False
        if next_chunk_index < current_section.chunk_count:
            session.position = ReadingPosition(
                section_index=session.position.section_index,
                chunk_index=next_chunk_index,
            )
        elif session.position.section_index + 1 < len(document.sections):
            session.position = ReadingPosition(
                section_index=session.position.section_index + 1,
                chunk_index=0,
            )
            crossed_chapter = True
        else:
            return OperationResult.error("Already at the end of the document.")

        if session.state is not ReaderState.READING:
            try:
                self._state_machine.transition(session, ReaderState.READING)
            except InvalidTransitionError as exc:
                return OperationResult.error(str(exc))

        result = self._start_current_chunk(
            session,
            document,
            command_name="next-chunk",
            command_source=command_source,
            recognized_command=recognized_command,
            message="Moved to the next chunk.",
            publish_started=False,
        )
        if crossed_chapter:
            self._publish(
                EventName.CHAPTER_ADVANCED,
                session_id=session.session_id,
                document_id=session.document_id,
                section_index=session.position.section_index,
                anchor=session.position.anchor,
            )
        return result

    def previous_chapter(
        self,
        *,
        command_source: str = "cli",
        recognized_command: str | None = None,
    ) -> OperationResult:
        session = self._session_repository.get_active_session()
        if session is None:
            return OperationResult.error("No active reading session exists.")

        document = self._document_repository.get_document(session.document_id)
        if document is None:
            return OperationResult.error("The active session references a missing document.")

        self._synchronize_session_playback(session)
        previous_section_index = session.position.section_index - 1
        if previous_section_index < 0:
            return OperationResult.error("Already at the first chapter.")

        session.position = ReadingPosition(section_index=previous_section_index, chunk_index=0)
        if session.state is ReaderState.READING:
            result = self._start_current_chunk(
                session,
                document,
                command_name="previous-chapter",
                command_source=command_source,
                recognized_command=recognized_command,
                message="Session moved to the previous chapter.",
                publish_started=False,
            )
        else:
            playback_snapshot = self._playback_engine.seek(session.position)
            self._apply_playback_snapshot(session, playback_snapshot)
            self._mark_command(
                session,
                command_name="previous-chapter",
                command_source=command_source,
                recognized_command=recognized_command,
            )
            self._session_repository.save_session(session)
            result = OperationResult.ok(
                "Session moved to the previous chapter.",
                data={"session": session, "playback": playback_snapshot},
            )

        progress = self._reading_progress(document, session)
        self._publish(
            EventName.READING_PROGRESSED,
            session_id=session.session_id,
            document_id=session.document_id,
            section_index=session.position.section_index,
            chunk_index=session.position.chunk_index,
            anchor=session.position.anchor,
            section_count=progress["section_count"],
            section_chunk_count=progress["section_chunk_count"],
            chunks_read=progress["chunks_read"],
            total_chunks=progress["total_chunks"],
        )
        return result

    def restart_chapter(
        self,
        *,
        command_source: str = "cli",
        recognized_command: str | None = None,
    ) -> OperationResult:
        session = self._session_repository.get_active_session()
        if session is None:
            return OperationResult.error("No active reading session exists.")

        document = self._document_repository.get_document(session.document_id)
        if document is None:
            return OperationResult.error("The active session references a missing document.")

        self._synchronize_session_playback(session)
        session.position = ReadingPosition(
            section_index=session.position.section_index,
            chunk_index=0,
        )
        if session.state is ReaderState.READING:
            result = self._start_current_chunk(
                session,
                document,
                command_name="restart-chapter",
                command_source=command_source,
                recognized_command=recognized_command,
                message="Session moved to the start of the current chapter.",
                publish_started=False,
            )
        else:
            playback_snapshot = self._playback_engine.seek(session.position)
            self._apply_playback_snapshot(session, playback_snapshot)
            self._mark_command(
                session,
                command_name="restart-chapter",
                command_source=command_source,
                recognized_command=recognized_command,
            )
            self._session_repository.save_session(session)
            result = OperationResult.ok(
                "Session moved to the start of the current chapter.",
                data={"session": session, "playback": playback_snapshot},
            )
        progress = self._reading_progress(document, session)
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
            section_count=progress["section_count"],
            section_chunk_count=progress["section_chunk_count"],
            chunks_read=progress["chunks_read"],
            total_chunks=progress["total_chunks"],
        )
        return result

    def next_chapter(
        self,
        *,
        command_source: str = "cli",
        recognized_command: str | None = None,
    ) -> OperationResult:
        session = self._session_repository.get_active_session()
        if session is None:
            return OperationResult.error("No active reading session exists.")

        document = self._document_repository.get_document(session.document_id)
        if document is None:
            return OperationResult.error("The active session references a missing document.")

        self._synchronize_session_playback(session)
        next_section_index = session.position.section_index + 1
        if next_section_index >= len(document.sections):
            return OperationResult.error("Already at the final chapter.")

        session.position = ReadingPosition(section_index=next_section_index, chunk_index=0)
        if session.state is ReaderState.READING:
            result = self._start_current_chunk(
                session,
                document,
                command_name="next-chapter",
                command_source=command_source,
                recognized_command=recognized_command,
                message="Session moved to the next chapter.",
                publish_started=False,
            )
        else:
            playback_snapshot = self._playback_engine.seek(session.position)
            self._apply_playback_snapshot(session, playback_snapshot)
            self._mark_command(
                session,
                command_name="next-chapter",
                command_source=command_source,
                recognized_command=recognized_command,
            )
            self._session_repository.save_session(session)
            result = OperationResult.ok(
                "Session moved to the next chapter.",
                data={"session": session, "playback": playback_snapshot},
            )
        progress = self._reading_progress(document, session)
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
            section_count=progress["section_count"],
            section_chunk_count=progress["section_chunk_count"],
            chunks_read=progress["chunks_read"],
            total_chunks=progress["total_chunks"],
        )
        return result

    def stop(
        self,
        *,
        command_source: str = "cli",
        recognized_command: str | None = None,
    ) -> OperationResult:
        session = self._session_repository.get_active_session()
        if session is None:
            return OperationResult.error("No active reading session exists.")

        logger.info(
            "Stop requested for session %s (source=%s)", session.session_id, command_source
        )
        self._synchronize_session_playback(session)
        playback_snapshot = self._playback_engine.stop()
        session.state = ReaderState.IDLE
        self._apply_playback_snapshot(session, playback_snapshot)
        session.command_listening_active = False
        session.runtime_status = "stopped"
        session.runtime_error = None
        self._mark_command(
            session,
            command_name="stop",
            command_source=command_source,
            recognized_command=recognized_command,
        )
        self._session_repository.save_session(session)
        self._publish(
            EventName.PLAYBACK_STOPPED,
            session_id=session.session_id,
            document_id=session.document_id,
            playback_state=playback_snapshot.state.value,
        )
        return OperationResult.ok(
            "Active reading session stopped.",
            data={"session": session, "playback": playback_snapshot},
        )

    def report_voice_status(self, *, recognized_command: str | None = None) -> OperationResult:
        """Return the current voice-oriented session status without re-entering capture."""

        return self._voice_status(recognized_command=recognized_command)

    def _build_intent_dispatch_table(
        self,
    ) -> dict[VoiceCommandIntent, Callable[[CommandRecognition], OperationResult]]:
        """Return a mapping from each voice command intent to a handler callable."""

        def _wrap(
            method: Callable[..., OperationResult],
        ) -> Callable[[CommandRecognition], OperationResult]:
            def handler(r: CommandRecognition) -> OperationResult:
                return method(command_source="voice", recognized_command=r.command)

            return handler

        return {
            VoiceCommandIntent.PAUSE: _wrap(self.pause),
            VoiceCommandIntent.RESUME: _wrap(self.resume),
            VoiceCommandIntent.REPEAT: _wrap(self.repeat_current_chunk),
            VoiceCommandIntent.REWIND: _wrap(self.previous_chunk),
            VoiceCommandIntent.NEXT_CHAPTER: _wrap(self.next_chapter),
            VoiceCommandIntent.RESTART_CHAPTER: _wrap(self.restart_chapter),
            VoiceCommandIntent.STATUS: lambda r: self._voice_status(
                recognized_command=r.command
            ),
            VoiceCommandIntent.STOP: _wrap(self.stop),
            VoiceCommandIntent.HELP: lambda r: self._handle_help(recognized_command=r.command),
        }

    def _dispatch_voice_intent(
        self,
        intent: VoiceCommandIntent,
        recognition: CommandRecognition,
    ) -> OperationResult:
        dispatch_table = self._build_intent_dispatch_table()
        handler = dispatch_table.get(intent)
        if handler is None:
            logger.error("No handler registered for intent %s", intent.value)
            return OperationResult.error(
                f"Intent '{intent.value}' is recognized but not handled.",
            )
        logger.info("Dispatching intent %s from phrase %r", intent.value, recognition.command)
        return handler(recognition)

    def _voice_status(self, *, recognized_command: str | None = None) -> OperationResult:
        session = self._session_repository.get_active_session()
        if session is None:
            return OperationResult.error("No active reading session exists.")

        document = self._document_repository.get_document(session.document_id)
        if document is None:
            return OperationResult.error("The active session references a missing document.")

        playback_snapshot = self._synchronize_session_playback(session)
        section, chunk = self._current_location(document, session)
        session.last_command = "status"
        session.last_command_source = "voice"
        session.last_recognized_command = recognized_command
        self._session_repository.save_session(session)
        return OperationResult.ok(
            "Active reading status reported.",
            data={
                "session": session,
                "position": {
                    "anchor": session.position.anchor,
                    "section_title": section.title,
                    "chunk_text": chunk.text,
                },
                "progress": self._reading_progress(document, session),
                "playback": playback_snapshot,
            },
        )

    def _handle_help(self, *, recognized_command: str | None = None) -> OperationResult:
        """Return available voice commands in the current language."""

        session = self._session_repository.get_active_session()
        if session is None:
            return OperationResult.error("No active reading session exists.")

        session.last_command = "help"
        session.last_command_source = "voice"
        session.last_recognized_command = recognized_command
        session.touch()
        self._session_repository.save_session(session)
        available_commands = {
            intent.value: list(phrases)
            for intent, phrases in self._command_lexicon.phrases_by_intent.items()
        }
        logger.info("Help requested: returning %d command intents", len(available_commands))
        return OperationResult.ok(
            "Available voice commands reported.",
            data={
                "session": session,
                "language": self._command_lexicon.language,
                "available_commands": available_commands,
            },
        )

    def _start_current_chunk(
        self,
        session: ReadingSession,
        document: Document,
        *,
        command_name: str,
        command_source: str,
        recognized_command: str | None,
        message: str,
        publish_started: bool,
    ) -> OperationResult:
        section, chunk = self._current_location(document, session)
        return self._start_playback_for_text(
            session,
            document,
            section=section,
            chunk=chunk,
            text_to_synthesize=chunk.text,
            playback_scope="current-chunk",
            command_name=command_name,
            command_source=command_source,
            recognized_command=recognized_command,
            message=message,
            publish_started=publish_started,
        )

    def _start_playback_for_text(
        self,
        session: ReadingSession,
        document: Document,
        *,
        section: DocumentSection,
        chunk: DocumentChunk,
        text_to_synthesize: str,
        playback_scope: str,
        command_name: str,
        command_source: str,
        recognized_command: str | None,
        message: str,
        publish_started: bool,
    ) -> OperationResult:
        self._wait_for_pre_synthesis()
        synthesis_result = self._speech_synthesizer.synthesize(
            SynthesisRequest(text=text_to_synthesize, voice=self._default_voice)
        )
        playback_snapshot = self._playback_engine.start(
            document,
            session.position,
            synthesis=synthesis_result,
        )
        self._pre_synthesize_next_chunk(document, session)
        self._apply_playback_snapshot(session, playback_snapshot)
        session.voice = synthesis_result.voice
        session.tts_provider = synthesis_result.provider_name
        self._mark_command(
            session,
            command_name=command_name,
            command_source=command_source,
            recognized_command=recognized_command,
        )
        self._session_repository.save_session(session)
        if publish_started:
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
        elif command_name == "resume":
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
        progress = self._reading_progress(document, session)
        self._publish(
            EventName.READING_PROGRESSED,
            session_id=session.session_id,
            document_id=session.document_id,
            section_index=session.position.section_index,
            chunk_index=session.position.chunk_index,
            anchor=session.position.anchor,
            section_count=progress["section_count"],
            section_chunk_count=progress["section_chunk_count"],
            chunks_read=progress["chunks_read"],
            total_chunks=progress["total_chunks"],
        )
        return OperationResult.ok(
            message,
            data={
                "session": session,
                "document_title": document.title,
                "section_title": section.title,
                "current_chunk": chunk.text,
                "playback_scope": playback_scope,
                "rendered_char_count": len(text_to_synthesize),
                "synthesis": synthesis_result,
                "playback": playback_snapshot,
                "progress": progress,
            },
        )

    def _current_location(
        self,
        document: Document,
        session: ReadingSession,
    ) -> tuple[DocumentSection, DocumentChunk]:
        section = document.get_section(session.position.section_index)
        return section, section.get_chunk(session.position.chunk_index)

    @staticmethod
    def _reading_progress(
        document: Document,
        session: ReadingSession,
    ) -> dict[str, object]:
        """Compute reading progress fractions for the current position."""

        section_index = session.position.section_index
        chunk_index = session.position.chunk_index
        section_count = document.chapter_count
        current_section = document.get_section(section_index)
        section_chunk_count = current_section.chunk_count
        chunks_before = sum(
            document.get_section(i).chunk_count for i in range(section_index)
        )
        chunks_read = chunks_before + chunk_index
        total_chunks = document.total_chunk_count
        return {
            "section_index": section_index,
            "section_count": section_count,
            "chunk_index": chunk_index,
            "section_chunk_count": section_chunk_count,
            "chunks_read": chunks_read,
            "total_chunks": total_chunks,
        }

    def _pre_synthesize_next_chunk(
        self, document: Document, session: ReadingSession
    ) -> None:
        """Pre-synthesize the next chunk's audio in a background thread.

        When the current chunk finishes, the next one's WAV is already cached
        on disk, eliminating the inter-chunk gap caused by TTS latency.
        """

        section_index = session.position.section_index
        chunk_index = session.position.chunk_index
        current_section = document.get_section(section_index)
        next_chunk_idx = chunk_index + 1

        if next_chunk_idx < current_section.chunk_count:
            text = current_section.get_chunk(next_chunk_idx).text
        elif section_index + 1 < document.chapter_count:
            next_section = document.get_section(section_index + 1)
            if next_section.chunk_count > 0:
                text = next_section.get_chunk(0).text
            else:
                return
        else:
            return  # end of document

        if not text.strip():
            return

        voice = self._default_voice

        def _synthesize() -> None:
            try:
                self._speech_synthesizer.synthesize(
                    SynthesisRequest(text=text, voice=voice)
                )
                logger.debug("Pre-synthesized next chunk (%d chars)", len(text))
            except Exception:
                logger.debug("Pre-synthesis failed (non-fatal)", exc_info=True)

        thread = threading.Thread(target=_synthesize, daemon=True)
        thread.start()
        self._pre_synth_thread = thread

    def _wait_for_pre_synthesis(self) -> None:
        """Wait for any pending background pre-synthesis to finish."""

        if self._pre_synth_thread is not None and self._pre_synth_thread.is_alive():
            self._pre_synth_thread.join(timeout=10.0)
        self._pre_synth_thread = None

    def _mark_command(
        self,
        session: ReadingSession,
        *,
        command_name: str,
        command_source: str,
        recognized_command: str | None,
    ) -> None:
        session.last_command = command_name
        session.last_command_source = command_source
        if recognized_command is not None:
            session.last_recognized_command = recognized_command
        session.command_stt_provider = self._command_provider_name
        session.playback_provider = session.playback_provider or self._playback_provider_name
        session.tts_provider = session.tts_provider or self._tts_provider_name
        session.voice = session.voice or self._default_voice
        session.touch()

    def _apply_playback_snapshot(
        self,
        session: ReadingSession,
        snapshot: PlaybackSnapshot,
    ) -> None:
        session.playback_state = snapshot.state
        session.audio_reference = snapshot.audio_reference
        session.playback_process_id = snapshot.process_id
        session.playback_provider = snapshot.provider_name or self._playback_provider_name
        session.command_stt_provider = self._command_provider_name
        session.touch()

    def _playback_snapshot_for_session(self, session: ReadingSession) -> PlaybackSnapshot:
        return PlaybackSnapshot(
            state=session.playback_state,
            last_action=session.last_command or "session-state",
            document_id=session.document_id,
            anchor=session.position.anchor,
            progress_units=0,
            audio_reference=session.audio_reference,
            provider_name=session.playback_provider or self._playback_provider_name,
            process_id=session.playback_process_id,
        )

    def _synchronize_session_playback(self, session: ReadingSession) -> PlaybackSnapshot:
        self._playback_engine.hydrate(self._playback_snapshot_for_session(session))
        snapshot = self._playback_engine.snapshot()
        self._apply_playback_snapshot(session, snapshot)
        self._session_repository.save_session(session)
        return snapshot

    def _play_message(self, *, playback_scope: str) -> str:
        if self._tts_provider_name == "fake-tts":
            return "Reading session is active. Audio output is still backed by a fake adapter."
        return "Reading session is active with local audio playback."

    def _publish(self, event_name: EventName, **payload: object) -> None:
        self._event_publisher.publish(DomainEvent(name=event_name, payload=payload))
