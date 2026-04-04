"""Rewrite draft domain models."""

from __future__ import annotations

from dataclasses import dataclass, field
from datetime import UTC, datetime
from enum import Enum


def _utc_now() -> datetime:
    return datetime.now(UTC)


class RewriteStatus(str, Enum):
    """Lifecycle state for a rewrite draft."""

    REQUESTED = "requested"
    GENERATED = "generated"
    DISMISSED = "dismissed"


@dataclass(frozen=True, slots=True)
class RewriteDraft:
    """Draft rewrite for a section informed by anchored notes."""

    draft_id: str
    document_id: str
    section_index: int
    source_excerpt: str
    note_transcripts: tuple[str, ...]
    rewritten_text: str
    status: RewriteStatus = RewriteStatus.REQUESTED
    created_at: datetime = field(default_factory=_utc_now)
