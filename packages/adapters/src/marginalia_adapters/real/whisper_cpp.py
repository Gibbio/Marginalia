"""Real whisper.cpp-based dictation transcriber.

Records audio from the microphone into a temporary WAV file, invokes the
whisper.cpp ``main`` binary as a subprocess, and parses the plain-text output
into a ``DictationTranscript``.

Requirements:
  - whisper.cpp compiled binary (``main`` or ``whisper-cli``)
  - A GGML model file (e.g. ``ggml-base.bin``)
  - ``sounddevice`` Python package (already required by Vosk)
"""

from __future__ import annotations

import logging
import shutil
import subprocess
import tempfile
import wave
from pathlib import Path

from marginalia_core.ports.capabilities import ProviderCapabilities
from marginalia_core.ports.stt import DictationSegment, DictationTranscript

logger = logging.getLogger(__name__)

WHISPER_CPP_CAPABILITIES = ProviderCapabilities(
    provider_name="whisper-cpp",
    interface_kind="dictation-stt",
    supported_languages=("it", "en", "es", "fr", "de", "pt", "ja", "zh"),
    supports_streaming=False,
    supports_partial_results=False,
    supports_timestamps=True,
    low_latency_suitable=False,
    offline_capable=True,
)

_DEFAULT_SAMPLE_RATE = 16_000
_DEFAULT_MAX_SECONDS = 120
_DEFAULT_SILENCE_THRESHOLD = 900
_DEFAULT_SILENCE_DURATION = 2.0


class WhisperCppDictationTranscriber:
    """Transcribe dictated notes via whisper.cpp on Apple Silicon."""

    def __init__(
        self,
        *,
        executable: str = "whisper-cpp",
        model_path: Path,
        language: str = "it",
        sample_rate: int = _DEFAULT_SAMPLE_RATE,
        max_record_seconds: int = _DEFAULT_MAX_SECONDS,
        silence_threshold: int = _DEFAULT_SILENCE_THRESHOLD,
        silence_duration: float = _DEFAULT_SILENCE_DURATION,
    ) -> None:
        self._executable = executable
        self._model_path = model_path
        self._language = language
        self._sample_rate = sample_rate
        self._max_record_seconds = max_record_seconds
        self._silence_threshold = silence_threshold
        self._silence_duration = silence_duration

    def describe_capabilities(self) -> ProviderCapabilities:
        return WHISPER_CPP_CAPABILITIES

    def transcribe(
        self,
        *,
        session_id: str | None = None,
        note_id: str | None = None,
    ) -> DictationTranscript:
        resolved_executable = shutil.which(self._executable)
        if resolved_executable is None:
            raise RuntimeError(
                f"whisper.cpp executable '{self._executable}' is not available on PATH."
            )
        if not self._model_path.exists():
            raise RuntimeError(
                f"whisper.cpp model '{self._model_path}' does not exist."
            )

        wav_path = self._record_audio()
        try:
            raw_text = self._run_whisper(resolved_executable, wav_path)
        finally:
            wav_path.unlink(missing_ok=True)

        text = raw_text.strip()
        logger.info(
            "Whisper transcription completed: %d chars (session=%s, note=%s)",
            len(text),
            session_id,
            note_id,
        )
        segment_end_ms = max(len(text.split()) * 480, 480)
        return DictationTranscript(
            text=text,
            provider_name=WHISPER_CPP_CAPABILITIES.provider_name,
            language=self._language,
            segments=(DictationSegment(text=text, start_ms=0, end_ms=segment_end_ms),),
            raw_text=raw_text,
        )

    def _record_audio(self) -> Path:
        """Record from the default microphone until silence or max duration."""

        try:
            import sounddevice as sd  # type: ignore[import-not-found]
        except ImportError as exc:
            raise RuntimeError(
                "whisper.cpp dictation requires the 'sounddevice' package."
            ) from exc

        import array
        import time

        logger.info(
            "Recording dictation (max %ds, silence after %.1fs)...",
            self._max_record_seconds,
            self._silence_duration,
        )

        frames: list[bytes] = []
        silence_start: float | None = None
        recording_done = False

        max_frames = self._max_record_seconds * self._sample_rate
        total_frames = 0
        block_size = 4000
        started_at = time.monotonic()

        def callback(
            indata: bytes,
            frame_count: int,
            time_info: object,
            status: object,
        ) -> None:
            nonlocal silence_start, recording_done, total_frames
            del time_info
            if status:
                logger.debug("Audio callback status: %s", status)
            frames.append(bytes(indata))
            total_frames += frame_count

            samples = array.array("h")
            samples.frombytes(indata)
            peak = max(abs(s) for s in samples) if samples else 0
            now = time.monotonic()

            if peak < self._silence_threshold:
                if silence_start is None:
                    silence_start = now
                elif now - silence_start >= self._silence_duration:
                    recording_done = True
            else:
                silence_start = None

            if total_frames >= max_frames:
                recording_done = True

        stream = sd.RawInputStream(
            samplerate=self._sample_rate,
            blocksize=block_size,
            dtype="int16",
            channels=1,
            callback=callback,
        )

        with stream:
            while not recording_done:
                sd.sleep(100)

        elapsed = time.monotonic() - started_at
        logger.info("Recorded %.1f seconds of audio (%d frames)", elapsed, total_frames)

        tmp = tempfile.NamedTemporaryFile(suffix=".wav", delete=False)
        tmp.close()
        wav_path = Path(tmp.name)
        with wave.open(str(wav_path), "wb") as wf:
            wf.setnchannels(1)
            wf.setsampwidth(2)  # int16 = 2 bytes
            wf.setframerate(self._sample_rate)
            wf.writeframes(b"".join(frames))

        return wav_path

    def _run_whisper(self, executable: str, wav_path: Path) -> str:
        """Invoke whisper.cpp and return the transcribed text."""

        command = [
            executable,
            "-m", str(self._model_path),
            "-f", str(wav_path),
            "-l", self._language,
            "--no-timestamps",
            "-nt",
        ]
        logger.debug("Running whisper.cpp: %s", " ".join(command))

        try:
            result = subprocess.run(
                command,
                capture_output=True,
                text=True,
                check=True,
                timeout=60,
            )
        except subprocess.TimeoutExpired as exc:
            raise RuntimeError("whisper.cpp transcription timed out after 60s") from exc
        except subprocess.CalledProcessError as exc:
            stderr = (exc.stderr or "").strip()
            raise RuntimeError(f"whisper.cpp failed: {stderr}") from exc

        # whisper.cpp outputs text to stdout, one line per segment
        lines = [
            line.strip()
            for line in result.stdout.splitlines()
            if line.strip() and not line.strip().startswith("[")
        ]
        return " ".join(lines)
