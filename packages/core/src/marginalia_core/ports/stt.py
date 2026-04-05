"""Speech-to-text ports and result models."""

from __future__ import annotations

from collections.abc import Callable
from dataclasses import dataclass
from typing import Protocol

from marginalia_core.ports.capabilities import ProviderCapabilities


@dataclass(frozen=True, slots=True)
class CommandRecognition:
    """Structured result for short command recognition."""

    command: str
    provider_name: str
    confidence: float = 1.0
    is_final: bool = True
    raw_text: str | None = None


@dataclass(frozen=True, slots=True)
class DictationSegment:
    """Single dictated segment with optional timing metadata."""

    text: str
    start_ms: int
    end_ms: int


@dataclass(frozen=True, slots=True)
class DictationTranscript:
    """Structured dictation transcript."""

    text: str
    provider_name: str
    language: str = "en"
    is_final: bool = True
    segments: tuple[DictationSegment, ...] = ()
    raw_text: str | None = None


@dataclass(frozen=True, slots=True)
class SpeechInterruptCapture:
    """Structured result for a short speech interrupt capture window."""

    provider_name: str
    speech_detected: bool
    capture_ended_ms: int
    speech_detected_ms: int | None = None
    capture_started_ms: int | None = None
    recognized_command: str | None = None
    raw_text: str | None = None
    timed_out: bool = False
    input_device_index: int | None = None
    input_device_name: str | None = None
    sample_rate: int | None = None


class SpeechInterruptMonitor(Protocol):
    """Long-lived capture handle for repeated speech interrupts."""

    def capture_next_interrupt(
        self,
        *,
        timeout_seconds: float | None = None,
        on_speech_start: Callable[[int], None] | None = None,
    ) -> SpeechInterruptCapture:
        """Capture the next interrupt while keeping the underlying input path open."""
        ...

    def close(self) -> None:
        """Release any held microphone or stream resources."""
        ...

    def __enter__(self) -> SpeechInterruptMonitor:
        """Open monitor resources for repeated capture."""
        ...

    def __exit__(self, exc_type: object, exc: object, tb: object) -> bool | None:
        """Close monitor resources on scope exit."""
        ...


class CommandRecognizer(Protocol):
    """Recognize short command-style utterances."""

    def describe_capabilities(self) -> ProviderCapabilities:
        """Describe recognizer behavior and constraints."""
        ...

    def listen_for_command(self) -> CommandRecognition | None:
        """Return the next recognized command if one is available."""
        ...

    def capture_interrupt(
        self,
        *,
        timeout_seconds: float | None = None,
        on_speech_start: Callable[[int], None] | None = None,
    ) -> SpeechInterruptCapture:
        """Capture a short interrupt window, optionally signalling speech onset."""
        ...

    def open_interrupt_monitor(self) -> SpeechInterruptMonitor:
        """Return a long-lived interrupt monitor that keeps the mic path open."""
        ...


class DictationTranscriber(Protocol):
    """Transcribe longer dictated note content."""

    def describe_capabilities(self) -> ProviderCapabilities:
        """Describe transcriber behavior and constraints."""
        ...

    def transcribe(
        self,
        *,
        session_id: str | None = None,
        note_id: str | None = None,
    ) -> DictationTranscript:
        """Return a transcript of the latest dictation input."""
        ...
