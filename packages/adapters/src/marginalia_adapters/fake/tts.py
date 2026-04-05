"""Fake TTS adapter."""

from __future__ import annotations


class FakeSpeechSynthesizer:
    """Return a deterministic byte payload instead of audio synthesis."""

    def synthesize(self, text: str, *, voice: str | None = None) -> bytes:
        selected_voice = voice or "marginalia-default"
        return f"FAKE-AUDIO::{selected_voice}::{text[:80]}".encode()
