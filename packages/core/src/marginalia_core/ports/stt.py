"""Speech-to-text ports and result models."""

from __future__ import annotations

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


class CommandRecognizer(Protocol):
    """Recognize short command-style utterances."""

    def describe_capabilities(self) -> ProviderCapabilities:
        """Describe recognizer behavior and constraints."""
        ...

    def listen_for_command(self) -> CommandRecognition | None:
        """Return the next recognized command if one is available."""
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
