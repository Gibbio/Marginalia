"""Fake STT adapters."""

from __future__ import annotations

from collections import deque
from collections.abc import Callable, Sequence
from typing import Literal

from marginalia_core.ports.capabilities import ProviderCapabilities
from marginalia_core.ports.stt import (
    CommandRecognition,
    DictationSegment,
    DictationTranscript,
    SpeechInterruptCapture,
    SpeechInterruptMonitor,
)

COMMAND_STT_CAPABILITIES = ProviderCapabilities(
    provider_name="fake-command-stt",
    interface_kind="command-stt",
    supported_languages=("it", "en"),
    supports_streaming=False,
    supports_partial_results=False,
    supports_timestamps=False,
    low_latency_suitable=True,
    offline_capable=True,
)

DICTATION_STT_CAPABILITIES = ProviderCapabilities(
    provider_name="fake-dictation-stt",
    interface_kind="dictation-stt",
    supported_languages=("en",),
    supports_streaming=False,
    supports_partial_results=False,
    supports_timestamps=True,
    low_latency_suitable=False,
    offline_capable=True,
)


class FakeCommandRecognizer:
    """FIFO command recognizer for local testing and smoke flows."""

    def __init__(self, commands: Sequence[str] | None = None) -> None:
        self._commands = deque(commands or [])

    def describe_capabilities(self) -> ProviderCapabilities:
        return COMMAND_STT_CAPABILITIES

    def listen_for_command(self) -> CommandRecognition | None:
        if not self._commands:
            return None
        command = self._commands.popleft()
        return CommandRecognition(
            command=command,
            provider_name=COMMAND_STT_CAPABILITIES.provider_name,
            raw_text=command,
        )

    def capture_interrupt(
        self,
        *,
        timeout_seconds: float | None = None,
        on_speech_start: Callable[[int], None] | None = None,
    ) -> SpeechInterruptCapture:
        if not self._commands:
            return SpeechInterruptCapture(
                provider_name=COMMAND_STT_CAPABILITIES.provider_name,
                speech_detected=False,
                capture_ended_ms=int((timeout_seconds or 0.0) * 1000),
                timed_out=True,
                input_device_name="fake-input",
            )

        command = self._commands.popleft()
        detection_ms = 120
        if on_speech_start is not None:
            on_speech_start(detection_ms)
        return SpeechInterruptCapture(
            provider_name=COMMAND_STT_CAPABILITIES.provider_name,
            speech_detected=True,
            speech_detected_ms=detection_ms,
            capture_started_ms=detection_ms,
            capture_ended_ms=detection_ms + 240,
            recognized_command=command,
            raw_text=command,
            input_device_name="fake-input",
        )

    def open_interrupt_monitor(self) -> SpeechInterruptMonitor:
        return _FakeSpeechInterruptMonitor(self)


class FakeDictationTranscriber:
    """Deterministic dictation placeholder."""

    def __init__(
        self,
        transcript: str = "Placeholder dictated note.",
        *,
        language: str = "en",
    ) -> None:
        self._transcripts = deque([transcript])
        self._language = language

    def describe_capabilities(self) -> ProviderCapabilities:
        return DICTATION_STT_CAPABILITIES

    def transcribe(
        self,
        *,
        session_id: str | None = None,
        note_id: str | None = None,
    ) -> DictationTranscript:
        if len(self._transcripts) > 1:
            transcript = self._transcripts.popleft()
        else:
            transcript = self._transcripts[0]
        segment_end = max(len(transcript.split()) * 480, 480)
        raw_text = transcript
        if session_id or note_id:
            raw_text = (
                f"{transcript} " f"[session={session_id or 'unknown'} note={note_id or 'unknown'}]"
            )
        return DictationTranscript(
            text=transcript,
            provider_name=DICTATION_STT_CAPABILITIES.provider_name,
            language=self._language,
            segments=(DictationSegment(text=transcript, start_ms=0, end_ms=segment_end),),
            raw_text=raw_text,
        )

    def set_transcript(self, transcript: str) -> None:
        self._transcripts = deque([transcript])

    def queue_transcript(self, transcript: str) -> None:
        self._transcripts.append(transcript)


class _FakeSpeechInterruptMonitor:
    """Keep fake command capture alive across multiple interrupt attempts."""

    def __init__(self, recognizer: FakeCommandRecognizer) -> None:
        self._recognizer = recognizer

    def __enter__(self) -> _FakeSpeechInterruptMonitor:
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
        return self._recognizer.capture_interrupt(
            timeout_seconds=timeout_seconds,
            on_speech_start=on_speech_start,
        )

    def close(self) -> None:
        return None
