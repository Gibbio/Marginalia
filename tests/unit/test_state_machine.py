"""State machine tests."""

from __future__ import annotations

import pytest

from marginalia_core.application.state_machine import (
    InvalidTransitionError,
    ReaderStateMachine,
    playback_state_for,
)
from marginalia_core.domain.reading_session import PlaybackState, ReaderState, ReadingSession


def test_reader_state_machine_accepts_pause_transition() -> None:
    session = ReadingSession(session_id="session-1", document_id="doc-1", state=ReaderState.READING)

    ReaderStateMachine().transition(session, ReaderState.PAUSED)

    assert session.state is ReaderState.PAUSED


def test_reader_state_machine_rejects_invalid_transition() -> None:
    session = ReadingSession(session_id="session-1", document_id="doc-1", state=ReaderState.IDLE)

    with pytest.raises(InvalidTransitionError):
        ReaderStateMachine().transition(session, ReaderState.RECORDING_NOTE)


def test_reader_state_machine_sets_paused_playback_for_rewrite_processing() -> None:
    session = ReadingSession(session_id="session-1", document_id="doc-1", state=ReaderState.PAUSED)

    ReaderStateMachine().transition(session, ReaderState.PROCESSING_REWRITE)

    assert session.state is ReaderState.PROCESSING_REWRITE
    assert session.playback_state is PlaybackState.PAUSED


def test_playback_state_for_reading_rewrite_is_playing() -> None:
    assert playback_state_for(ReaderState.READING_REWRITE) is PlaybackState.PLAYING
