"""Reader state machine and allowed transitions."""

from __future__ import annotations

from marginalia_core.domain.reading_session import PlaybackState, ReaderState, ReadingSession


class InvalidTransitionError(ValueError):
    """Raised when a state transition violates the current lifecycle graph."""


ALLOWED_TRANSITIONS: dict[ReaderState, set[ReaderState]] = {
    ReaderState.IDLE: {ReaderState.READING, ReaderState.ERROR},
    ReaderState.READING: {
        ReaderState.PAUSED,
        ReaderState.LISTENING_FOR_COMMAND,
        ReaderState.RECORDING_NOTE,
        ReaderState.PROCESSING_REWRITE,
        ReaderState.ERROR,
    },
    ReaderState.PAUSED: {
        ReaderState.READING,
        ReaderState.LISTENING_FOR_COMMAND,
        ReaderState.RECORDING_NOTE,
        ReaderState.PROCESSING_REWRITE,
        ReaderState.ERROR,
    },
    ReaderState.LISTENING_FOR_COMMAND: {
        ReaderState.READING,
        ReaderState.PAUSED,
        ReaderState.RECORDING_NOTE,
        ReaderState.ERROR,
    },
    ReaderState.RECORDING_NOTE: {
        ReaderState.PAUSED,
        ReaderState.READING,
        ReaderState.ERROR,
    },
    ReaderState.PROCESSING_REWRITE: {
        ReaderState.READING_REWRITE,
        ReaderState.PAUSED,
        ReaderState.ERROR,
    },
    ReaderState.READING_REWRITE: {
        ReaderState.PAUSED,
        ReaderState.READING,
        ReaderState.ERROR,
    },
    ReaderState.ERROR: {ReaderState.IDLE},
}


def playback_state_for(reader_state: ReaderState) -> PlaybackState:
    """Map the high-level state to an expected playback status."""

    if reader_state in {ReaderState.READING, ReaderState.READING_REWRITE}:
        return PlaybackState.PLAYING
    if reader_state is ReaderState.IDLE:
        return PlaybackState.STOPPED
    return PlaybackState.PAUSED


class ReaderStateMachine:
    """State transition helper for application services."""

    def transition(self, session: ReadingSession, target: ReaderState) -> ReadingSession:
        if session.state is target:
            return session

        allowed_targets = ALLOWED_TRANSITIONS.get(session.state, set())
        if target not in allowed_targets:
            raise InvalidTransitionError(
                f"Cannot move from {session.state.value} to {target.value}."
            )

        session.state = target
        session.playback_state = playback_state_for(target)
        session.touch()
        return session
