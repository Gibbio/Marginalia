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


def test_fake_command_recognizer_returns_structured_command() -> None:
    recognizer = FakeCommandRecognizer(commands=("pause", "resume"))

    first = recognizer.listen_for_command()
    second = recognizer.listen_for_command()
    third = recognizer.listen_for_command()

    assert first is not None
    assert first.command == "pause"
    assert second is not None
    assert second.command == "resume"
    assert third is None
    assert recognizer.describe_capabilities().low_latency_suitable is True


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
