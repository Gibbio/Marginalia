"""Frontend-facing event envelopes."""

from __future__ import annotations

from dataclasses import dataclass, field
from datetime import UTC, datetime
from enum import Enum
from uuid import uuid4

from marginalia_core.application.frontend.envelopes import FRONTEND_PROTOCOL_VERSION


def _utc_now() -> datetime:
    return datetime.now(UTC)


class FrontendEventName(str, Enum):
    """Stable event names exposed at the frontend boundary."""

    NOTE_SAVED = "note_saved"
    PLAYBACK_STATE_CHANGED = "playback_state_changed"
    PROVIDER_WARNING_EMITTED = "provider_warning_emitted"
    RUNTIME_FAILED = "runtime_failed"
    RUNTIME_STOPPED = "runtime_stopped"
    SESSION_PROGRESS_UPDATED = "session_progress_updated"
    SESSION_STARTED = "session_started"


@dataclass(frozen=True, slots=True)
class FrontendEvent:
    """Frontend-facing event envelope."""

    name: str
    payload: dict[str, object]
    event_id: str = field(default_factory=lambda: str(uuid4()))
    occurred_at: datetime = field(default_factory=_utc_now)
    protocol_version: int = FRONTEND_PROTOCOL_VERSION
