"""Real local Vosk command recognizer."""

from __future__ import annotations

import json
import queue
import time
from array import array
from collections.abc import Callable
from pathlib import Path
from typing import Any, Literal

from marginalia_core.ports.capabilities import ProviderCapabilities
from marginalia_core.ports.stt import (
    CommandRecognition,
    SpeechInterruptCapture,
    SpeechInterruptMonitor,
)

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
        input_device_index: int | None = None,
        input_device_name: str | None = None,
        speech_threshold: int = 900,
        silence_timeout_seconds: float = 0.8,
    ) -> None:
        self._model_path = model_path
        self._commands = commands
        self._sample_rate = sample_rate
        self._timeout_seconds = timeout_seconds
        self._input_device_index = input_device_index
        self._input_device_name = input_device_name
        self._speech_threshold = speech_threshold
        self._silence_timeout_seconds = silence_timeout_seconds
        self._model: object | None = None

    def describe_capabilities(self) -> ProviderCapabilities:
        return VOSK_COMMAND_CAPABILITIES

    def listen_for_command(self) -> CommandRecognition | None:
        capture = self.capture_interrupt()
        if not capture.recognized_command:
            return None
        return CommandRecognition(
            command=capture.recognized_command,
            provider_name=capture.provider_name,
            confidence=1.0,
            is_final=True,
            raw_text=capture.raw_text or capture.recognized_command,
        )

    def capture_interrupt(
        self,
        *,
        timeout_seconds: float | None = None,
        on_speech_start: Callable[[int], None] | None = None,
    ) -> SpeechInterruptCapture:
        with self.open_interrupt_monitor() as monitor:
            return monitor.capture_next_interrupt(
                timeout_seconds=timeout_seconds,
                on_speech_start=on_speech_start,
            )

    def open_interrupt_monitor(self) -> SpeechInterruptMonitor:
        dependencies = self._resolve_dependencies()
        return _VoskSpeechInterruptMonitor(
            model=dependencies["model"],
            recognizer_factory=dependencies["recognizer_factory"],
            sounddevice=dependencies["sounddevice"],
            commands=self._commands,
            sample_rate=self._sample_rate,
            timeout_seconds=self._timeout_seconds,
            input_device_index=self._input_device_index,
            input_device_name=self._input_device_name,
            speech_threshold=self._speech_threshold,
            silence_timeout_seconds=self._silence_timeout_seconds,
        )

    def _resolve_dependencies(self) -> dict[str, Any]:
        if self._model_path is None or not self._model_path.exists():
            raise RuntimeError("Vosk model path is not configured or does not exist.")

        try:
            import sounddevice  # type: ignore[import-untyped]
            from vosk import KaldiRecognizer, Model  # type: ignore[import-untyped]
        except ImportError as exc:
            raise RuntimeError(
                "Vosk command recognition requires the 'vosk' and 'sounddevice' packages."
            ) from exc

        if self._model is None:
            self._model = Model(str(self._model_path))
        return {
            "model": self._model,
            "recognizer_factory": KaldiRecognizer,
            "sounddevice": sounddevice,
        }


class _VoskSpeechInterruptMonitor:
    """Hold a Vosk input stream open across multiple interrupt captures."""

    def __init__(
        self,
        *,
        model: object,
        recognizer_factory: Any,
        sounddevice: Any,
        commands: tuple[str, ...],
        sample_rate: int,
        timeout_seconds: float,
        input_device_index: int | None,
        input_device_name: str | None,
        speech_threshold: int,
        silence_timeout_seconds: float,
    ) -> None:
        self._model = model
        self._recognizer_factory = recognizer_factory
        self._sounddevice = sounddevice
        self._commands = commands
        self._sample_rate = sample_rate
        self._timeout_seconds = timeout_seconds
        self._input_device = _resolve_input_device(
            sounddevice,
            requested_index=input_device_index,
            requested_name=input_device_name,
        )
        self._speech_threshold = speech_threshold
        self._silence_timeout_seconds = silence_timeout_seconds
        self._audio_queue: queue.Queue[bytes] = queue.Queue()
        self._stream: Any | None = None
        self._started_at: float | None = None

    def __enter__(self) -> _VoskSpeechInterruptMonitor:
        def callback(indata: bytes, frames: int, time_info: object, status: object) -> None:
            del frames, time_info
            if status:
                return
            self._audio_queue.put(bytes(indata))

        try:
            self._stream = self._sounddevice.RawInputStream(
                device=self._input_device["index"],
                samplerate=self._sample_rate,
                blocksize=8_000,
                dtype="int16",
                channels=1,
                callback=callback,
            )
            self._stream.__enter__()
        except Exception as exc:
            raise RuntimeError(f"Vosk command capture failed: {exc}") from exc
        self._started_at = time.monotonic()
        return self

    def __exit__(self, exc_type: object, exc: object, tb: object) -> Literal[False]:
        self.close()
        return False

    def capture_next_interrupt(
        self,
        *,
        timeout_seconds: float | None = None,
        on_speech_start: Callable[[int], None] | None = None,
    ) -> SpeechInterruptCapture:
        if self._stream is None or self._started_at is None:
            raise RuntimeError("Vosk interrupt monitor must be opened before capture.")

        self._drain_audio_queue()
        recognizer = self._recognizer_factory(
            self._model,
            self._sample_rate,
            json.dumps(list(self._commands), ensure_ascii=False),
        )
        effective_timeout = (
            timeout_seconds if timeout_seconds is not None else self._timeout_seconds
        )
        attempt_started_at = time.monotonic()
        detected_offset_ms: int | None = None
        capture_started_ms: int | None = None
        capture_ended_ms = 0
        recognized_text: str | None = None
        end_reason = "timeout"
        silence_started_at: float | None = None

        while time.monotonic() - attempt_started_at < effective_timeout:
            try:
                data = self._audio_queue.get(timeout=0.25)
            except queue.Empty:
                continue
            now = time.monotonic()
            if _audio_peak(data) >= self._speech_threshold:
                silence_started_at = None
                if detected_offset_ms is None:
                    detected_offset_ms = _elapsed_ms(attempt_started_at, now)
                    capture_started_ms = detected_offset_ms
                    if on_speech_start is not None:
                        on_speech_start(detected_offset_ms)
            elif detected_offset_ms is not None:
                if silence_started_at is None:
                    silence_started_at = now
                elif now - silence_started_at >= self._silence_timeout_seconds:
                    end_reason = "silence"
                    break
            if recognizer.AcceptWaveform(data):
                text = _result_text(recognizer.Result())
                if text:
                    recognized_text = text
                    end_reason = "recognized"
                    capture_ended_ms = _elapsed_ms(attempt_started_at, now)
                    break
        else:
            end_reason = "timeout"

        if capture_ended_ms == 0:
            capture_ended_ms = _elapsed_ms(attempt_started_at, time.monotonic())
        final_text = _result_text(recognizer.FinalResult())
        if recognized_text is None:
            recognized_text = final_text or None
        return SpeechInterruptCapture(
            provider_name=VOSK_COMMAND_CAPABILITIES.provider_name,
            speech_detected=detected_offset_ms is not None,
            speech_detected_ms=detected_offset_ms,
            capture_started_ms=capture_started_ms,
            capture_ended_ms=capture_ended_ms,
            recognized_command=recognized_text,
            raw_text=recognized_text,
            timed_out=end_reason == "timeout",
            input_device_index=self._input_device["index"],
            input_device_name=self._input_device["name"],
            sample_rate=self._sample_rate,
        )

    def close(self) -> None:
        if self._stream is None:
            return
        stream = self._stream
        self._stream = None
        try:
            stream.__exit__(None, None, None)
        except Exception:
            return

    def _drain_audio_queue(self) -> None:
        while True:
            try:
                self._audio_queue.get_nowait()
            except queue.Empty:
                return


