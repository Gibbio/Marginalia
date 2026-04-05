"""Search query and result models."""

from __future__ import annotations

from dataclasses import dataclass


@dataclass(frozen=True, slots=True)
class SearchQuery:
    """Simple local search request."""

    text: str
    document_id: str | None = None
    limit: int = 10

    @property
    def normalized_text(self) -> str:
        return self.text.strip()


@dataclass(frozen=True, slots=True)
class SearchResult:
    """Ranked search hit inside documents or notes."""

    entity_kind: str
    entity_id: str
    score: float
    excerpt: str
    anchor: str
