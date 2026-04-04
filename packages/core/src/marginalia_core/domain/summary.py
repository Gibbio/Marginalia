"""Topic summarization request and result models."""

from __future__ import annotations

from dataclasses import dataclass, field
from datetime import UTC, datetime


def _utc_now() -> datetime:
    return datetime.now(UTC)


@dataclass(frozen=True, slots=True)
class SummaryRequest:
    """User request to summarize a topic inside the local corpus."""

    topic: str
    document_id: str | None = None
    requested_at: datetime = field(default_factory=_utc_now)


@dataclass(frozen=True, slots=True)
class SummaryResult:
    """Placeholder summary output."""

    topic: str
    summary_text: str
    matched_document_ids: tuple[str, ...] = ()
    generated_at: datetime = field(default_factory=_utc_now)
