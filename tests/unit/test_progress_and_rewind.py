"""Tests for reading progress tracking (Step 4) and REWIND command (Step 5)."""

from __future__ import annotations

from pathlib import Path

from marginalia_adapters.fake.playback import FakePlaybackEngine
from marginalia_adapters.fake.stt import FakeCommandRecognizer
from marginalia_adapters.fake.tts import FakeSpeechSynthesizer
from marginalia_core.application.command_router import (
    CommandLexicon,
    VoiceCommandIntent,
    load_command_lexicon,
    resolve_voice_command,
)
from marginalia_core.application.services.document_ingestion_service import (
    DocumentIngestionService,
)
from marginalia_core.application.services.reader_service import ReaderService
from marginalia_core.application.services.session_query_service import SessionQueryService
from marginalia_core.events.models import EventName
from marginalia_infra.events import InMemoryEventBus
from marginalia_infra.storage.sqlite import (
    SQLiteDatabase,
    SQLiteDocumentRepository,
    SQLiteNoteRepository,
    SQLiteRewriteDraftRepository,
    SQLiteSessionRepository,
)

# ---------------------------------------------------------------------------
# Step 4 — Reading Progress
# ---------------------------------------------------------------------------


def test_status_includes_progress_fractions(tmp_path: Path) -> None:
    """current_status() includes a progress dict with section/chunk fractions."""

    container = _build_services(tmp_path)
    _ingest_and_play(container)

    result = container.session_query.current_status()

    assert result.status.value == "ok"
    progress = result.data["progress"]
    assert progress["section_index"] == 0
    assert progress["section_count"] == 2
    assert progress["chunk_index"] == 0
    assert progress["total_chunks"] >= 2


def test_voice_status_includes_progress(tmp_path: Path) -> None:
    """report_voice_status() includes a progress dict."""

    container = _build_services(tmp_path)
    _ingest_and_play(container)

    result = container.reader.report_voice_status()

    assert result.status.value == "ok"
    assert "progress" in result.data
    progress = result.data["progress"]
    assert progress["section_count"] == 2
    assert progress["chunks_read"] == 0


def test_synchronize_includes_progress(tmp_path: Path) -> None:
    """synchronize_active_session() includes a progress dict."""

    container = _build_services(tmp_path)
    _ingest_and_play(container)

    result = container.reader.synchronize_active_session()

    assert result.status.value == "ok"
    assert "progress" in result.data
    assert result.data["progress"]["section_count"] == 2


def test_progress_event_includes_totals(tmp_path: Path) -> None:
    """READING_PROGRESSED events carry section_count and total_chunks."""

    container = _build_services(tmp_path)
    _ingest_and_play(container)

    progress_events = [
        e for e in container.event_bus.published_events
        if e.name is EventName.READING_PROGRESSED
    ]
    assert len(progress_events) >= 1
    payload = progress_events[0].payload
    assert "section_count" in payload
    assert "section_chunk_count" in payload
    assert "total_chunks" in payload
    assert "chunks_read" in payload


def test_progress_advances_with_next_chapter(tmp_path: Path) -> None:
    """After next_chapter(), progress section_index increments."""

    container = _build_services(tmp_path)
    _ingest_and_play(container)

    container.reader.next_chapter(command_source="test")
    result = container.reader.report_voice_status()

    progress = result.data["progress"]
    assert progress["section_index"] == 1
    assert progress["chunks_read"] > 0


# ---------------------------------------------------------------------------
# Step 5 — REWIND command
# ---------------------------------------------------------------------------


def test_rewind_intent_in_lexicon() -> None:
    """The REWIND intent is loaded from both Italian and English lexicons."""

    it_lexicon = load_command_lexicon(
        Path("packages/infra/src/marginalia_infra/config/commands/it.toml")
    )
    en_lexicon = load_command_lexicon(
        Path("packages/infra/src/marginalia_infra/config/commands/en.toml")
    )

    assert resolve_voice_command("indietro", it_lexicon) is VoiceCommandIntent.REWIND
    assert resolve_voice_command("precedente", it_lexicon) is VoiceCommandIntent.REWIND
    assert resolve_voice_command("back", en_lexicon) is VoiceCommandIntent.REWIND
    assert resolve_voice_command("previous", en_lexicon) is VoiceCommandIntent.REWIND


def test_rewind_at_document_start_returns_error(tmp_path: Path) -> None:
    """Rewinding at section 0, chunk 0 returns an error."""

    container = _build_services(tmp_path)
    _ingest_and_play(container)

    result = container.reader.previous_chunk(command_source="test")

    assert result.status.value == "error"
    assert "beginning" in result.message.lower()


