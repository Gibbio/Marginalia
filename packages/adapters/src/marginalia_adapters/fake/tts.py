"""Fake TTS adapter."""

from __future__ import annotations

from hashlib import sha1

from marginalia_core.ports.capabilities import ProviderCapabilities
from marginalia_core.ports.tts import SynthesisRequest, SynthesisResult

TTS_CAPABILITIES = ProviderCapabilities(
    provider_name="fake-tts",
    interface_kind="speech-synthesizer",
    supported_languages=("en",),
    supports_streaming=False,
    supports_partial_results=False,
    supports_timestamps=False,
    low_latency_suitable=True,
    offline_capable=True,
)


class FakeSpeechSynthesizer:
    """Return deterministic synthesis metadata instead of real audio."""

    def describe_capabilities(self) -> ProviderCapabilities:
        return TTS_CAPABILITIES

    def synthesize(self, request: SynthesisRequest) -> SynthesisResult:
        selected_voice = request.voice or "marginalia-default"
        payload = f"FAKE-AUDIO::{selected_voice}::{request.language}::{request.text}".encode()
        digest = sha1(payload).hexdigest()[:16]
        return SynthesisResult(
            provider_name=TTS_CAPABILITIES.provider_name,
            voice=selected_voice,
            content_type="audio/fake",
            audio_reference=f"fake-audio:{digest}",
            byte_length=len(payload),
            text_excerpt=request.text[:120],
            metadata={"language": request.language},
        )
