"""Fake TTS adapter."""

from __future__ import annotations


class FakeSpeechSynthesizer:
    """Return a deterministic byte payload instead of audio synthesis."""

    def synthesize(self, text: str, *, voice: str | None = None) -> bytes:
        del voice
        return f"FAKE-AUDIO::{text[:80]}".encode("utf-8")
