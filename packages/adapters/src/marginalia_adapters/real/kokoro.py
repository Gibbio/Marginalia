"""Real Kokoro-based text-to-speech adapter."""

from __future__ import annotations

import json
import logging
import shutil
import subprocess
from hashlib import sha1
from io import BufferedWriter
from pathlib import Path
from typing import Any

from marginalia_core.ports.capabilities import ProviderCapabilities
from marginalia_core.ports.tts import SynthesisRequest, SynthesisResult

logger = logging.getLogger(__name__)

KOKORO_CAPABILITIES = ProviderCapabilities(
    provider_name="kokoro",
    interface_kind="speech-synthesizer",
    supported_languages=("en", "it", "es", "fr", "hi", "ja", "pt", "zh"),
    supports_streaming=False,
    supports_partial_results=False,
    supports_timestamps=False,
    low_latency_suitable=True,
    offline_capable=False,
)


class KokoroSpeechSynthesizer:
    """Synthesize speech through a dedicated Kokoro Python runtime.

    On the first synthesis call, a persistent worker process is spawned.
    The Kokoro model is loaded once and reused for all subsequent requests,
    avoiding the ~3s cold start per chunk.
    """

    def __init__(
        self,
        *,
        python_executable: str,
        output_dir: Path,
        lang_code: str = "i",
        speed: float = 1.0,
        worker_script: Path | None = None,
    ) -> None:
        self._python_executable = python_executable
        self._output_dir = output_dir
        self._lang_code = lang_code
        self._speed = speed
        self._worker_script = worker_script or Path(__file__).with_name("kokoro_worker.py")
        self._process: subprocess.Popen[bytes] | None = None
        self._stdin: BufferedWriter | None = None

    def describe_capabilities(self) -> ProviderCapabilities:
        return KOKORO_CAPABILITIES

    def synthesize(self, request: SynthesisRequest) -> SynthesisResult:
        self._output_dir.mkdir(parents=True, exist_ok=True)
        selected_voice = request.voice or _default_voice_for_lang_code(self._lang_code)
        cache_key = sha1(
            (
                f"{selected_voice}:{self._lang_code}:{self._speed}:{request.language}:"
                f"{request.text}"
            ).encode()
        ).hexdigest()
        output_path = self._output_dir / f"{cache_key}.wav"

        if not output_path.exists():
            self._ensure_worker()
            self._send_request(request.text, selected_voice, output_path)

        return SynthesisResult(
            provider_name=KOKORO_CAPABILITIES.provider_name,
            voice=selected_voice,
            content_type="audio/wav",
            audio_reference=str(output_path),
            byte_length=output_path.stat().st_size,
            text_excerpt=request.text[:120],
            metadata={
                "language": request.language,
                "lang_code": self._lang_code,
                "python_executable": self._python_executable,
            },
        )

    def shutdown(self) -> None:
        if self._process is not None:
            try:
                if self._process.stdin:
                    self._process.stdin.close()
                self._process.wait(timeout=5)
            except Exception:
                self._process.kill()
                self._process.wait()
            self._process = None
            self._stdin = None

    def _ensure_worker(self) -> None:
        if self._process is not None and self._process.poll() is None:
            return

        python_path = shutil.which(self._python_executable)
        if python_path is None:
            raise RuntimeError(
                f"Kokoro python executable '{self._python_executable}' is not available."
            )
        if not self._worker_script.exists():
            raise RuntimeError(f"Kokoro worker script '{self._worker_script}' is missing.")

        logger.info("Spawning persistent Kokoro worker (lang=%s)", self._lang_code)
        self._process = subprocess.Popen(
            [
                python_path,
                str(self._worker_script),
                "--serve",
                "--lang-code",
                self._lang_code,
                "--voice",
                _default_voice_for_lang_code(self._lang_code),
                "--speed",
                str(self._speed),
            ],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        assert self._process.stdout is not None
        ready = self._read_json_message("start")
        if ready.get("status") != "ready":
            raise RuntimeError(f"Kokoro worker unexpected ready response: {ready}")
        logger.info("Kokoro worker ready (pid=%d)", self._process.pid)

    def _send_request(self, text: str, voice: str, output_path: Path) -> None:
        assert self._process is not None
        assert self._process.stdin is not None
        assert self._process.stdout is not None

        request = json.dumps({
            "text": text,
            "voice": voice,
            "speed": self._speed,
            "output_path": str(output_path),
        })
        try:
            self._process.stdin.write((request + "\n").encode("utf-8"))
            self._process.stdin.flush()
        except BrokenPipeError:
            self._process = None
            raise RuntimeError("Kokoro worker process died unexpectedly.")

        response = self._read_json_message("synthesis response")
        if response.get("status") != "ok":
            raise RuntimeError(f"Kokoro synthesis failed: {response.get('message', 'unknown')}")

    def _read_json_message(self, context: str) -> dict[str, Any]:
        assert self._process is not None
        assert self._process.stdout is not None

        skipped_lines: list[str] = []
        while True:
            response_line = self._process.stdout.readline()
            if not response_line:
                stderr = ""
                if self._process.stderr:
                    stderr = self._process.stderr.read().decode("utf-8", errors="replace")
                self._process = None
                skipped_suffix = ""
                if skipped_lines:
                    skipped_suffix = f" Ignored stdout: {' | '.join(skipped_lines)}"
                raise RuntimeError(
                    f"Kokoro worker closed unexpectedly during {context}: {stderr}{skipped_suffix}"
                )

            decoded_line = response_line.decode("utf-8", errors="replace").strip()
            if not decoded_line:
                continue
            try:
                return json.loads(decoded_line)
            except json.JSONDecodeError:
                skipped_lines.append(decoded_line)
                logger.warning("Ignoring non-JSON Kokoro worker stdout during %s: %s", context, decoded_line)


def _default_voice_for_lang_code(lang_code: str) -> str:
    if lang_code == "i":
        return "if_sara"
    return "af_heart"