def _result_text(raw_result: str) -> str:
    try:
        payload = json.loads(raw_result)
    except json.JSONDecodeError:
        return ""
    return str(payload.get("text", "")).strip().lower()


def _resolve_input_device(
    sounddevice: Any,
    *,
    requested_index: int | None = None,
    requested_name: str | None = None,
) -> dict[str, Any]:
    try:
        devices = list(sounddevice.query_devices())
    except Exception as exc:  # pragma: no cover - delegated to PortAudio
        raise RuntimeError(f"Unable to query audio input devices: {exc}") from exc

    if requested_index is not None:
        if requested_index < 0 or requested_index >= len(devices):
            raise RuntimeError(f"Configured Vosk input device index {requested_index} is invalid.")
        if _device_input_channels(devices[requested_index]) <= 0:
            raise RuntimeError(
                f"Configured Vosk input device index {requested_index} has no input channels."
            )
        return {"index": requested_index, "name": _device_name(devices[requested_index])}

    if requested_name:
        normalized_requested = requested_name.strip().lower()
        for index, device in enumerate(devices):
            if _device_input_channels(device) <= 0:
                continue
            device_name = _device_name(device)
            normalized_device_name = device_name.lower()
            if (
                normalized_requested == normalized_device_name
                or normalized_requested in normalized_device_name
            ):
                return {"index": index, "name": device_name}
        raise RuntimeError(
            "Configured Vosk input device "
            f"'{requested_name}' was not found among local input devices."
        )

    default_input_device = _default_input_device_index(sounddevice)
    if default_input_device is not None and default_input_device < len(devices):
        if _device_input_channels(devices[default_input_device]) > 0:
            return {
                "index": default_input_device,
                "name": _device_name(devices[default_input_device]),
            }

    for index, device in enumerate(devices):
        if _device_input_channels(device) > 0:
            return {"index": index, "name": _device_name(device)}

    raise RuntimeError(
        "No input audio device is available for Vosk command capture. "
        "Connect or enable a microphone in macOS Sound settings."
    )


def _default_input_device_index(sounddevice: Any) -> int | None:
    default = getattr(getattr(sounddevice, "default", None), "device", None)
    if isinstance(default, list | tuple):
        if not default:
            return None
        default = default[0]
    if default is None:
        return None
    try:
        value = int(str(default))
    except ValueError:
        return None
    return value if value >= 0 else None


def _device_input_channels(device: Any) -> int:
    if isinstance(device, dict):
        value = device.get("max_input_channels", 0)
    else:
        value = getattr(device, "max_input_channels", 0)
    try:
        return int(value)
    except (TypeError, ValueError):
        return 0


def _device_name(device: Any) -> str:
    if isinstance(device, dict):
        value = device.get("name", "")
    else:
        value = getattr(device, "name", "")
    return str(value)


def _audio_peak(data: bytes) -> int:
    samples = array("h")
    samples.frombytes(data)
    if not samples:
        return 0
    return max(abs(sample) for sample in samples)


def _elapsed_ms(started_at: float, now: float) -> int:
    return max(0, int((now - started_at) * 1000))
