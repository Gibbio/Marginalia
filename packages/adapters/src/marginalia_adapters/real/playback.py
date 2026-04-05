"""Real subprocess-backed playback adapter."""

from __future__ import annotations

import os
import shutil
import signal
import subprocess
import time
from pathlib import Path

from marginalia_core.domain.document import Document
from marginalia_core.domain.reading_session import PlaybackState, ReadingPosition
from marginalia_core.ports.capabilities import ProviderCapabilities
from marginalia_core.ports.playback import PlaybackSnapshot
from marginalia_core.ports.tts import SynthesisResult

SUBPROCESS_PLAYBACK_CAPABILITIES = ProviderCapabilities(
    provider_name="subprocess-playback",
    interface_kind="playback",
    supported_languages=("it", "en"),
    supports_streaming=False,
    supports_partial_results=False,
    supports_timestamps=False,
    low_latency_suitable=True,
    offline_capable=True,
)


class SubprocessPlaybackEngine:
    """Play local audio files through a macOS-friendly subprocess backend."""

    def __init__(self, *, command: str = "afplay") -> None:
        self._command = command
        self._state = PlaybackState.STOPPED
        self._last_document_id: str | None = None
        self._last_position = ReadingPosition()
        self._last_action = "stopped"
        self._progress_units = 0
        self._audio_reference: str | None = None
        self._process_id: int | None = None

    def describe_capabilities(self) -> ProviderCapabilities:
        return SUBPROCESS_PLAYBACK_CAPABILITIES

    def hydrate(self, snapshot: PlaybackSnapshot | None) -> None:
        if snapshot is None:
            return
        self._state = snapshot.state
        self._last_document_id = snapshot.document_id
        if snapshot.anchor:
            self._last_position = _position_from_anchor(snapshot.anchor)
        self._last_action = snapshot.last_action
        self._progress_units = snapshot.progress_units
        self._audio_reference = snapshot.audio_reference
        self._process_id = snapshot.process_id

    def start(
        self,
        document: Document,
        position: ReadingPosition,
        *,
        synthesis: SynthesisResult | None = None,
    ) -> PlaybackSnapshot:
        command_path = shutil.which(self._command)
        if command_path is None:
            raise RuntimeError(
                f"Playback command '{self._command}' is not available in the local environment."
            )
        if synthesis is None or not synthesis.audio_reference:
            raise RuntimeError("Playback requires a synthesized local audio artifact.")

        self.stop()
        audio_path = Path(synthesis.audio_reference)
        if not audio_path.exists():
            raise RuntimeError(f"Audio artifact '{audio_path}' does not exist.")

        process = subprocess.Popen(
            [command_path, str(audio_path)],
            stdin=subprocess.DEVNULL,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            start_new_session=True,
        )
        self._last_document_id = document.document_id
        self._last_position = position
        self._state = PlaybackState.PLAYING
        self._last_action = "start"
        self._progress_units += 1
        self._audio_reference = synthesis.audio_reference
        self._process_id = process.pid
        return self.snapshot()

    def pause(self) -> PlaybackSnapshot:
        self._refresh_state()
        if self._process_id is not None and self._state is PlaybackState.PLAYING:
            os.kill(self._process_id, signal.SIGSTOP)
            self._state = PlaybackState.PAUSED
        self._last_action = "pause"
        return self.snapshot()

    def resume(self) -> PlaybackSnapshot:
        self._refresh_state()
        if self._process_id is not None and self._state is PlaybackState.PAUSED:
            os.kill(self._process_id, signal.SIGCONT)
            self._state = PlaybackState.PLAYING
            self._progress_units += 1
        self._last_action = "resume"
        return self.snapshot()

    def stop(self) -> PlaybackSnapshot:
        if self._process_id is not None and _process_exists(self._process_id):
            os.kill(self._process_id, signal.SIGTERM)
            for _ in range(10):
                if not _process_exists(self._process_id):
                    break
                time.sleep(0.05)
            if _process_exists(self._process_id):
                os.kill(self._process_id, signal.SIGKILL)
        self._state = PlaybackState.STOPPED
        self._last_action = "stop"
        self._process_id = None
        return self.snapshot()

    def seek(self, position: ReadingPosition) -> PlaybackSnapshot:
        self.stop()
        self._last_position = position
        self._state = PlaybackState.PAUSED
        self._last_action = "seek"
        return self.snapshot()

    def snapshot(self) -> PlaybackSnapshot:
        self._refresh_state()
        return PlaybackSnapshot(
            state=self._state,
            last_action=self._last_action,
            document_id=self._last_document_id,
            anchor=self._last_position.anchor,
            progress_units=self._progress_units,
            audio_reference=self._audio_reference,
            provider_name=SUBPROCESS_PLAYBACK_CAPABILITIES.provider_name,
            process_id=self._process_id,
        )

    def _refresh_state(self) -> None:
        if self._process_id is None:
            if self._state is not PlaybackState.PAUSED:
                self._state = PlaybackState.STOPPED
            return
        if _process_exists(self._process_id):
            return
        if self._state is PlaybackState.PLAYING:
            self._last_action = "completed"
        self._state = PlaybackState.STOPPED
        self._process_id = None


def _position_from_anchor(anchor: str) -> ReadingPosition:
    section_index = 0
    chunk_index = 0
    for item in anchor.split("/"):
        key, _, raw_value = item.partition(":")
        if key == "section":
            section_index = int(raw_value)
        elif key == "chunk":
            chunk_index = int(raw_value)
    return ReadingPosition(section_index=section_index, chunk_index=chunk_index)


def _process_exists(process_id: int) -> bool:
    try:
        os.kill(process_id, 0)
    except OSError:
        return False
    return True
