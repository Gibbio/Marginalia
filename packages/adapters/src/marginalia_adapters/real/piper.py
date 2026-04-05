"""Real Piper-based text-to-speech adapter."""

from __future__ import annotations

import shutil
import subprocess
from hashlib import sha1
from pathlib import Path

from marginalia_core.ports.capabilities import ProviderCapabilities
from marginalia_core.ports.tts import SynthesisRequest, SynthesisResult

PIPER_CAPABILITIES = ProviderCapabilities(
    provider_name="piper",
    interface_kind="speech-synthesizer",
    supported_languages=("it", "en"),
    supports_streaming=False,
    supports_partial_results=False,
    supports_timestamps=False,
    low_latency_suitable=True,
    offline_capable=True,
)


class PiperSpeechSynthesizer:
    """Synthesize speech locally through the Piper CLI."""

    def __init__(
        self,
        *,
        executable: str,
        model_path: Path | None,
        output_dir: Path,
        speaker_id: int | None = None,
        length_scale: float = 1.0,
        noise_scale: float = 0.667,
    ) -> None:
        self._executable = executable
        self._model_path = model_path
        self._output_dir = output_dir
        self._speaker_id = speaker_id
        self._length_scale = length_scale
        self._noise_scale = noise_scale

    def describe_capabilities(self) -> ProviderCapabilities:
        return PIPER_CAPABILITIES

    def synthesize(self, request: SynthesisRequest) -> SynthesisResult:
        executable_path = shutil.which(self._executable)
        if executable_path is None:
            raise RuntimeError(
                f"Piper executable '{self._executable}' is not available in the local environment."
            )
        if self._model_path is None or not self._model_path.exists():
            raise RuntimeError("Piper model path is not configured or does not exist.")

        self._output_dir.mkdir(parents=True, exist_ok=True)
        selected_voice = request.voice or self._model_path.stem
        cache_key = sha1(
            (
                f"{self._model_path}:{selected_voice}:{request.language}:"
                f"{self._speaker_id}:{self._length_scale}:{self._noise_scale}:{request.text}"
            ).encode()
        ).hexdigest()
        output_path = self._output_dir / f"{cache_key}.wav"
        if not output_path.exists():
            command = [
                executable_path,
                "--model",
                str(self._model_path),
                "--output_file",
                str(output_path),
                "--length_scale",
                str(self._length_scale),
                "--noise_scale",
                str(self._noise_scale),
            ]
            if self._speaker_id is not None:
                command.extend(["--speaker", str(self._speaker_id)])
            subprocess.run(
                command,
                input=request.text.encode("utf-8"),
                capture_output=True,
                check=True,
            )

        return SynthesisResult(
            provider_name=PIPER_CAPABILITIES.provider_name,
            voice=selected_voice,
            content_type="audio/wav",
            audio_reference=str(output_path),
            byte_length=output_path.stat().st_size,
            text_excerpt=request.text[:120],
            metadata={
                "language": request.language,
                "model_path": str(self._model_path),
                "speaker_id": str(self._speaker_id) if self._speaker_id is not None else "",
            },
        )
