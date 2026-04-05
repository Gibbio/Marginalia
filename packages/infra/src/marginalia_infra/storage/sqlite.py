"""SQLite-backed repositories and schema bootstrap."""

from __future__ import annotations

import json
import sqlite3
from collections.abc import Sequence
from datetime import datetime
from pathlib import Path
from typing import Any

from marginalia_core.domain.document import Document, DocumentChunk, DocumentSection
from marginalia_core.domain.note import VoiceNote
from marginalia_core.domain.reading_session import (
    PlaybackState,
    ReaderState,
    ReadingPosition,
    ReadingSession,
)
from marginalia_core.domain.rewrite import RewriteDraft, RewriteStatus
from marginalia_core.domain.search import SearchQuery, SearchResult

SCHEMA_VERSION = "1"

SCHEMA_SQL = """
CREATE TABLE IF NOT EXISTS schema_metadata (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS documents (
    document_id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    source_path TEXT NOT NULL,
    imported_at TEXT NOT NULL,
    outline_json TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS sessions (
    session_id TEXT PRIMARY KEY,
    document_id TEXT NOT NULL,
    state TEXT NOT NULL,
    playback_state TEXT NOT NULL,
    section_index INTEGER NOT NULL,
    chunk_index INTEGER NOT NULL,
    char_offset INTEGER NOT NULL,
    active_note_id TEXT,
    last_command TEXT,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS notes (
    note_id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL,
    document_id TEXT NOT NULL,
    section_index INTEGER NOT NULL,
    chunk_index INTEGER NOT NULL,
    char_offset INTEGER NOT NULL,
    transcript TEXT NOT NULL,
    raw_audio_path TEXT,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS drafts (
    draft_id TEXT PRIMARY KEY,
    document_id TEXT NOT NULL,
    section_index INTEGER NOT NULL,
    source_excerpt TEXT NOT NULL,
    note_transcripts_json TEXT NOT NULL,
    rewritten_text TEXT NOT NULL,
    status TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_documents_imported_at ON documents(imported_at DESC);
CREATE INDEX IF NOT EXISTS idx_sessions_updated_at ON sessions(updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_notes_document_id ON notes(document_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_drafts_document_id ON drafts(document_id, created_at DESC);
"""


class SQLiteDatabase:
    """Single SQLite database handle and schema bootstrap utility."""

    def __init__(self, database_path: Path) -> None:
        self._database_path = database_path

    @property
    def database_path(self) -> Path:
        return self._database_path

    def connect(self) -> sqlite3.Connection:
        self._database_path.parent.mkdir(parents=True, exist_ok=True)
        connection = sqlite3.connect(self._database_path)
        connection.row_factory = sqlite3.Row
        return connection

    def initialize(self) -> None:
        with self.connect() as connection:
            connection.executescript(SCHEMA_SQL)
            connection.execute(
                """
                INSERT INTO schema_metadata(key, value)
                VALUES('schema_version', ?)
                ON CONFLICT(key) DO UPDATE SET value = excluded.value
                """,
                (SCHEMA_VERSION,),
            )

    def schema_version(self) -> str:
        self.initialize()
        with self.connect() as connection:
            row = connection.execute(
                "SELECT value FROM schema_metadata WHERE key = 'schema_version'"
            ).fetchone()
        return str(row["value"]) if row is not None else "unknown"

    def table_names(self) -> tuple[str, ...]:
        self.initialize()
        with self.connect() as connection:
            rows = connection.execute(
                """
                SELECT name
                FROM sqlite_master
                WHERE type = 'table' AND name NOT LIKE 'sqlite_%'
                ORDER BY name ASC
                """
            ).fetchall()
        return tuple(str(row["name"]) for row in rows)

    def health_report(self) -> dict[str, Any]:
        return {
            "database_path": self.database_path,
            "database_exists": self.database_path.exists(),
            "schema_version": self.schema_version(),
            "tables": self.table_names(),
        }


class _SQLiteRepository:
    def __init__(self, database: Path | SQLiteDatabase) -> None:
        self._database = (
            database if isinstance(database, SQLiteDatabase) else SQLiteDatabase(database)
        )

    def _connect(self) -> sqlite3.Connection:
        return self._database.connect()

    def ensure_schema(self) -> None:
        self._database.initialize()


