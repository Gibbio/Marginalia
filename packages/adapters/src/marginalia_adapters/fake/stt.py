"""Fake STT adapters."""

from __future__ import annotations

from collections import deque
from collections.abc import Sequence


class FakeCommandRecognizer:
    """FIFO command recognizer for local testing and smoke flows."""

    def __init__(self, commands: Sequence[str] | None = None) -> None:
        self._commands = deque(commands or [])

    def listen_for_command(self) -> str | None:
        if not self._commands:
            return None
        return self._commands.popleft()


class FakeDictationTranscriber:
    """Deterministic dictation placeholder."""

    def __init__(self, transcript: str = "Placeholder dictated note.") -> None:
        self._transcript = transcript

    def transcribe(self) -> str:
        return self._transcript

    def set_transcript(self, transcript: str) -> None:
        self._transcript = transcript
