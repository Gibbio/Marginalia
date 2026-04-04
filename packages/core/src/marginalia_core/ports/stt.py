"""Speech-to-text ports."""

from __future__ import annotations

from typing import Protocol


class CommandRecognizer(Protocol):
    """Recognize short command-style utterances."""

    def listen_for_command(self) -> str | None:
        """Return the next recognized command if one is available."""
        ...


class DictationTranscriber(Protocol):
    """Transcribe longer dictated note content."""

    def transcribe(self) -> str:
        """Return a transcript of the latest dictation input."""
        ...
