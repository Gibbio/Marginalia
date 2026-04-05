"""Fake playback engine."""

from __future__ import annotations

from marginalia_core.domain.document import Document
from marginalia_core.domain.reading_session import PlaybackState, ReadingPosition
from marginalia_core.ports.capabilities import ProviderCapabilities
from marginalia_core.ports.playback import PlaybackSnapshot
from marginalia_core.ports.tts import SynthesisResult

PLAYBACK_CAPABILITIES = ProviderCapabilities(
    provider_name="fake-playback",
    interface_kind="playback",
    supported_languages=("en",),
    supports_streaming=False,
    supports_partial_results=False,
    supports_timestamps=True,
    low_latency_suitable=True,
    offline_capable=True,
)


class FakePlaybackEngine:
    """Track playback commands without attempting real audio output."""

    def __init__(self) -> None:
        self.state = PlaybackState.STOPPED
        self.last_document_id: str | None = None
        self.last_position = ReadingPosition()
        self.last_action = "stopped"
        self.progress_units = 0
        self.audio_reference: str | None = None

    def describe_capabilities(self) -> ProviderCapabilities:
        return PLAYBACK_CAPABILITIES

    def hydrate(self, snapshot: PlaybackSnapshot | None) -> None:
        if snapshot is None:
            return
        self.state = snapshot.state
        self.last_document_id = snapshot.document_id
        if snapshot.anchor is not None:
            self.last_position = _position_from_anchor(snapshot.anchor)
        self.last_action = snapshot.last_action
        self.progress_units = snapshot.progress_units
        self.audio_reference = snapshot.audio_reference

    def start(
        self,
        document: Document,
        position: ReadingPosition,
        *,
        synthesis: SynthesisResult | None = None,
    ) -> PlaybackSnapshot:
        self.last_document_id = document.document_id
        self.last_position = position
        self.state = PlaybackState.PLAYING
        self.last_action = "start"
        self.progress_units += 1
        self.audio_reference = synthesis.audio_reference if synthesis else None
        return self.snapshot()

    def pause(self) -> PlaybackSnapshot:
        self.state = PlaybackState.PAUSED
        self.last_action = "pause"
        return self.snapshot()

    def resume(self) -> PlaybackSnapshot:
        self.state = PlaybackState.PLAYING
        self.last_action = "resume"
        self.progress_units += 1
        return self.snapshot()

    def stop(self) -> PlaybackSnapshot:
        self.state = PlaybackState.STOPPED
        self.last_action = "stop"
        self.audio_reference = None
        return self.snapshot()

    def seek(self, position: ReadingPosition) -> PlaybackSnapshot:
        self.last_position = position
        self.last_action = "seek"
        self.progress_units += 1
        return self.snapshot()

    def snapshot(self) -> PlaybackSnapshot:
        return PlaybackSnapshot(
            state=self.state,
            last_action=self.last_action,
            document_id=self.last_document_id,
            anchor=self.last_position.anchor,
            progress_units=self.progress_units,
            audio_reference=self.audio_reference,
            provider_name=PLAYBACK_CAPABILITIES.provider_name,
            process_id=None,
        )


def _position_from_anchor(anchor: str) -> ReadingPosition:
    section_index = 0
    chunk_index = 0
    for item in anchor.split("/"):
        key, _, raw_value = item.partition(":")
        if key == "section":
            section_index = int(raw_value)
        elif key == "chunk":
            chunk_index = int(raw_value)
    return ReadingPosition(section_index=section_index, chunk_index=chunk_index)
