"""Reading session and state models."""

from __future__ import annotations

from dataclasses import dataclass, field
from datetime import UTC, datetime
from enum import Enum


def _utc_now() -> datetime:
    return datetime.now(UTC)


class PlaybackState(str, Enum):
    """Low-level playback status."""

    STOPPED = "stopped"
    PLAYING = "playing"
    PAUSED = "paused"


class ReaderState(str, Enum):
    """High-level lifecycle state for the local reading engine."""

    IDLE = "IDLE"
    READING = "READING"
    PAUSED = "PAUSED"
    LISTENING_FOR_COMMAND = "LISTENING_FOR_COMMAND"
    RECORDING_NOTE = "RECORDING_NOTE"
    PROCESSING_REWRITE = "PROCESSING_REWRITE"
    READING_REWRITE = "READING_REWRITE"
    ERROR = "ERROR"


@dataclass(slots=True)
class ReadingPosition:
    """Anchor inside a document."""

    section_index: int = 0
    chunk_index: int = 0
    char_offset: int = 0

    @property
    def anchor(self) -> str:
        return f"section:{self.section_index}/chunk:{self.chunk_index}"

    @classmethod
    def from_anchor(cls, anchor: str) -> ReadingPosition:
        """Parse a ``section:N/chunk:M`` anchor string back into a position."""

        section_index = 0
        chunk_index = 0
        for item in anchor.split("/"):
            key, _, raw_value = item.partition(":")
            if key == "section":
                section_index = int(raw_value)
            elif key == "chunk":
                chunk_index = int(raw_value)
        return cls(section_index=section_index, chunk_index=chunk_index)


@dataclass(slots=True)
class ReadingSession:
    """Mutable session record for CLI-driven local reading."""

    session_id: str
    document_id: str
    state: ReaderState = ReaderState.IDLE
    playback_state: PlaybackState = PlaybackState.STOPPED
    position: ReadingPosition = field(default_factory=ReadingPosition)
    active_note_id: str | None = None
    last_command: str | None = None
    last_command_source: str | None = None
    last_recognized_command: str | None = None
    voice: str | None = None
    tts_provider: str | None = None
    command_stt_provider: str | None = None
    playback_provider: str | None = None
    command_listening_active: bool = False
    command_language: str | None = None
    audio_reference: str | None = None
    playback_process_id: int | None = None
    runtime_process_id: int | None = None
    runtime_status: str | None = None
    runtime_error: str | None = None
    startup_cleanup_summary: str | None = None
    is_active: bool = True
    updated_at: datetime = field(default_factory=_utc_now)

    def touch(self) -> None:
        self.updated_at = _utc_now()
