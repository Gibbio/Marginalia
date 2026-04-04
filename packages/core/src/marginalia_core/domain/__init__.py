"""Domain models for Marginalia."""

from marginalia_core.domain.document import (
    Document,
    DocumentChunk,
    DocumentSection,
    build_document_outline,
)
from marginalia_core.domain.note import VoiceNote
from marginalia_core.domain.reading_session import PlaybackState, ReaderState, ReadingPosition, ReadingSession
from marginalia_core.domain.rewrite import RewriteDraft, RewriteStatus
from marginalia_core.domain.search import SearchResult
from marginalia_core.domain.summary import SummaryRequest, SummaryResult

__all__ = [
    "Document",
    "DocumentChunk",
    "DocumentSection",
    "PlaybackState",
    "ReaderState",
    "ReadingPosition",
    "ReadingSession",
    "RewriteDraft",
    "RewriteStatus",
    "SearchResult",
    "SummaryRequest",
    "SummaryResult",
    "VoiceNote",
    "build_document_outline",
]
