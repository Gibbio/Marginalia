"""Stable snapshot DTOs exposed to frontends."""

from __future__ import annotations

from dataclasses import dataclass
from datetime import datetime


@dataclass(frozen=True, slots=True)
class DocumentListItem:
    """Metadata summary for a stored document."""

    chapter_count: int
    chunk_count: int
    document_id: str
    title: str


@dataclass(frozen=True, slots=True)
class AppSnapshot:
    """High-level backend state for initial client render."""

    active_session_id: str | None
    document_count: int
    latest_document_id: str | None
    playback_state: str | None
    runtime_status: str | None
    state: str


@dataclass(frozen=True, slots=True)
class SessionSnapshot:
    """Reading-session projection suitable for any frontend."""

    anchor: str
    chunk_index: int
    chunk_text: str
    command_listening_active: bool
    command_stt_provider: str | None
    document_id: str
    notes_count: int
    playback_provider: str | None
    playback_state: str
    section_count: int
    section_index: int
    section_title: str
    session_id: str
    state: str
    tts_provider: str | None
    voice: str | None


@dataclass(frozen=True, slots=True)
class DocumentChunkView:
    """Chunk projection for frontend rendering."""

    anchor: str
    char_end: int
    char_start: int
    index: int
    is_active: bool
    is_read: bool
    text: str


@dataclass(frozen=True, slots=True)
class DocumentSectionView:
    """Section projection for frontend rendering."""

    chunk_count: int
    chunks: tuple[DocumentChunkView, ...]
    index: int
    source_anchor: str | None
    title: str


@dataclass(frozen=True, slots=True)
class DocumentView:
    """Document projection with sections and chunks for UI clients."""

    active_chunk_index: int | None
    active_section_index: int | None
    chapter_count: int
    chunk_count: int
    document_id: str
    sections: tuple[DocumentSectionView, ...]
    source_path: str
    title: str


@dataclass(frozen=True, slots=True)
class NoteView:
    """Stable note projection for frontend clients."""

    anchor: str
    created_at: datetime
    document_id: str
    language: str
    note_id: str
    section_index: int
    chunk_index: int
    session_id: str
    transcript: str
    transcription_provider: str


@dataclass(frozen=True, slots=True)
class NotesSnapshot:
    """Collection of notes for a document."""

    document_id: str
    notes: tuple[NoteView, ...]


@dataclass(frozen=True, slots=True)
class SearchResultView:
    """Stable search result projection for frontend clients."""

    anchor: str
    entity_id: str
    entity_kind: str
    excerpt: str
    score: float


@dataclass(frozen=True, slots=True)
class SearchResultsSnapshot:
    """Collection of search hits for a given query."""

    query: str
    results: tuple[SearchResultView, ...]
