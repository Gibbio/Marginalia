"""Real local Vosk command recognizer."""

from __future__ import annotations

import json
import queue
import time
from pathlib import Path

from marginalia_core.ports.capabilities import ProviderCapabilities
from marginalia_core.ports.stt import CommandRecognition

VOSK_COMMAND_CAPABILITIES = ProviderCapabilities(
    provider_name="vosk-command-stt",
    interface_kind="command-stt",
    supported_languages=("it",),
    supports_streaming=True,
    supports_partial_results=True,
    supports_timestamps=False,
    low_latency_suitable=True,
    offline_capable=True,
)


class VoskCommandRecognizer:
    """Recognize a small local command grammar through Vosk."""

    def __init__(
        self,
        *,
        model_path: Path | None,
        commands: tuple[str, ...],
        sample_rate: int = 16_000,
        timeout_seconds: float = 4.0,
    ) -> None:
        self._model_path = model_path
        self._commands = commands
        self._sample_rate = sample_rate
        self._timeout_seconds = timeout_seconds
        self._model: object | None = None

    def describe_capabilities(self) -> ProviderCapabilities:
        return VOSK_COMMAND_CAPABILITIES

    def listen_for_command(self) -> CommandRecognition | None:
        if self._model_path is None or not self._model_path.exists():
            raise RuntimeError("Vosk model path is not configured or does not exist.")

        try:
            import sounddevice  # type: ignore[import-not-found]
            from vosk import KaldiRecognizer, Model  # type: ignore[import-not-found]
        except ImportError as exc:
            raise RuntimeError(
                "Vosk command recognition requires the 'vosk' and 'sounddevice' packages."
            ) from exc

        if self._model is None:
            self._model = Model(str(self._model_path))

        recognizer = KaldiRecognizer(
            self._model,
            self._sample_rate,
            json.dumps(list(self._commands), ensure_ascii=False),
        )
        audio_queue: queue.Queue[bytes] = queue.Queue()

        def callback(indata: bytes, frames: int, time_info: object, status: object) -> None:
            del frames, time_info
            if status:
                return
            audio_queue.put(bytes(indata))

        started_at = time.monotonic()
        try:
            with sounddevice.RawInputStream(
                samplerate=self._sample_rate,
                blocksize=8_000,
                dtype="int16",
                channels=1,
                callback=callback,
            ):
                while time.monotonic() - started_at < self._timeout_seconds:
                    try:
                        data = audio_queue.get(timeout=0.25)
                    except queue.Empty:
                        continue
                    if recognizer.AcceptWaveform(data):
                        text = _result_text(recognizer.Result())
                        if text:
                            return CommandRecognition(
                                command=text,
                                provider_name=VOSK_COMMAND_CAPABILITIES.provider_name,
                                confidence=1.0,
                                is_final=True,
                                raw_text=text,
                            )
                final_text = _result_text(recognizer.FinalResult())
        except Exception as exc:
            raise RuntimeError(f"Vosk command capture failed: {exc}") from exc

        if not final_text:
            return None
        return CommandRecognition(
            command=final_text,
            provider_name=VOSK_COMMAND_CAPABILITIES.provider_name,
            confidence=1.0,
            is_final=True,
            raw_text=final_text,
        )


def _result_text(raw_result: str) -> str:
    try:
        payload = json.loads(raw_result)
    except json.JSONDecodeError:
        return ""
    return str(payload.get("text", "")).strip().lower()
