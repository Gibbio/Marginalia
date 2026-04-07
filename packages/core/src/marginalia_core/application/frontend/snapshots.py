"""Stable snapshot DTOs exposed to frontends."""

from __future__ import annotations

from dataclasses import dataclass


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
