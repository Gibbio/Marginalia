"""Real Kokoro-based text-to-speech adapter."""

from __future__ import annotations

import shutil
import subprocess
from hashlib import sha1
from pathlib import Path

from marginalia_core.ports.capabilities import ProviderCapabilities
from marginalia_core.ports.tts import SynthesisRequest, SynthesisResult

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
    """Synthesize speech through a dedicated Kokoro Python runtime."""

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

    def describe_capabilities(self) -> ProviderCapabilities:
        return KOKORO_CAPABILITIES

    def synthesize(self, request: SynthesisRequest) -> SynthesisResult:
        python_path = shutil.which(self._python_executable)
        if python_path is None:
            raise RuntimeError(
                f"Kokoro python executable '{self._python_executable}' is not available."
            )
        if not self._worker_script.exists():
            raise RuntimeError(f"Kokoro worker script '{self._worker_script}' is missing.")

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
            command = [
                python_path,
                str(self._worker_script),
                "--output-path",
                str(output_path),
                "--lang-code",
                self._lang_code,
                "--voice",
                selected_voice,
                "--speed",
                str(self._speed),
            ]
            try:
                subprocess.run(
                    command,
                    input=request.text.encode("utf-8"),
                    capture_output=True,
                    check=True,
                )
            except subprocess.CalledProcessError as exc:
                stderr = exc.stderr.decode("utf-8", errors="replace").strip()
                stdout = exc.stdout.decode("utf-8", errors="replace").strip()
                details = stderr or stdout or "unknown Kokoro worker failure"
                raise RuntimeError(f"Kokoro synthesis failed: {details}") from exc

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
                "python_executable": python_path,
            },
        )


def _default_voice_for_lang_code(lang_code: str) -> str:
    if lang_code == "i":
        return "if_sara"
    return "af_heart"
