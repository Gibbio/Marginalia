"""Composition root for the Marginalia CLI."""

from __future__ import annotations

import logging
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from marginalia_adapters.fake.llm import FakeRewriteGenerator, FakeTopicSummarizer
from marginalia_adapters.fake.playback import FakePlaybackEngine
from marginalia_adapters.fake.stt import FakeCommandRecognizer, FakeDictationTranscriber
from marginalia_adapters.fake.tts import FakeSpeechSynthesizer
from marginalia_adapters.real.kokoro import KokoroSpeechSynthesizer
from marginalia_adapters.real.piper import PiperSpeechSynthesizer
from marginalia_adapters.real.playback import SubprocessPlaybackEngine
from marginalia_adapters.real.vosk import VoskCommandRecognizer
from marginalia_core.application.command_router import CommandLexicon, load_command_lexicon
from marginalia_core.application.services.document_ingestion_service import (
    DocumentIngestionService,
)
from marginalia_core.application.services.note_service import NoteService
from marginalia_core.application.services.reader_service import ReaderService
from marginalia_core.application.services.reading_runtime_service import ReadingRuntimeService
from marginalia_core.application.services.rewrite_service import RewriteService
from marginalia_core.application.services.search_service import SearchService
from marginalia_core.application.services.session_query_service import SessionQueryService
from marginalia_core.application.services.summary_service import SummaryService
from marginalia_core.ports.llm import RewriteGenerator, TopicSummarizer
from marginalia_core.ports.playback import PlaybackEngine
from marginalia_core.ports.runtime import RuntimeSupervisor
from marginalia_core.ports.stt import CommandRecognizer, DictationTranscriber
from marginalia_core.ports.tts import SpeechSynthesizer
from marginalia_infra.config.settings import AppSettings
from marginalia_infra.events import InMemoryEventBus
from marginalia_infra.logging.setup import configure_logging
from marginalia_infra.runtime.session_supervisor import FileRuntimeSupervisor
from marginalia_infra.storage.cache import cleanup_audio_cache
from marginalia_infra.storage.sqlite import (
    SQLiteDatabase,
    SQLiteDocumentRepository,
    SQLiteNoteRepository,
    SQLiteRewriteDraftRepository,
    SQLiteSessionRepository,
)

logger = logging.getLogger(__name__)


@dataclass(slots=True)
class CliContainer:
    """Runtime object graph for CLI commands."""

    settings: AppSettings
    database: SQLiteDatabase
    event_bus: InMemoryEventBus
    command_lexicon: CommandLexicon
    runtime_supervisor: RuntimeSupervisor
    command_stt: CommandRecognizer
    dictation_stt: DictationTranscriber
    speech_synthesizer: SpeechSynthesizer
    playback_engine: PlaybackEngine
    rewrite_provider: RewriteGenerator
    summary_provider: TopicSummarizer
    ingestion_service: DocumentIngestionService
    reader_service: ReaderService
    reading_runtime_service: ReadingRuntimeService
    note_service: NoteService
    rewrite_service: RewriteService
    summary_service: SummaryService
    search_service: SearchService
    session_query_service: SessionQueryService


def build_container(config_path: Path | None = None, *, verbose: bool = False) -> CliContainer:
    """Construct the service graph for a CLI invocation."""

    settings = AppSettings.load(config_path=config_path)
    settings.ensure_directories()
    configure_logging(
        level="DEBUG" if verbose else settings.log_level,
        log_file=settings.log_file,
    )

    cleanup_audio_cache(settings.audio_cache_dir, max_age_hours=settings.audio_cache_max_age_hours)
    event_bus = InMemoryEventBus()
    database = SQLiteDatabase(settings.database_path)
    database.initialize()

    document_repository = SQLiteDocumentRepository(database)
    session_repository = SQLiteSessionRepository(database)
    note_repository = SQLiteNoteRepository(database)
    draft_repository = SQLiteRewriteDraftRepository(database)

    stale_count = session_repository.deactivate_stale_sessions(
        max_inactive_hours=settings.session_max_inactive_hours,
    )
    if stale_count:
        logger.info("Deactivated %d stale session(s)", stale_count)

    provider_checks = settings.doctor_report()["provider_checks"]
    command_lexicon = _load_runtime_command_lexicon(settings)
    runtime_supervisor = FileRuntimeSupervisor(settings.runtime_dir / "active-session.json")
    command_stt = _build_command_recognizer(settings, provider_checks, command_lexicon)
    dictation_stt = FakeDictationTranscriber(transcript=settings.fake_dictation_text)
    tts = _build_speech_synthesizer(settings, provider_checks)
    playback = _build_playback_engine(settings, provider_checks)
    rewrite_generator = FakeRewriteGenerator()
    topic_summarizer = FakeTopicSummarizer()
    ingestion_service = DocumentIngestionService(
        document_repository=document_repository,
        event_publisher=event_bus,
        chunk_target_chars=settings.chunk_target_chars,
    )
    reader_service = ReaderService(
        document_repository=document_repository,
        session_repository=session_repository,
        playback_engine=playback,
        speech_synthesizer=tts,
        event_publisher=event_bus,
        command_recognizer=command_stt,
        command_lexicon=command_lexicon,
        default_voice=settings.default_voice,
    )
    reading_runtime_service = ReadingRuntimeService(
        document_repository=document_repository,
        session_repository=session_repository,
        ingestion_service=ingestion_service,
        reader_service=reader_service,
        command_recognizer=command_stt,
        runtime_supervisor=runtime_supervisor,
        command_lexicon=command_lexicon,
    )
    note_service = NoteService(
        session_repository=session_repository,
        note_repository=note_repository,
        dictation_transcriber=dictation_stt,
        event_publisher=event_bus,
    )
    rewrite_service = RewriteService(
        session_repository=session_repository,
        note_repository=note_repository,
        draft_repository=draft_repository,
        document_repository=document_repository,
        rewrite_generator=rewrite_generator,
        event_publisher=event_bus,
    )
    summary_service = SummaryService(
        document_repository=document_repository,
        topic_summarizer=topic_summarizer,
        event_publisher=event_bus,
    )
    search_service = SearchService(
        document_repository=document_repository,
        note_repository=note_repository,
    )
    session_query_service = SessionQueryService(
        session_repository=session_repository,
        document_repository=document_repository,
        note_repository=note_repository,
        draft_repository=draft_repository,
        playback_engine=playback,
    )

    return CliContainer(
        settings=settings,
        database=database,
        event_bus=event_bus,
        command_lexicon=command_lexicon,
        runtime_supervisor=runtime_supervisor,
        command_stt=command_stt,
        dictation_stt=dictation_stt,
        speech_synthesizer=tts,
        playback_engine=playback,
        rewrite_provider=rewrite_generator,
        summary_provider=topic_summarizer,
        ingestion_service=ingestion_service,
        reader_service=reader_service,
        reading_runtime_service=reading_runtime_service,
        note_service=note_service,
        rewrite_service=rewrite_service,
        summary_service=summary_service,
        search_service=search_service,
        session_query_service=session_query_service,
    )


