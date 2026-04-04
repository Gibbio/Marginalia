"""SQLite repository tests."""

from __future__ import annotations

from pathlib import Path

from marginalia_core.domain.document import build_document_outline
from marginalia_core.domain.note import VoiceNote
from marginalia_core.domain.reading_session import ReadingPosition
from marginalia_infra.storage.sqlite import SQLiteDocumentRepository, SQLiteNoteRepository


def test_sqlite_document_round_trip(tmp_path: Path) -> None:
    database_path = tmp_path / "marginalia.sqlite3"
    repository = SQLiteDocumentRepository(database_path)
    repository.ensure_schema()

    document = build_document_outline(
        tmp_path / "sample.md",
        "# Chapter One\n\nExample text.\n\n# Chapter Two\n\nMore text.",
    )
    repository.save_document(document)

    restored = repository.get_document(document.document_id)

    assert restored is not None
    assert restored.document_id == document.document_id
    assert restored.chapter_count == 2


def test_sqlite_note_search(tmp_path: Path) -> None:
    database_path = tmp_path / "marginalia.sqlite3"
    repository = SQLiteNoteRepository(database_path)
    repository.ensure_schema()

    repository.save_note(
        VoiceNote(
            note_id="note-1",
            session_id="session-1",
            document_id="doc-1",
            position=ReadingPosition(section_index=0, chunk_index=0),
            transcript="This section needs a clearer argument.",
        )
    )

    results = repository.search_notes("clearer")

    assert len(results) == 1
    assert results[0].entity_id == "note-1"
