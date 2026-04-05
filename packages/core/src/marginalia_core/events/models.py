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

    DOCUMENT_INGESTED = "document.ingested"
    READING_STARTED = "reading.started"
    READING_PAUSED = "reading.paused"
    READING_RESUMED = "reading.resumed"
    READING_PROGRESSED = "reading.progressed"
    CHAPTER_RESTARTED = "chapter.restarted"
    CHAPTER_ADVANCED = "chapter.advanced"
    NOTE_RECORDING_STARTED = "note.recording.started"
    NOTE_RECORDING_STOPPED = "note.recording.stopped"
    NOTE_SAVED = "note.saved"
    REWRITE_REQUESTED = "rewrite.requested"
    REWRITE_COMPLETED = "rewrite.completed"
    SUMMARY_REQUESTED = "summary.requested"
    SUMMARY_COMPLETED = "summary.completed"
    PLAYBACK_STARTED = "playback.started"
    PLAYBACK_PAUSED = "playback.paused"
    PLAYBACK_RESUMED = "playback.resumed"
    PLAYBACK_STOPPED = "playback.stopped"
    READING_COMPLETED = "reading.completed"
    COMMAND_DISPATCHED = "command.dispatched"
    ERROR_RAISED = "error.raised"


@dataclass(frozen=True, slots=True)
class DomainEvent:
    """Simple event envelope for local in-process subscribers."""

    name: EventName
    payload: dict[str, object]
    event_id: str = field(default_factory=lambda: str(uuid4()))
    occurred_at: datetime = field(default_factory=_utc_now)
