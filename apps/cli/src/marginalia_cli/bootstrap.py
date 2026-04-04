"""Composition root for the Marginalia CLI."""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path

from marginalia_adapters.fake.llm import FakeRewriteGenerator, FakeTopicSummarizer
from marginalia_adapters.fake.playback import FakePlaybackEngine
from marginalia_adapters.fake.stt import FakeCommandRecognizer, FakeDictationTranscriber
from marginalia_adapters.fake.tts import FakeSpeechSynthesizer
from marginalia_core.application.services.note_service import NoteService
from marginalia_core.application.services.reader_service import ReaderService
from marginalia_core.application.services.rewrite_service import RewriteService
from marginalia_core.application.services.search_service import SearchService
from marginalia_core.application.services.storage_coordinator import StorageCoordinationService
from marginalia_core.application.services.summary_service import SummaryService
from marginalia_infra.config.settings import AppSettings
from marginalia_infra.events import InMemoryEventBus
from marginalia_infra.logging.setup import configure_logging
from marginalia_infra.storage.sqlite import (
    SQLiteDocumentRepository,
    SQLiteNoteRepository,
    SQLiteRewriteDraftRepository,
    SQLiteSessionRepository,
)


@dataclass(slots=True)
class CliContainer:
    """Runtime object graph for CLI commands."""

    settings: AppSettings
    reader_service: ReaderService
    note_service: NoteService
    rewrite_service: RewriteService
    summary_service: SummaryService
    search_service: SearchService
    storage_service: StorageCoordinationService


def build_container(config_path: Path | None = None, *, verbose: bool = False) -> CliContainer:
    """Construct the service graph for a CLI invocation."""

    settings = AppSettings.load(config_path=config_path)
    settings.ensure_directories()
    configure_logging(level="DEBUG" if verbose else settings.log_level)

    event_bus = InMemoryEventBus()

    document_repository = SQLiteDocumentRepository(settings.database_path)
    session_repository = SQLiteSessionRepository(settings.database_path)
    note_repository = SQLiteNoteRepository(settings.database_path)
    draft_repository = SQLiteRewriteDraftRepository(settings.database_path)

    for repository in (
        document_repository,
        session_repository,
        note_repository,
        draft_repository,
    ):
        repository.ensure_schema()

    command_stt = FakeCommandRecognizer()
    dictation_stt = FakeDictationTranscriber()
    tts = FakeSpeechSynthesizer()
    playback = FakePlaybackEngine()
    rewrite_generator = FakeRewriteGenerator()
    topic_summarizer = FakeTopicSummarizer()

    return CliContainer(
        settings=settings,
        reader_service=ReaderService(
            document_repository=document_repository,
            session_repository=session_repository,
            playback_engine=playback,
            speech_synthesizer=tts,
            event_publisher=event_bus,
            command_recognizer=command_stt,
        ),
        note_service=NoteService(
            session_repository=session_repository,
            note_repository=note_repository,
            dictation_transcriber=dictation_stt,
            event_publisher=event_bus,
        ),
        rewrite_service=RewriteService(
            session_repository=session_repository,
            note_repository=note_repository,
            draft_repository=draft_repository,
            document_repository=document_repository,
            rewrite_generator=rewrite_generator,
            event_publisher=event_bus,
        ),
        summary_service=SummaryService(
            document_repository=document_repository,
            topic_summarizer=topic_summarizer,
            event_publisher=event_bus,
        ),
        search_service=SearchService(
            document_repository=document_repository,
            note_repository=note_repository,
        ),
        storage_service=StorageCoordinationService(
            document_repository=document_repository,
        ),
    )
