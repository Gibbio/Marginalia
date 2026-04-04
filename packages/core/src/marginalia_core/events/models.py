"""Standardized event names and payload envelope."""

from __future__ import annotations

from dataclasses import dataclass, field
from datetime import UTC, datetime
from enum import Enum
from uuid import uuid4


def _utc_now() -> datetime:
    return datetime.now(UTC)


class EventName(str, Enum):
    """Stable event names used by the local core."""

    READING_STARTED = "reader.session.started"
    PLAYBACK_PAUSED = "reader.playback.paused"
    PLAYBACK_RESUMED = "reader.playback.resumed"
    READING_POSITION_CHANGED = "reader.position.changed"
    NOTE_RECORDING_STARTED = "note.recording.started"
    NOTE_SAVED = "note.saved"
    REWRITE_COMPLETED = "rewrite.completed"
    SUMMARY_COMPLETED = "summary.completed"
    SYSTEM_ERROR = "system.error"


@dataclass(frozen=True, slots=True)
class DomainEvent:
    """Simple event envelope for local in-process subscribers."""

    name: EventName
    payload: dict[str, object]
    event_id: str = field(default_factory=lambda: str(uuid4()))
    occurred_at: datetime = field(default_factory=_utc_now)
