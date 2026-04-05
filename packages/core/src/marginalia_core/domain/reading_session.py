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
    audio_reference: str | None = None
    playback_process_id: int | None = None
    updated_at: datetime = field(default_factory=_utc_now)

    def touch(self) -> None:
        self.updated_at = _utc_now()