def test_rewind_goes_back_one_chunk(tmp_path: Path) -> None:
    """After advancing one chunk, rewind returns to the previous one."""

    # Use a small chunk target so each section has multiple chunks
    container = _build_services(tmp_path, playback_auto_complete=0, chunk_target_chars=30)
    _ingest_and_play(container)

    # Advance to chunk 1 within the same section
    advance_result = container.reader.advance_after_playback_completion()
    assert advance_result.data["completed_document"] is False
    status_before = container.reader.report_voice_status()
    chunk_before = status_before.data["progress"]["chunk_index"]
    assert chunk_before > 0, "Need multiple chunks per section for this test"

    result = container.reader.previous_chunk(command_source="test")

    assert result.status.value == "ok"
    status_after = container.reader.report_voice_status()
    assert status_after.data["progress"]["chunk_index"] == chunk_before - 1


def test_rewind_crosses_to_previous_section(tmp_path: Path) -> None:
    """Rewinding at the first chunk of a section goes to the last chunk of the previous one."""

    container = _build_services(tmp_path)
    _ingest_and_play(container)

    # Move to second chapter
    container.reader.next_chapter(command_source="test")
    status = container.reader.report_voice_status()
    assert status.data["progress"]["section_index"] == 1
    assert status.data["progress"]["chunk_index"] == 0

    result = container.reader.previous_chunk(command_source="test")

    assert result.status.value == "ok"
    status_after = container.reader.report_voice_status()
    assert status_after.data["progress"]["section_index"] == 0


def test_rewind_dispatched_via_voice_command(tmp_path: Path) -> None:
    """The 'indietro' voice command dispatches as rewind."""

    container = _build_services(
        tmp_path,
        commands=("capitolo successivo", "indietro", "stop"),
        playback_auto_complete=2,
    )
    from marginalia_core.application.services.reading_runtime_service import ReadingRuntimeService
    from marginalia_infra.runtime.session_supervisor import FileRuntimeSupervisor

    runtime_supervisor = FileRuntimeSupervisor(tmp_path / "runtime" / "active-session.json")
    runtime_service = ReadingRuntimeService(
        document_repository=container.doc_repo,
        session_repository=container.session_repo,
        ingestion_service=container.ingestion,
        reader_service=container.reader,
        command_recognizer=container.recognizer,
        runtime_supervisor=runtime_supervisor,
        command_lexicon=container.lexicon,
    )

    result = runtime_service.play(str(Path("tests/fixtures/sample_document.txt").resolve()))

    assert result.status.value == "ok"
    handled = result.data["runtime"]["handled_commands"]
    intents = [cmd["handled_command"] for cmd in handled]
    assert "rewind" in intents


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


class _Container:
    """Lightweight holder for test service instances."""

    def __init__(
        self,
        *,
        reader: ReaderService,
        session_query: SessionQueryService,
        ingestion: DocumentIngestionService,
        event_bus: InMemoryEventBus,
        doc_repo: SQLiteDocumentRepository,
        session_repo: SQLiteSessionRepository,
        recognizer: FakeCommandRecognizer,
        lexicon: CommandLexicon,
    ) -> None:
        self.reader = reader
        self.session_query = session_query
        self.ingestion = ingestion
        self.event_bus = event_bus
        self.doc_repo = doc_repo
        self.session_repo = session_repo
        self.recognizer = recognizer
        self.lexicon = lexicon


def _build_services(
    tmp_path: Path,
    *,
    commands: tuple[str, ...] = (),
    playback_auto_complete: int | None = None,
    chunk_target_chars: int = 300,
) -> _Container:
    database = SQLiteDatabase(tmp_path / "marginalia.sqlite3")
    database.initialize()
    doc_repo = SQLiteDocumentRepository(database)
    session_repo = SQLiteSessionRepository(database)
    note_repo = SQLiteNoteRepository(database)
    draft_repo = SQLiteRewriteDraftRepository(database)
    event_bus = InMemoryEventBus()
    ingestion = DocumentIngestionService(
        document_repository=doc_repo,
        event_publisher=event_bus,
        chunk_target_chars=chunk_target_chars,
    )
    lexicon = load_command_lexicon(
        Path("packages/infra/src/marginalia_infra/config/commands/it.toml")
    )
    recognizer = FakeCommandRecognizer(commands=commands)
    reader = ReaderService(
        document_repository=doc_repo,
        session_repository=session_repo,
        playback_engine=FakePlaybackEngine(
            auto_complete_after_snapshots=playback_auto_complete,
        ),
        speech_synthesizer=FakeSpeechSynthesizer(),
        event_publisher=event_bus,
        command_recognizer=recognizer,
        command_lexicon=lexicon,
        default_voice="if_sara",
    )
    session_query = SessionQueryService(
        session_repository=session_repo,
        document_repository=doc_repo,
        note_repository=note_repo,
        draft_repository=draft_repo,
        playback_engine=FakePlaybackEngine(),
    )
    return _Container(
        reader=reader,
        session_query=session_query,
        ingestion=ingestion,
        event_bus=event_bus,
        doc_repo=doc_repo,
        session_repo=session_repo,
        recognizer=recognizer,
        lexicon=lexicon,
    )


def _ingest_and_play(container: _Container) -> None:
    fixture = Path("tests/fixtures/sample_document.txt").resolve()
    container.ingestion.ingest_text_file(fixture)
    container.reader.play(None, command_source="test")
