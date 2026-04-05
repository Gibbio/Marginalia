"""Fake provider tests."""

from __future__ import annotations

from pathlib import Path

from marginalia_adapters.fake.llm import FakeRewriteGenerator, FakeTopicSummarizer
from marginalia_adapters.fake.playback import FakePlaybackEngine
from marginalia_adapters.fake.stt import FakeCommandRecognizer, FakeDictationTranscriber
from marginalia_adapters.fake.tts import FakeSpeechSynthesizer
from marginalia_core.domain.document import build_document_outline
from marginalia_core.domain.reading_session import PlaybackState, ReadingPosition
from marginalia_core.ports.llm import RewriteInstruction, SummaryInstruction
from marginalia_core.ports.tts import SynthesisRequest


def test_fake_command_recognizer_returns_structured_capture() -> None:
    recognizer = FakeCommandRecognizer(commands=("pause", "resume"))

    first = recognizer.capture_interrupt()
    second = recognizer.capture_interrupt()
    third = recognizer.capture_interrupt()

    assert first.recognized_command == "pause"
    assert second.recognized_command == "resume"
    assert third.timed_out is True
    assert recognizer.describe_capabilities().low_latency_suitable is True


def test_fake_command_recognizer_can_capture_speech_interrupt() -> None:
    recognizer = FakeCommandRecognizer(commands=("pausa",))
    detected_offsets: list[int] = []

    capture = recognizer.capture_interrupt(on_speech_start=detected_offsets.append)

    assert detected_offsets == [120]
    assert capture.speech_detected is True
    assert capture.recognized_command == "pausa"
    assert capture.capture_started_ms is not None
    assert capture.capture_ended_ms > capture.capture_started_ms


def test_fake_command_recognizer_can_keep_interrupt_monitor_open() -> None:
    recognizer = FakeCommandRecognizer(commands=("pausa", "continua"))

    with recognizer.open_interrupt_monitor() as monitor:
        first = monitor.capture_next_interrupt()
        second = monitor.capture_next_interrupt()

    assert first.recognized_command == "pausa"
    assert second.recognized_command == "continua"


def test_fake_dictation_transcriber_returns_timestamped_transcript() -> None:
    transcriber = FakeDictationTranscriber("Refine the pacing in this paragraph.")

    transcript = transcriber.transcribe(session_id="session-1", note_id="note-1")

    assert transcript.provider_name == "fake-dictation-stt"
    assert transcript.segments[0].start_ms == 0
    assert transcript.segments[0].end_ms > 0
    assert "session=session-1" in (transcript.raw_text or "")


def test_fake_speech_synthesizer_returns_structured_metadata() -> None:
    synthesizer = FakeSpeechSynthesizer()

    result = synthesizer.synthesize(SynthesisRequest(text="Read this aloud.", voice="narrator"))

    assert result.provider_name == "fake-tts"
    assert result.voice == "narrator"
    assert result.audio_reference.startswith("fake-audio:")
    assert result.byte_length > 0


def test_fake_playback_engine_reports_snapshots() -> None:
    document = build_document_outline(
        source_path=Path("sample.md"),
        raw_text="# Chapter One\n\nExample paragraph.",
    )
    engine = FakePlaybackEngine()

    start_snapshot = engine.start(document, ReadingPosition())
    pause_snapshot = engine.pause()
    resume_snapshot = engine.resume()

    assert start_snapshot.state is PlaybackState.PLAYING
    assert pause_snapshot.state is PlaybackState.PAUSED
    assert resume_snapshot.state is PlaybackState.PLAYING
    assert resume_snapshot.progress_units >= start_snapshot.progress_units


def test_fake_playback_engine_can_auto_complete_after_snapshot_polls() -> None:
    document = build_document_outline(
        source_path=Path("sample.md"),
        raw_text="# Chapter One\n\nExample paragraph.",
    )
    engine = FakePlaybackEngine(auto_complete_after_snapshots=0)

    engine.start(document, ReadingPosition())
    completed_snapshot = engine.snapshot()

    assert completed_snapshot.state is PlaybackState.STOPPED
    assert completed_snapshot.last_action == "completed"


def test_fake_llm_adapters_return_deterministic_outputs() -> None:
    rewrite_provider = FakeRewriteGenerator()
    summary_provider = FakeTopicSummarizer()

    rewrite_output = rewrite_provider.rewrite_section(
        RewriteInstruction(
            document_title="Sample",
            section_title="Chapter One",
            source_anchor="section:0/chunk:0",
            section_text="Original section text.",
            note_texts=("Make the tone sharper.",),
        )
    )
    summary_output = summary_provider.summarize_topic(
        SummaryInstruction(topic="local", matched_document_ids=("doc-1", "doc-2"))
    )

    assert rewrite_output.provider_name == "fake-rewrite-llm"
    assert rewrite_output.note_count == 1
    assert "Anchor: section:0/chunk:0" in rewrite_output.rewritten_text
    assert summary_output.provider_name == "fake-summary-llm"
    assert len(summary_output.highlights) == 3
