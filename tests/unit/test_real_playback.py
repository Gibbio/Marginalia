"""Real playback adapter tests."""

from __future__ import annotations

import subprocess
from pathlib import Path

from pytest import MonkeyPatch

from marginalia_adapters.real.playback import SubprocessPlaybackEngine
from marginalia_core.domain.document import build_document_outline
from marginalia_core.domain.reading_session import PlaybackState, ReadingPosition
from marginalia_core.ports.tts import SynthesisResult


class _FakeProcess:
    def __init__(self) -> None:
        self.pid = 43210
        self._poll_count = 0

    def poll(self) -> int | None:
        self._poll_count += 1
        if self._poll_count < 3:
            return None
        return 0

    def terminate(self) -> None:
        return None

    def kill(self) -> None:
        return None

    def wait(self, timeout: float | None = None) -> int:
        return 0


def test_subprocess_playback_engine_marks_completed_process_as_stopped(
    tmp_path: Path,
    monkeypatch: MonkeyPatch,
) -> None:
    document = build_document_outline(
        source_path=Path("sample.md"),
        raw_text="# Chapter One\n\nExample paragraph.",
    )
    audio_path = tmp_path / "chunk.wav"
    audio_path.write_bytes(b"fake-audio")
    process = _FakeProcess()

    monkeypatch.setattr("shutil.which", lambda command: f"/usr/bin/{command}")
    monkeypatch.setattr(
        subprocess,
        "Popen",
        lambda *args, **kwargs: process,
    )

    engine = SubprocessPlaybackEngine(command="afplay")
    start_snapshot = engine.start(
        document,
        ReadingPosition(),
        synthesis=SynthesisResult(
            provider_name="fake-tts",
            voice="narrator",
            content_type="audio/wav",
            audio_reference=str(audio_path),
            byte_length=audio_path.stat().st_size,
            text_excerpt="Example paragraph.",
        ),
    )
    still_playing = engine.snapshot()
    completed_snapshot = engine.snapshot()

    assert start_snapshot.state is PlaybackState.PLAYING
    assert still_playing.state is PlaybackState.PLAYING
    assert completed_snapshot.state is PlaybackState.STOPPED
    assert completed_snapshot.last_action == "completed"
