"""State machine tests."""

from __future__ import annotations

import pytest

from marginalia_core.application.state_machine import InvalidTransitionError, ReaderStateMachine
from marginalia_core.domain.reading_session import ReaderState, ReadingSession


def test_reader_state_machine_accepts_pause_transition() -> None:
    session = ReadingSession(session_id="session-1", document_id="doc-1", state=ReaderState.READING)

    ReaderStateMachine().transition(session, ReaderState.PAUSED)

    assert session.state is ReaderState.PAUSED


def test_reader_state_machine_rejects_invalid_transition() -> None:
    session = ReadingSession(session_id="session-1", document_id="doc-1", state=ReaderState.IDLE)

    with pytest.raises(InvalidTransitionError):
        ReaderStateMachine().transition(session, ReaderState.RECORDING_NOTE)
