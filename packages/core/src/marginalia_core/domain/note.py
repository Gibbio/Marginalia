"""Voice note domain models."""

from __future__ import annotations

from dataclasses import dataclass, field
from datetime import UTC, datetime
from pathlib import Path

from marginalia_core.domain.reading_session import ReadingPosition


def _utc_now() -> datetime:
    return datetime.now(UTC)


@dataclass(frozen=True, slots=True)
class VoiceNote:
    """A note anchored to a specific reading position."""

    note_id: str
    session_id: str
    document_id: str
    position: ReadingPosition
    transcript: str
    raw_audio_path: Path | None = None
    created_at: datetime = field(default_factory=_utc_now)

    @property
    def anchor(self) -> str:
        return self.position.anchor
