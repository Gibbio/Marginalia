"""Text-to-speech ports."""

from __future__ import annotations

from typing import Protocol


class SpeechSynthesizer(Protocol):
    """Convert text to an audio payload."""

    def synthesize(self, text: str, *, voice: str | None = None) -> bytes:
        """Return synthetic audio bytes for the given text."""
        ...
