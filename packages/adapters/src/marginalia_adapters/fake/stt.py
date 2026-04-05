"""Fake STT adapters."""

from __future__ import annotations

from collections import deque
from collections.abc import Sequence

from marginalia_core.ports.capabilities import ProviderCapabilities
from marginalia_core.ports.stt import (
    CommandRecognition,
    DictationSegment,
    DictationTranscript,
)

COMMAND_STT_CAPABILITIES = ProviderCapabilities(
    provider_name="fake-command-stt",
    interface_kind="command-stt",
    supported_languages=("en",),
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
