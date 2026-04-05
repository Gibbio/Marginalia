"""Text-to-speech ports."""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Protocol

from marginalia_core.ports.capabilities import ProviderCapabilities


@dataclass(frozen=True, slots=True)
class SynthesisRequest:
    """Text that should be converted into a playable representation."""

    text: str
    voice: str | None = None
    language: str = "en"


@dataclass(frozen=True, slots=True)
class SynthesisResult:
    """Structured synthesis output for downstream playback or reporting."""

    provider_name: str
    voice: str
    content_type: str
    audio_reference: str
    byte_length: int
    text_excerpt: str
    metadata: dict[str, str] = field(default_factory=dict)


class SpeechSynthesizer(Protocol):
    """Convert text to an audio payload."""

    def describe_capabilities(self) -> ProviderCapabilities:
        """Describe synthesizer behavior and constraints."""
        ...

    def synthesize(self, request: SynthesisRequest) -> SynthesisResult:
        """Return synthetic audio metadata for the given text."""
        ...
