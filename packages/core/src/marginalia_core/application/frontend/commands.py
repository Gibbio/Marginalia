"""Stable frontend command names."""

from __future__ import annotations

from enum import Enum


class FrontendCommandName(str, Enum):
    """Commands that mutate backend state."""

    CREATE_NOTE = "create_note"
    INGEST_DOCUMENT = "ingest_document"
    NEXT_CHUNK = "next_chunk"
    NEXT_CHAPTER = "next_chapter"
    PAUSE_SESSION = "pause_session"
    PREVIOUS_CHAPTER = "previous_chapter"
    PREVIOUS_CHUNK = "previous_chunk"
    REPEAT_CHUNK = "repeat_chunk"
    RESTART_CHAPTER = "restart_chapter"
    RESUME_SESSION = "resume_session"
    START_SESSION = "start_session"
    STOP_SESSION = "stop_session"