class SQLiteDocumentRepository(_SQLiteRepository):
    """SQLite storage for documents."""

    def save_document(self, document: Document) -> None:
        payload = json.dumps(_document_to_payload(document))
        with self._connect() as connection:
            connection.execute(
                """
                INSERT INTO documents(document_id, title, source_path, imported_at, outline_json)
                VALUES(?, ?, ?, ?, ?)
                ON CONFLICT(document_id) DO UPDATE SET
                    title = excluded.title,
                    source_path = excluded.source_path,
                    imported_at = excluded.imported_at,
                    outline_json = excluded.outline_json
                """,
                (
                    document.document_id,
                    document.title,
                    str(document.source_path),
                    document.imported_at.isoformat(),
                    payload,
                ),
            )

    def get_document(self, document_id: str) -> Document | None:
        with self._connect() as connection:
            row = connection.execute(
                "SELECT outline_json FROM documents WHERE document_id = ?",
                (document_id,),
            ).fetchone()
        if row is None:
            return None
        return _document_from_payload(json.loads(str(row["outline_json"])))

    def list_documents(self) -> Sequence[Document]:
        with self._connect() as connection:
            rows = connection.execute(
                "SELECT outline_json FROM documents ORDER BY imported_at DESC"
            ).fetchall()
        return tuple(_document_from_payload(json.loads(str(row["outline_json"]))) for row in rows)

    def search_documents(self, query: SearchQuery | str) -> Sequence[SearchResult]:
        search_query = _normalize_search_query(query)
        needle = f"%{search_query.normalized_text.lower()}%"

        sql = """
            SELECT document_id, outline_json
            FROM documents
            WHERE (LOWER(title) LIKE ? OR LOWER(outline_json) LIKE ?)
        """
        params: list[object] = [needle, needle]
        if search_query.document_id is not None:
            sql += " AND document_id = ?"
            params.append(search_query.document_id)
        sql += " ORDER BY imported_at DESC LIMIT ?"
        params.append(max(search_query.limit, 1))

        with self._connect() as connection:
            rows = connection.execute(sql, tuple(params)).fetchall()

        results: list[SearchResult] = []
        for row in rows:
            payload = json.loads(str(row["outline_json"]))
            excerpt = _excerpt_from_document_payload(payload, search_query.normalized_text)
            results.append(
                SearchResult(
                    entity_kind="document",
                    entity_id=str(row["document_id"]),
                    score=1.0,
                    excerpt=excerpt,
                    anchor="document",
                )
            )
        return tuple(results)


class SQLiteSessionRepository(_SQLiteRepository):
    """SQLite storage for reading sessions."""

    def save_session(self, session: ReadingSession) -> None:
        with self._connect() as connection:
            connection.execute(
                """
                INSERT INTO sessions(
                    session_id,
                    document_id,
                    state,
                    playback_state,
                    section_index,
                    chunk_index,
                    char_offset,
                    active_note_id,
                    last_command,
                    updated_at
                )
                VALUES(?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                ON CONFLICT(session_id) DO UPDATE SET
                    document_id = excluded.document_id,
                    state = excluded.state,
                    playback_state = excluded.playback_state,
                    section_index = excluded.section_index,
                    chunk_index = excluded.chunk_index,
                    char_offset = excluded.char_offset,
                    active_note_id = excluded.active_note_id,
                    last_command = excluded.last_command,
                    updated_at = excluded.updated_at
                """,
                (
                    session.session_id,
                    session.document_id,
                    session.state.value,
                    session.playback_state.value,
                    session.position.section_index,
                    session.position.chunk_index,
                    session.position.char_offset,
                    session.active_note_id,
                    session.last_command,
                    session.updated_at.isoformat(),
                ),
            )

    def get_active_session(self) -> ReadingSession | None:
        with self._connect() as connection:
            row = connection.execute(
                """
                SELECT *
                FROM sessions
                ORDER BY updated_at DESC
                LIMIT 1
                """
            ).fetchone()
        if row is None:
            return None
        return ReadingSession(
            session_id=str(row["session_id"]),
            document_id=str(row["document_id"]),
            state=ReaderState(str(row["state"])),
            playback_state=PlaybackState(str(row["playback_state"])),
            position=ReadingPosition(
                section_index=int(row["section_index"]),
                chunk_index=int(row["chunk_index"]),
                char_offset=int(row["char_offset"]),
            ),
            active_note_id=str(row["active_note_id"]) if row["active_note_id"] else None,
            last_command=str(row["last_command"]) if row["last_command"] else None,
            updated_at=datetime.fromisoformat(str(row["updated_at"])),
        )