def _build_command_recognizer(
    settings: AppSettings,
    provider_checks: dict[str, Any],
    command_lexicon: CommandLexicon,
) -> CommandRecognizer:
    provider_name = settings.command_stt_provider
    if provider_name == "vosk":
        if provider_checks["vosk"]["ready"] or not settings.allow_provider_fallback:
            logger.info("Command STT: using real Vosk recognizer")
            return VoskCommandRecognizer(
                model_path=settings.vosk_model_path,
                commands=command_lexicon.grammar,
                sample_rate=settings.vosk_sample_rate,
                timeout_seconds=settings.vosk_listen_timeout_seconds,
            )
        logger.warning("Command STT: Vosk requested but not ready, falling back to fake")
    else:
        logger.info("Command STT: using fake provider (configured=%s)", provider_name)
    return FakeCommandRecognizer(commands=settings.fake_command_script)


def _build_speech_synthesizer(
    settings: AppSettings,
    provider_checks: dict[str, Any],
) -> SpeechSynthesizer:
    provider_name = settings.tts_provider
    if provider_name == "kokoro":
        if provider_checks["kokoro"]["ready"] or not settings.allow_provider_fallback:
            logger.info("TTS: using real Kokoro synthesizer")
            return KokoroSpeechSynthesizer(
                python_executable=settings.kokoro_python_executable,
                output_dir=settings.audio_cache_dir,
                lang_code=settings.kokoro_lang_code,
                speed=settings.kokoro_speed,
            )
        logger.warning("TTS: Kokoro requested but not ready, falling back to fake")
    if provider_name == "piper":
        if provider_checks["piper"]["ready"] or not settings.allow_provider_fallback:
            logger.info("TTS: using real Piper synthesizer")
            return PiperSpeechSynthesizer(
                executable=settings.piper_executable,
                model_path=settings.piper_model_path,
                output_dir=settings.audio_cache_dir,
                speaker_id=settings.piper_speaker_id,
                length_scale=settings.piper_length_scale,
                noise_scale=settings.piper_noise_scale,
            )
        logger.warning("TTS: Piper requested but not ready, falling back to fake")
    if provider_name not in ("kokoro", "piper"):
        logger.info("TTS: using fake provider (configured=%s)", provider_name)
    return FakeSpeechSynthesizer()


def _build_playback_engine(
    settings: AppSettings,
    provider_checks: dict[str, Any],
) -> PlaybackEngine:
    provider_name = settings.playback_provider
    if provider_name == "subprocess":
        if provider_checks["playback"]["ready"] or not settings.allow_provider_fallback:
            logger.info(
                "Playback: using real subprocess engine (command=%s)",
                settings.playback_command,
            )
            return SubprocessPlaybackEngine(command=settings.playback_command)
        logger.warning("Playback: subprocess requested but not ready, falling back to fake")
    else:
        logger.info("Playback: using fake provider (configured=%s)", provider_name)
    return FakePlaybackEngine(
        auto_complete_after_snapshots=settings.fake_playback_auto_complete_polls
    )


def _load_runtime_command_lexicon(settings: AppSettings) -> CommandLexicon:
    lexicon_path = settings.command_lexicon_dir / f"{settings.command_language}.toml"
    return load_command_lexicon(lexicon_path)
