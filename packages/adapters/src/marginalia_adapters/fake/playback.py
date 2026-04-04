"""Fake playback engine."""

from __future__ import annotations

from marginalia_core.domain.document import Document
from marginalia_core.domain.reading_session import PlaybackState, ReadingPosition


class FakePlaybackEngine:
    """Track playback commands without attempting real audio output."""

    def __init__(self) -> None:
        self.state = PlaybackState.STOPPED
        self.last_document_id: str | None = None
        self.last_position = ReadingPosition()

    def start(self, document: Document, position: ReadingPosition) -> None:
        self.last_document_id = document.document_id
        self.last_position = position
        self.state = PlaybackState.PLAYING

    def pause(self) -> None:
        self.state = PlaybackState.PAUSED

    def resume(self) -> None:
        self.state = PlaybackState.PLAYING

    def stop(self) -> None:
        self.state = PlaybackState.STOPPED

    def seek(self, position: ReadingPosition) -> None:
        self.last_position = position
