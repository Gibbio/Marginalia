"""Tests for background pre-synthesis of the next chunk."""

from __future__ import annotations

import threading
from pathlib import Path

from marginalia_adapters.fake.playback import FakePlaybackEngine
from marginalia_adapters.fake.stt import FakeCommandRecognizer
from marginalia_adapters.fake.tts import FakeSpeechSynthesizer
from marginalia_core.application.command_router import load_command_lexicon
from marginalia_core.application.services.document_ingestion_service import (
    DocumentIngestionService,
)
from marginalia_core.application.services.reader_service import ReaderService
from marginalia_core.ports.tts import SynthesisRequest, SynthesisResult
from marginalia_infra.events import InMemoryEventBus
from marginalia_infra.storage.sqlite import (
    SQLiteDatabase,
    SQLiteDocumentRepository,
    SQLiteSessionRepository,
)


class _CountingSynthesizer(FakeSpeechSynthesizer):
    """FakeSpeechSynthesizer that counts how many times synthesize() is called."""

    def __init__(self) -> None:
        super().__init__()
        self.call_count = 0
        self.texts: list[str] = []
        self._lock = threading.Lock()

    def synthesize(self, request: SynthesisRequest) -> SynthesisResult:
        with self._lock:
            self.call_count += 1
            self.texts.append(request.text)
        return super().synthesize(request)


def test_play_triggers_pre_synthesis_of_next_chunk(tmp_path: Path) -> None:
    """After play(), the synthesizer is called twice: current chunk + pre-synth."""

    synth = _CountingSynthesizer()
    reader, _ = _build(tmp_path, synthesizer=synth, chunk_target_chars=30)
    _ingest(tmp_path, reader)

    reader.play(None, command_source="test")
    # Wait for the background pre-synthesis thread to complete
    reader._wait_for_pre_synthesis()

    # At least 2 calls: one for the current chunk, one for pre-synthesis
    assert synth.call_count >= 2, (
        f"Expected at least 2 synthesize calls, got {synth.call_count}"
    )


def test_pre_synthesis_does_not_fire_at_last_chunk(tmp_path: Path) -> None:
    """At the very last chunk of the document, no pre-synthesis thread is spawned."""

    synth = _CountingSynthesizer()
    reader, _ = _build(tmp_path, synthesizer=synth)
    _ingest(tmp_path, reader)

    reader.play(None, command_source="test")
    reader._wait_for_pre_synthesis()

    # Navigate to the last chapter, last chunk
    reader.next_chapter(command_source="test")
    reader._wait_for_pre_synthesis()

    # The pre-synth thread should be None (no next chunk to pre-synth)
    assert reader._pre_synth_thread is None or not reader._pre_synth_thread.is_alive()
    # Confirm we're at the last section
    status = reader.report_voice_status()
    progress = status.data["progress"]
    assert progress["section_index"] == progress["section_count"] - 1


def test_advance_uses_pre_synthesized_cache(tmp_path: Path) -> None:
    """After pre-synthesis, advancing to the next chunk calls synthesize on cached text."""

    synth = _CountingSynthesizer()
    reader, _ = _build(tmp_path, synthesizer=synth, chunk_target_chars=30,
                       playback_auto_complete=0)
    _ingest(tmp_path, reader)

    reader.play(None, command_source="test")
    reader._wait_for_pre_synthesis()

    calls_before = synth.call_count
    pre_synth_texts = list(synth.texts)

    # Advance to next chunk — should call synthesize for same text that was pre-synth'd
    reader.advance_after_playback_completion()
    reader._wait_for_pre_synthesis()

    # The text synthesized for the advance should match a pre-synthesized text
    advance_texts = synth.texts[calls_before:]
    assert len(advance_texts) >= 1
    # The first call after advance is the chunk playback — its text should be
    # one of the texts we pre-synthesized
    assert advance_texts[0] in pre_synth_texts


def test_pre_synthesis_thread_is_daemon(tmp_path: Path) -> None:
    """The pre-synthesis thread is a daemon thread (won't block process exit)."""

    synth = _CountingSynthesizer()
    reader, _ = _build(tmp_path, synthesizer=synth, chunk_target_chars=30)
    _ingest(tmp_path, reader)

    reader.play(None, command_source="test")

    if reader._pre_synth_thread is not None:
        assert reader._pre_synth_thread.daemon is True
    reader._wait_for_pre_synthesis()


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def _build(
    tmp_path: Path,
    *,
    synthesizer: FakeSpeechSynthesizer | None = None,
    chunk_target_chars: int = 300,
    playback_auto_complete: int | None = None,
) -> tuple[ReaderService, SQLiteSessionRepository]:
    database = SQLiteDatabase(tmp_path / "marginalia.sqlite3")
    database.initialize()
    doc_repo = SQLiteDocumentRepository(database)
    session_repo = SQLiteSessionRepository(database)
    event_bus = InMemoryEventBus()
    ingestion = DocumentIngestionService(
        document_repository=doc_repo,
        event_publisher=event_bus,
        chunk_target_chars=chunk_target_chars,
    )
    lexicon = load_command_lexicon(
        Path("packages/infra/src/marginalia_infra/config/commands/it.toml")
    )
    reader = ReaderService(
        document_repository=doc_repo,
        session_repository=session_repo,
        playback_engine=FakePlaybackEngine(
            auto_complete_after_snapshots=playback_auto_complete,
        ),
        speech_synthesizer=synthesizer or FakeSpeechSynthesizer(),
        event_publisher=event_bus,
        command_recognizer=FakeCommandRecognizer(),
        command_lexicon=lexicon,
        default_voice="if_sara",
    )
    # Store ingestion service for use by _ingest
    reader._test_ingestion = ingestion  # type: ignore[attr-defined]
    reader._test_doc_repo = doc_repo  # type: ignore[attr-defined]
    return reader, session_repo


def _ingest(tmp_path: Path, reader: ReaderService) -> None:
    fixture = Path("tests/fixtures/sample_document.txt").resolve()
    ingestion = reader._test_ingestion  # type: ignore[attr-defined]
    ingestion.ingest_text_file(fixture)