class SQLiteNoteRepository(_SQLiteRepository):
    """SQLite storage for anchored notes."""

    def save_note(self, note: VoiceNote) -> None:
        with self._connect() as connection:
            connection.execute(
                """
                INSERT INTO notes(
                    note_id,
                    session_id,
                    document_id,
                    section_index,
                    chunk_index,
                    char_offset,
                    transcript,
                    raw_audio_path,
                    created_at
                )
                VALUES(?, ?, ?, ?, ?, ?, ?, ?, ?)
                ON CONFLICT(note_id) DO UPDATE SET
                    session_id = excluded.session_id,
                    document_id = excluded.document_id,
                    section_index = excluded.section_index,
                    chunk_index = excluded.chunk_index,
                    char_offset = excluded.char_offset,
                    transcript = excluded.transcript,
                    raw_audio_path = excluded.raw_audio_path,
                    created_at = excluded.created_at
                """,
                (
                    note.note_id,
                    note.session_id,
                    note.document_id,
                    note.position.section_index,
                    note.position.chunk_index,
                    note.position.char_offset,
                    note.transcript,
                    str(note.raw_audio_path) if note.raw_audio_path else None,
                    note.created_at.isoformat(),
                ),
            )

    def list_notes_for_document(self, document_id: str) -> Sequence[VoiceNote]:
        with self._connect() as connection:
            rows = connection.execute(
                "SELECT * FROM notes WHERE document_id = ? ORDER BY created_at ASC",
                (document_id,),
            ).fetchall()
        return tuple(_note_from_row(row) for row in rows)

    def search_notes(self, query: SearchQuery | str) -> Sequence[SearchResult]:
        search_query = _normalize_search_query(query)
        needle = f"%{search_query.normalized_text.lower()}%"
        sql = """
            SELECT note_id, section_index, chunk_index, transcript
            FROM notes
            WHERE LOWER(transcript) LIKE ?
        """
        params: list[object] = [needle]
        if search_query.document_id is not None:
            sql += " AND document_id = ?"
            params.append(search_query.document_id)
        sql += " ORDER BY created_at DESC LIMIT ?"
        params.append(max(search_query.limit, 1))

        with self._connect() as connection:
            rows = connection.execute(sql, tuple(params)).fetchall()
        return tuple(
            SearchResult(
                entity_kind="note",
                entity_id=str(row["note_id"]),
                score=1.0,
                excerpt=str(row["transcript"])[:180],
                anchor=f"section:{row['section_index']}/chunk:{row['chunk_index']}",
            )
            for row in rows
        )


class SQLiteRewriteDraftRepository(_SQLiteRepository):
    """SQLite storage for rewrite drafts."""

    def save_draft(self, draft: RewriteDraft) -> None:
        with self._connect() as connection:
            connection.execute(
                """
                INSERT INTO drafts(
                    draft_id,
                    document_id,
                    section_index,
                    source_excerpt,
                    note_transcripts_json,
                    rewritten_text,
                    status,
                    created_at
                )
                VALUES(?, ?, ?, ?, ?, ?, ?, ?)
                ON CONFLICT(draft_id) DO UPDATE SET
                    document_id = excluded.document_id,
                    section_index = excluded.section_index,
                    source_excerpt = excluded.source_excerpt,
                    note_transcripts_json = excluded.note_transcripts_json,
                    rewritten_text = excluded.rewritten_text,
                    status = excluded.status,
                    created_at = excluded.created_at
                """,
                (
                    draft.draft_id,
                    draft.document_id,
                    draft.section_index,
                    draft.source_excerpt,
                    json.dumps(list(draft.note_transcripts)),
                    draft.rewritten_text,
                    draft.status.value,
                    draft.created_at.isoformat(),
                ),
            )

    def list_drafts_for_document(self, document_id: str) -> Sequence[RewriteDraft]:
        with self._connect() as connection:
            rows = connection.execute(
                "SELECT * FROM drafts WHERE document_id = ? ORDER BY created_at DESC",
                (document_id,),
            ).fetchall()
        return tuple(_draft_from_row(row) for row in rows)


