"""SQLite repository tests."""

from __future__ import annotations

from pathlib import Path

from marginalia_core.application.services.document_ingestion_service import (
    DocumentIngestionService,
)
from marginalia_core.domain.document import build_document_outline
from marginalia_core.domain.note import VoiceNote
from marginalia_core.domain.reading_session import ReadingPosition
from marginalia_core.events.models import EventName
from marginalia_infra.events import InMemoryEventBus
from marginalia_infra.storage.sqlite import (
    SQLiteDatabase,
    SQLiteDocumentRepository,
    SQLiteNoteRepository,
)


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


def test_sqlite_database_health_report(tmp_path: Path) -> None:
    database = SQLiteDatabase(tmp_path / "marginalia.sqlite3")

    report = database.health_report()

    assert report["schema_version"] == "1"
    assert "documents" in report["tables"]


def test_document_ingestion_publishes_event(tmp_path: Path) -> None:
    database_path = tmp_path / "marginalia.sqlite3"
    repository = SQLiteDocumentRepository(database_path)
    repository.ensure_schema()
    event_bus = InMemoryEventBus()
    service = DocumentIngestionService(
        document_repository=repository,
        event_publisher=event_bus,
    )

    result = service.ingest_text_file(Path("tests/fixtures/sample_document.txt").resolve())

    assert result.status.value == "ok"
    assert event_bus.published_events[0].name is EventName.DOCUMENT_INGESTED
