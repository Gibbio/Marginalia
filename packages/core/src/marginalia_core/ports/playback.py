"""Audio playback ports."""

from __future__ import annotations

from typing import Protocol

from marginalia_core.domain.document import Document
from marginalia_core.domain.reading_session import ReadingPosition


class PlaybackEngine(Protocol):
    """Output audio and manage seek/pause/resume semantics."""

    def start(self, document: Document, position: ReadingPosition) -> None:
        """Start playback for a document position."""
        ...

    def pause(self) -> None:
        """Pause the currently active playback session."""
        ...

    def resume(self) -> None:
        """Resume the currently active playback session."""
        ...

    def stop(self) -> None:
        """Stop playback entirely."""
        ...

    def seek(self, position: ReadingPosition) -> None:
        """Move playback to a new document position."""
        ...
