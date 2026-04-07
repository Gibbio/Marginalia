"""SQLite-backed repositories and schema bootstrap."""

from __future__ import annotations

import json
import sqlite3
import threading
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

SCHEMA_VERSION = "4"
SCHEMA_PROFILE = "sqlite-v4-migrated"
_MIGRATIONS_DIR = Path(__file__).resolve().parent / "migrations"


class SQLiteDatabase:
    """Single SQLite database handle and schema bootstrap utility."""

    def __init__(self, database_path: Path) -> None:
        self._database_path = database_path
        self._connection: sqlite3.Connection | None = None
        self._lock = threading.RLock()

    @property
    def database_path(self) -> Path:
        return self._database_path

    @property
    def lock(self) -> threading.RLock:
        return self._lock

    def connect(self) -> sqlite3.Connection:
        if self._connection is not None:
            return self._connection
        self._database_path.parent.mkdir(parents=True, exist_ok=True)
        connection = sqlite3.connect(
            self._database_path,
            check_same_thread=False,
        )
        connection.row_factory = sqlite3.Row
        connection.execute("PRAGMA foreign_keys = ON")
        connection.execute("PRAGMA journal_mode = WAL")
        connection.execute("PRAGMA busy_timeout = 5000")
        self._connection = connection
        return connection

    def close(self) -> None:
        """Close the cached connection if open."""

        if self._connection is not None:
            self._connection.close()
            self._connection = None

    def initialize(self) -> None:
        with self.connect() as connection:
            self._run_migrations(connection)

    def _run_migrations(self, connection: sqlite3.Connection) -> None:
        connection.execute(
            """
            CREATE TABLE IF NOT EXISTS schema_migrations (
                migration_id TEXT PRIMARY KEY,
                applied_at TEXT NOT NULL
            )
            """
        )
        applied = {
            str(row["migration_id"])
            for row in connection.execute("SELECT migration_id FROM schema_migrations").fetchall()
        }
        migration_files = sorted(_MIGRATIONS_DIR.glob("*.sql"))
        for migration_file in migration_files:
            migration_id = migration_file.stem
            if migration_id in applied:
                continue
            sql = migration_file.read_text(encoding="utf-8")
            connection.executescript(sql)
            connection.execute(
                "INSERT INTO schema_migrations(migration_id, applied_at) VALUES(?, ?)",
                (migration_id, datetime.now().isoformat()),
            )
        connection.execute(
            """
            CREATE TABLE IF NOT EXISTS schema_metadata (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            )
            """
        )
        connection.execute(
            """
            INSERT INTO schema_metadata(key, value)
            VALUES('schema_version', ?)
            ON CONFLICT(key) DO UPDATE SET value = excluded.value
            """,
            (SCHEMA_VERSION,),
        )
        connection.execute(
            """
            INSERT INTO schema_metadata(key, value)
            VALUES('schema_profile', ?)
            ON CONFLICT(key) DO UPDATE SET value = excluded.value
            """,
            (SCHEMA_PROFILE,),
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
        self.initialize()
        return {
            "database_path": self.database_path,
            "database_exists": self.database_path.exists(),
            "schema_version": self.schema_version(),
            "schema_profile": self._metadata_value("schema_profile"),
            "tables": self.table_names(),
            "row_counts": self._table_row_counts(),
        }

    def _metadata_value(self, key: str) -> str:
        with self.connect() as connection:
            row = connection.execute(
                "SELECT value FROM schema_metadata WHERE key = ?",
                (key,),
            ).fetchone()
        return str(row["value"]) if row is not None else "unknown"

    def _table_row_counts(self) -> dict[str, int]:
        counts: dict[str, int] = {}
        table_names = self.table_names()
        with self.connect() as connection:
            for table_name in table_names:
                row = connection.execute(
                    f"SELECT COUNT(*) AS row_count FROM {table_name}"
                ).fetchone()
                counts[table_name] = int(row["row_count"]) if row is not None else 0
        return counts



class _SQLiteRepository:
    def __init__(self, database: Path | SQLiteDatabase) -> None:
        self._database = (
            database if isinstance(database, SQLiteDatabase) else SQLiteDatabase(database)
        )

    def _connect(self) -> "_LockedConnection":
        return _LockedConnection(self._database.connect(), self._database.lock)

    def ensure_schema(self) -> None:
        self._database.initialize()


class _LockedConnection:
    def __init__(self, connection: sqlite3.Connection, lock: threading.RLock) -> None:
        self._connection = connection
        self._lock = lock

    def __enter__(self) -> sqlite3.Connection:
        self._lock.acquire()
        self._connection.__enter__()
        return self._connection

    def __exit__(self, exc_type: object, exc: object, tb: object) -> bool | None:
        try:
            return self._connection.__exit__(exc_type, exc, tb)
        finally:
            self._lock.release()


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
            connection.execute(
                "DELETE FROM document_chunks WHERE document_id = ?",
                (document.document_id,),
            )
            connection.execute(
                "DELETE FROM document_sections WHERE document_id = ?",
                (document.document_id,),
            )
            for section in document.sections:
                connection.execute(
                    """
                    INSERT INTO document_sections(document_id, section_index, title, source_anchor)
                    VALUES(?, ?, ?, ?)
                    """,
                    (
                        document.document_id,
                        section.index,
                        section.title,
                        section.source_anchor,
                    ),
                )
                for chunk in section.chunks:
                    connection.execute(
                        """
                        INSERT INTO document_chunks(
                            document_id,
                            section_index,
                            chunk_index,
                            anchor,
                            text,
                            char_start,
                            char_end
                        )
                        VALUES(?, ?, ?, ?, ?, ?, ?)
                        """,
                        (
                            document.document_id,
                            section.index,
                            chunk.index,
                            f"section:{section.index}/chunk:{chunk.index}",
                            chunk.text,
                            chunk.char_start,
                            chunk.char_end,
                        ),
                    )

    def get_document(self, document_id: str) -> Document | None:
        with self._connect() as connection:
            row = connection.execute(
                """
                SELECT document_id, title, source_path, imported_at, outline_json
                FROM documents
                WHERE document_id = ?
                """,
                (document_id,),
            ).fetchone()
            sections = _sections_from_connection(connection, document_id)
        if row is None:
            return None
        if sections:
            return Document(
                document_id=str(row["document_id"]),
                title=str(row["title"]),
                source_path=Path(str(row["source_path"])),
                imported_at=datetime.fromisoformat(str(row["imported_at"])),
                sections=sections,
            )
        return _document_from_payload(json.loads(str(row["outline_json"])))

    def list_documents(self) -> Sequence[Document]:
        with self._connect() as connection:
            rows = connection.execute(
                "SELECT document_id FROM documents ORDER BY imported_at DESC"
            ).fetchall()
        documents: list[Document] = []
        for row in rows:
            document = self.get_document(str(row["document_id"]))
            if document is not None:
                documents.append(document)
        return tuple(documents)

    def search_documents(self, query: SearchQuery | str) -> Sequence[SearchResult]:
        search_query = _normalize_search_query(query)
        needle = f"%{search_query.normalized_text.lower()}%"
        sql = """
            SELECT
                d.document_id,
                d.title,
                s.title AS section_title,
                c.anchor,
                c.text AS chunk_text
            FROM document_chunks c
            JOIN document_sections s
                ON s.document_id = c.document_id
                AND s.section_index = c.section_index
            JOIN documents d ON d.document_id = c.document_id
            WHERE (
                LOWER(d.title) LIKE ?
                OR LOWER(s.title) LIKE ?
                OR LOWER(c.text) LIKE ?
            )
        """
        params: list[object] = [needle, needle, needle]
        if search_query.document_id is not None:
            sql += " AND d.document_id = ?"
            params.append(search_query.document_id)
        sql += " ORDER BY d.imported_at DESC, c.section_index ASC, c.chunk_index ASC LIMIT ?"
        params.append(max(search_query.limit, 1))

        with self._connect() as connection:
            rows = connection.execute(sql, tuple(params)).fetchall()

        if not rows:
            return _fallback_document_search(self.list_documents(), search_query)

        return tuple(
            SearchResult(
                entity_kind="document",
                entity_id=str(row["document_id"]),
                score=1.0,
                excerpt=_excerpt_from_text(
                    f"{row['section_title']}: {row['chunk_text']}",
                    search_query.normalized_text,
                ),
                anchor=str(row["anchor"]),
            )
            for row in rows
        )


class SQLiteSessionRepository(_SQLiteRepository):
    """SQLite storage for reading sessions."""

    def save_session(self, session: ReadingSession) -> None:
        with self._connect() as connection:
            if session.is_active:
                connection.execute(
                    "UPDATE sessions SET is_active = 0 WHERE session_id != ? AND is_active = 1",
                    (session.session_id,),
                )
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
                    last_command_source,
                    last_recognized_command,
                    voice,
                    tts_provider,
                    command_stt_provider,
                    playback_provider,
                    command_listening_active,
                    command_language,
                    audio_reference,
                    playback_process_id,
                    runtime_process_id,
                    runtime_status,
                    runtime_error,
                    startup_cleanup_summary,
                    is_active,
                    updated_at
                )
                VALUES(?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                ON CONFLICT(session_id) DO UPDATE SET
                    document_id = excluded.document_id,
                    state = excluded.state,
                    playback_state = excluded.playback_state,
                    section_index = excluded.section_index,
                    chunk_index = excluded.chunk_index,
                    char_offset = excluded.char_offset,
                    active_note_id = excluded.active_note_id,
                    last_command = excluded.last_command,
                    last_command_source = excluded.last_command_source,
                    last_recognized_command = excluded.last_recognized_command,
                    voice = excluded.voice,
                    tts_provider = excluded.tts_provider,
                    command_stt_provider = excluded.command_stt_provider,
                    playback_provider = excluded.playback_provider,
                    command_listening_active = excluded.command_listening_active,
                    command_language = excluded.command_language,
                    audio_reference = excluded.audio_reference,
                    playback_process_id = excluded.playback_process_id,
                    runtime_process_id = excluded.runtime_process_id,
                    runtime_status = excluded.runtime_status,
                    runtime_error = excluded.runtime_error,
                    startup_cleanup_summary = excluded.startup_cleanup_summary,
                    is_active = excluded.is_active,
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
                    session.last_command_source,
                    session.last_recognized_command,
                    session.voice,
                    session.tts_provider,
                    session.command_stt_provider,
                    session.playback_provider,
                    int(session.command_listening_active),
                    session.command_language,
                    session.audio_reference,
                    session.playback_process_id,
                    session.runtime_process_id,
                    session.runtime_status,
                    session.runtime_error,
                    session.startup_cleanup_summary,
                    int(session.is_active),
                    session.updated_at.isoformat(),
                ),
            )

    def get_active_session(self) -> ReadingSession | None:
        with self._connect() as connection:
            row = connection.execute(
                """
                SELECT *
                FROM sessions
                WHERE is_active = 1
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
            last_command_source=(
                str(row["last_command_source"]) if row["last_command_source"] else None
            ),
            last_recognized_command=(
                str(row["last_recognized_command"]) if row["last_recognized_command"] else None
            ),
            voice=str(row["voice"]) if row["voice"] else None,
            tts_provider=str(row["tts_provider"]) if row["tts_provider"] else None,
            command_stt_provider=(
                str(row["command_stt_provider"]) if row["command_stt_provider"] else None
            ),
            playback_provider=str(row["playback_provider"]) if row["playback_provider"] else None,
            command_listening_active=bool(int(row["command_listening_active"] or 0)),
            command_language=str(row["command_language"]) if row["command_language"] else None,
            audio_reference=str(row["audio_reference"]) if row["audio_reference"] else None,
            playback_process_id=(
                int(row["playback_process_id"]) if row["playback_process_id"] is not None else None
            ),
            runtime_process_id=(
                int(row["runtime_process_id"]) if row["runtime_process_id"] is not None else None
            ),
            runtime_status=str(row["runtime_status"]) if row["runtime_status"] else None,
            runtime_error=str(row["runtime_error"]) if row["runtime_error"] else None,
            startup_cleanup_summary=(
                str(row["startup_cleanup_summary"])
                if row["startup_cleanup_summary"]
                else None
            ),
            is_active=bool(int(row["is_active"] or 0)),
            updated_at=datetime.fromisoformat(str(row["updated_at"])),
        )


    def deactivate_stale_sessions(self, *, max_inactive_hours: int) -> int:
        """Mark sessions as inactive when they exceed the staleness threshold."""
        with self._connect() as connection:
            cursor = connection.execute(
                """
                UPDATE sessions
                SET is_active = 0
                WHERE is_active = 1
                  AND updated_at < datetime('now', ? || ' hours')
                """,
                (f"-{max_inactive_hours}",),
            )
            return cursor.rowcount


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
                    transcription_provider,
                    language,
                    raw_audio_path,
                    created_at
                )
                VALUES(?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                ON CONFLICT(note_id) DO UPDATE SET
                    session_id = excluded.session_id,
                    document_id = excluded.document_id,
                    section_index = excluded.section_index,
                    chunk_index = excluded.chunk_index,
                    char_offset = excluded.char_offset,
                    transcript = excluded.transcript,
                    transcription_provider = excluded.transcription_provider,
                    language = excluded.language,
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
                    note.transcription_provider,
                    note.language,
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
                    source_anchor,
                    source_excerpt,
                    note_transcripts_json,
                    rewritten_text,
                    provider_name,
                    status,
                    created_at
                )
                VALUES(?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                ON CONFLICT(draft_id) DO UPDATE SET
                    document_id = excluded.document_id,
                    section_index = excluded.section_index,
                    source_anchor = excluded.source_anchor,
                    source_excerpt = excluded.source_excerpt,
                    note_transcripts_json = excluded.note_transcripts_json,
                    rewritten_text = excluded.rewritten_text,
                    provider_name = excluded.provider_name,
                    status = excluded.status,
                    created_at = excluded.created_at
                """,
                (
                    draft.draft_id,
                    draft.document_id,
                    draft.section_index,
                    draft.source_anchor,
                    draft.source_excerpt,
                    json.dumps(list(draft.note_transcripts)),
                    draft.rewritten_text,
                    draft.provider_name,
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


def _fallback_document_search(
    documents: Sequence[Document],
    query: SearchQuery,
) -> tuple[SearchResult, ...]:
    results: list[SearchResult] = []
    lowered_query = query.normalized_text.lower()
    for document in documents:
        for section in document.sections:
            for chunk in section.chunks:
                haystack = f"{document.title} {section.title} {chunk.text}".lower()
                if lowered_query not in haystack:
                    continue
                results.append(
                    SearchResult(
                        entity_kind="document",
                        entity_id=document.document_id,
                        score=1.0,
                        excerpt=_excerpt_from_text(chunk.text, query.normalized_text),
                        anchor=f"section:{section.index}/chunk:{chunk.index}",
                    )
                )
                if len(results) >= max(query.limit, 1):
                    return tuple(results)
    return tuple(results)


def _sections_from_connection(
    connection: sqlite3.Connection,
    document_id: str,
) -> tuple[DocumentSection, ...]:
    section_rows = connection.execute(
        """
        SELECT section_index, title, source_anchor
        FROM document_sections
        WHERE document_id = ?
        ORDER BY section_index ASC
        """,
        (document_id,),
    ).fetchall()
    if not section_rows:
        return ()

    chunk_rows = connection.execute(
        """
        SELECT section_index, chunk_index, text, char_start, char_end
        FROM document_chunks
        WHERE document_id = ?
        ORDER BY section_index ASC, chunk_index ASC
        """,
        (document_id,),
    ).fetchall()
    chunks_by_section: dict[int, list[DocumentChunk]] = {}
    for row in chunk_rows:
        section_index = int(row["section_index"])
        chunks_by_section.setdefault(section_index, []).append(
            DocumentChunk(
                index=int(row["chunk_index"]),
                text=str(row["text"]),
                char_start=int(row["char_start"]),
                char_end=int(row["char_end"]),
            )
        )

    return tuple(
        DocumentSection(
            index=int(row["section_index"]),
            title=str(row["title"]),
            source_anchor=_optional_string(row["source_anchor"]),
            chunks=tuple(chunks_by_section.get(int(row["section_index"]), [])),
        )
        for row in section_rows
    )


def _excerpt_from_text(text: str, query: str) -> str:
    lowered = text.lower()
    index = lowered.find(query.lower())
    if index < 0:
        return text[:180]
    start = max(index - 40, 0)
    end = min(index + 140, len(text))
    return text[start:end]


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
        transcription_provider=str(row["transcription_provider"]),
        language=str(row["language"]),
        raw_audio_path=Path(str(row["raw_audio_path"])) if row["raw_audio_path"] else None,
        created_at=datetime.fromisoformat(str(row["created_at"])),
    )


def _draft_from_row(row: sqlite3.Row) -> RewriteDraft:
    note_transcripts = tuple(json.loads(str(row["note_transcripts_json"])))
    return RewriteDraft(
        draft_id=str(row["draft_id"]),
        document_id=str(row["document_id"]),
        section_index=int(row["section_index"]),
        source_anchor=str(row["source_anchor"]),
        source_excerpt=str(row["source_excerpt"]),
        note_transcripts=note_transcripts,
        rewritten_text=str(row["rewritten_text"]),
        provider_name=str(row["provider_name"]),
        status=RewriteStatus(str(row["status"])),
        created_at=datetime.fromisoformat(str(row["created_at"])),
    )


def _optional_string(value: object) -> str | None:
    if value is None:
        return None
    return str(value)
