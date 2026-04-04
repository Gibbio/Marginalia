"""Search result models."""

from __future__ import annotations

from dataclasses import dataclass


@dataclass(frozen=True, slots=True)
class SearchResult:
    """Ranked search hit inside documents or notes."""

    entity_kind: str
    entity_id: str
    score: float
    excerpt: str
    anchor: str
