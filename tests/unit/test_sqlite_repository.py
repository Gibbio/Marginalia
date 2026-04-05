"""SQLite repository tests."""

from __future__ import annotations

from pathlib import Path

from marginalia_core.application.services.document_ingestion_service import (
    DocumentIngestionService,
)
from marginalia_core.domain.document import build_document_outline
from marginalia_core.domain.note import VoiceNote
from marginalia_core.domain.reading_session import (
    PlaybackState,
    ReaderState,
    ReadingPosition,
    ReadingSession,
)
from marginalia_core.domain.rewrite import RewriteDraft, RewriteStatus
from marginalia_core.events.models import EventName
from marginalia_infra.events import InMemoryEventBus
from marginalia_infra.storage.sqlite import (
    SQLiteDatabase,
    SQLiteDocumentRepository,
    SQLiteNoteRepository,
    SQLiteRewriteDraftRepository,
    SQLiteSessionRepository,
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
    assert restored.get_chunk(1, 0).text == "More text."


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
    assert repository.list_notes_for_document("doc-1")[0].transcription_provider == "unknown"


def test_sqlite_database_health_report(tmp_path: Path) -> None:
    database = SQLiteDatabase(tmp_path / "marginalia.sqlite3")

    report = database.health_report()

    assert report["schema_version"] == "4"
    assert report["schema_profile"] == "sqlite-v4-migrated"
    assert "documents" in report["tables"]
    assert "document_sections" in report["tables"]
    assert "document_chunks" in report["tables"]


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


def test_sqlite_rewrite_draft_round_trip(tmp_path: Path) -> None:
    database_path = tmp_path / "marginalia.sqlite3"
    repository = SQLiteRewriteDraftRepository(database_path)
    repository.ensure_schema()

    repository.save_draft(
        RewriteDraft(
            draft_id="draft-1",
            document_id="doc-1",
            section_index=0,
            source_anchor="section:0/chunk:0",
            source_excerpt="Original text.",
            note_transcripts=("Clarify the argument.",),
            rewritten_text="Rewritten text.",
            provider_name="fake-rewrite-llm",
            status=RewriteStatus.GENERATED,
        )
    )

    drafts = repository.list_drafts_for_document("doc-1")

    assert len(drafts) == 1
    assert drafts[0].source_anchor == "section:0/chunk:0"
    assert drafts[0].provider_name == "fake-rewrite-llm"


def test_sqlite_session_round_trip_preserves_provider_runtime_metadata(tmp_path: Path) -> None:
    database_path = tmp_path / "marginalia.sqlite3"
    repository = SQLiteSessionRepository(database_path)
    repository.ensure_schema()

    repository.save_session(
        ReadingSession(
            session_id="session-1",
            document_id="doc-1",
            state=ReaderState.READING,
            playback_state=PlaybackState.PLAYING,
            position=ReadingPosition(section_index=1, chunk_index=2, char_offset=8),
            last_command="play",
            last_command_source="cli",
            last_recognized_command="continua",
            voice="it_IT-demo",
            tts_provider="piper",
            command_stt_provider="vosk-command-stt",
            playback_provider="subprocess-playback",
            command_listening_active=True,
            command_language="it",
            audio_reference="/tmp/audio.wav",
            playback_process_id=4321,
            runtime_process_id=9876,
            runtime_status="active",
            runtime_error=None,
            startup_cleanup_summary="Terminated stale runtime pid 111.",
        )
    )

    restored = repository.get_active_session()

    assert restored is not None
    assert restored.last_command_source == "cli"
    assert restored.last_recognized_command == "continua"
    assert restored.tts_provider == "piper"
    assert restored.command_stt_provider == "vosk-command-stt"
    assert restored.playback_provider == "subprocess-playback"
    assert restored.command_listening_active is True
    assert restored.command_language == "it"
    assert restored.audio_reference == "/tmp/audio.wav"
    assert restored.playback_process_id == 4321
    assert restored.runtime_process_id == 9876
    assert restored.runtime_status == "active"
    assert restored.startup_cleanup_summary == "Terminated stale runtime pid 111."