def _normalize_search_query(query: SearchQuery | str) -> SearchQuery:
    if isinstance(query, SearchQuery):
        return query
    return SearchQuery(text=query)


def _document_to_payload(document: Document) -> dict[str, Any]:
    return {
        "document_id": document.document_id,
        "title": document.title,
        "source_path": str(document.source_path),
        "imported_at": document.imported_at.isoformat(),
        "sections": [
            {
                "index": section.index,
                "title": section.title,
                "source_anchor": section.source_anchor,
                "chunks": [
                    {
                        "index": chunk.index,
                        "text": chunk.text,
                        "char_start": chunk.char_start,
                        "char_end": chunk.char_end,
                    }
                    for chunk in section.chunks
                ],
            }
            for section in document.sections
        ],
    }


def _document_from_payload(payload: dict[str, Any]) -> Document:
    sections = tuple(
        DocumentSection(
            index=int(section["index"]),
            title=str(section["title"]),
            source_anchor=_optional_string(section["source_anchor"]),
            chunks=tuple(
                DocumentChunk(
                    index=int(chunk["index"]),
                    text=str(chunk["text"]),
                    char_start=int(chunk["char_start"]),
                    char_end=int(chunk["char_end"]),
                )
                for chunk in section["chunks"]
            ),
        )
        for section in payload["sections"]
    )
    return Document(
        document_id=str(payload["document_id"]),
        title=str(payload["title"]),
        source_path=Path(str(payload["source_path"])),
        imported_at=datetime.fromisoformat(str(payload["imported_at"])),
        sections=sections,
    )


def _excerpt_from_document_payload(payload: dict[str, Any], query: str) -> str:
    title = str(payload["title"])
    rendered_sections: list[str] = []
    for section in payload["sections"]:
        section_title = str(section["title"])
        chunk_text = " ".join(str(chunk["text"]) for chunk in section["chunks"])
        rendered_sections.append(f"{section_title}: {chunk_text}")
    combined = f"{title} {' '.join(rendered_sections)}".strip()
    lowered = combined.lower()
    index = lowered.find(query.lower())
    if index < 0:
        return combined[:180]
    start = max(index - 40, 0)
    end = min(index + 140, len(combined))
    return combined[start:end]


def _note_from_row(row: sqlite3.Row) -> VoiceNote:
    return VoiceNote(
        note_id=str(row["note_id"]),
        session_id=str(row["session_id"]),
        document_id=str(row["document_id"]),
        position=ReadingPosition(
            section_index=int(row["section_index"]),
            chunk_index=int(row["chunk_index"]),
            char_offset=int(row["char_offset"]),
        ),
        transcript=str(row["transcript"]),
        raw_audio_path=Path(str(row["raw_audio_path"])) if row["raw_audio_path"] else None,
        created_at=datetime.fromisoformat(str(row["created_at"])),
    )


def _draft_from_row(row: sqlite3.Row) -> RewriteDraft:
    note_transcripts = tuple(json.loads(str(row["note_transcripts_json"])))
    return RewriteDraft(
        draft_id=str(row["draft_id"]),
        document_id=str(row["document_id"]),
        section_index=int(row["section_index"]),
        source_excerpt=str(row["source_excerpt"]),
        note_transcripts=note_transcripts,
        rewritten_text=str(row["rewritten_text"]),
        status=RewriteStatus(str(row["status"])),
        created_at=datetime.fromisoformat(str(row["created_at"])),
    )


def _optional_string(value: object) -> str | None:
    if value is None:
        return None
    return str(value)
