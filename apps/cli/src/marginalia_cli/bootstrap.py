"""Composition root for the Marginalia CLI."""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
from typing import Any

from marginalia_adapters.fake.llm import FakeRewriteGenerator, FakeTopicSummarizer
from marginalia_adapters.fake.playback import FakePlaybackEngine
from marginalia_adapters.fake.stt import FakeCommandRecognizer, FakeDictationTranscriber
from marginalia_adapters.fake.tts import FakeSpeechSynthesizer
from marginalia_adapters.real.piper import PiperSpeechSynthesizer
from marginalia_adapters.real.playback import SubprocessPlaybackEngine
from marginalia_adapters.real.vosk import VoskCommandRecognizer
from marginalia_core.application.services.document_ingestion_service import (
    DocumentIngestionService,
)
from marginalia_core.application.services.note_service import NoteService
from marginalia_core.application.services.reader_service import ReaderService
from marginalia_core.application.services.rewrite_service import RewriteService
from marginalia_core.application.services.search_service import SearchService
from marginalia_core.application.services.session_query_service import SessionQueryService
from marginalia_core.application.services.summary_service import SummaryService
from marginalia_core.ports.llm import RewriteGenerator, TopicSummarizer
from marginalia_core.ports.playback import PlaybackEngine
from marginalia_core.ports.stt import CommandRecognizer, DictationTranscriber
from marginalia_core.ports.tts import SpeechSynthesizer
from marginalia_infra.config.settings import AppSettings
from marginalia_infra.events import InMemoryEventBus
from marginalia_infra.logging.setup import configure_logging
from marginalia_infra.storage.sqlite import (
    SQLiteDatabase,
    SQLiteDocumentRepository,
    SQLiteNoteRepository,
    SQLiteRewriteDraftRepository,
    SQLiteSessionRepository,
)


@dataclass(slots=True)
class CliContainer:
    """Runtime object graph for CLI commands."""

    settings: AppSettings
    database: SQLiteDatabase
    event_bus: InMemoryEventBus
    command_stt: CommandRecognizer
    dictation_stt: DictationTranscriber
    speech_synthesizer: SpeechSynthesizer
    playback_engine: PlaybackEngine
    rewrite_provider: RewriteGenerator
    summary_provider: TopicSummarizer
    ingestion_service: DocumentIngestionService
    reader_service: ReaderService
    note_service: NoteService
    rewrite_service: RewriteService
    summary_service: SummaryService
    search_service: SearchService
    session_query_service: SessionQueryService


def build_container(config_path: Path | None = None, *, verbose: bool = False) -> CliContainer:
    """Construct the service graph for a CLI invocation."""

    settings = AppSettings.load(config_path=config_path)
    settings.ensure_directories()
    configure_logging(level="DEBUG" if verbose else settings.log_level)

    event_bus = InMemoryEventBus()
    database = SQLiteDatabase(settings.database_path)
    database.initialize()

    document_repository = SQLiteDocumentRepository(database)
    session_repository = SQLiteSessionRepository(database)
    note_repository = SQLiteNoteRepository(database)
    draft_repository = SQLiteRewriteDraftRepository(database)

    provider_checks = settings.doctor_report()["provider_checks"]
    command_stt = _build_command_recognizer(settings, provider_checks)
    dictation_stt = FakeDictationTranscriber(transcript=settings.fake_dictation_text)
    tts = _build_speech_synthesizer(settings, provider_checks)
    playback = _build_playback_engine(settings, provider_checks)
    rewrite_generator = FakeRewriteGenerator()
    topic_summarizer = FakeTopicSummarizer()

    return CliContainer(
        settings=settings,
        database=database,
        event_bus=event_bus,
        command_stt=command_stt,
        dictation_stt=dictation_stt,
        speech_synthesizer=tts,
        playback_engine=playback,
        rewrite_provider=rewrite_generator,
        summary_provider=topic_summarizer,
        ingestion_service=DocumentIngestionService(
            document_repository=document_repository,
            event_publisher=event_bus,
        ),
        reader_service=ReaderService(
            document_repository=document_repository,
            session_repository=session_repository,
            playback_engine=playback,
            speech_synthesizer=tts,
            event_publisher=event_bus,
            command_recognizer=command_stt,
            default_voice=settings.default_voice,
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
        session_query_service=SessionQueryService(
            session_repository=session_repository,
            document_repository=document_repository,
            note_repository=note_repository,
            draft_repository=draft_repository,
            playback_engine=playback,
        ),
    )


def _build_command_recognizer(
    settings: AppSettings,
    provider_checks: dict[str, Any],
) -> CommandRecognizer:
    provider_name = settings.command_stt_provider
    if provider_name == "vosk":
        if provider_checks["vosk"]["ready"] or not settings.allow_provider_fallback:
            return VoskCommandRecognizer(
                model_path=settings.vosk_model_path,
                commands=settings.vosk_command_grammar,
                sample_rate=settings.vosk_sample_rate,
                timeout_seconds=settings.vosk_listen_timeout_seconds,
            )
    return FakeCommandRecognizer(commands=settings.fake_command_script)


def _build_speech_synthesizer(
    settings: AppSettings,
    provider_checks: dict[str, Any],
) -> SpeechSynthesizer:
    provider_name = settings.tts_provider
    if provider_name == "piper":
        if provider_checks["piper"]["ready"] or not settings.allow_provider_fallback:
            return PiperSpeechSynthesizer(
                executable=settings.piper_executable,
                model_path=settings.piper_model_path,
                output_dir=settings.audio_cache_dir,
                speaker_id=settings.piper_speaker_id,
                length_scale=settings.piper_length_scale,
                noise_scale=settings.piper_noise_scale,
            )
    return FakeSpeechSynthesizer()


def _build_playback_engine(
    settings: AppSettings,
    provider_checks: dict[str, Any],
) -> PlaybackEngine:
    provider_name = settings.playback_provider
    if provider_name == "subprocess":
        if provider_checks["playback"]["ready"] or not settings.allow_provider_fallback:
            return SubprocessPlaybackEngine(command=settings.playback_command)
    return FakePlaybackEngine()
