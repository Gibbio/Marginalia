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
        self.last_action = "stopped"

    def start(self, document: Document, position: ReadingPosition) -> None:
        self.last_document_id = document.document_id
        self.last_position = position
        self.state = PlaybackState.PLAYING
        self.last_action = "start"

    def pause(self) -> None:
        self.state = PlaybackState.PAUSED
        self.last_action = "pause"

    def resume(self) -> None:
        self.state = PlaybackState.PLAYING
        self.last_action = "resume"

    def stop(self) -> None:
        self.state = PlaybackState.STOPPED
        self.last_action = "stop"

    def seek(self, position: ReadingPosition) -> None:
        self.last_position = position
        self.last_action = "seek"

    def snapshot(self) -> dict[str, object]:
        return {
            "state": self.state.value,
            "last_action": self.last_action,
            "last_document_id": self.last_document_id,
            "anchor": self.last_position.anchor,
        }
