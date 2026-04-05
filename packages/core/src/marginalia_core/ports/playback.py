"""Audio playback ports."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Protocol

from marginalia_core.domain.document import Document
from marginalia_core.domain.reading_session import PlaybackState, ReadingPosition
from marginalia_core.ports.capabilities import ProviderCapabilities
from marginalia_core.ports.tts import SynthesisResult


@dataclass(frozen=True, slots=True)
class PlaybackSnapshot:
    """Current state of the playback engine."""

    state: PlaybackState
    last_action: str
    document_id: str | None = None
    anchor: str | None = None
    progress_units: int = 0
    audio_reference: str | None = None
    provider_name: str | None = None
    process_id: int | None = None


class PlaybackEngine(Protocol):
    """Output audio and manage seek/pause/resume semantics."""

    def describe_capabilities(self) -> ProviderCapabilities:
        """Describe playback behavior and constraints."""
        ...

    def hydrate(self, snapshot: PlaybackSnapshot | None) -> None:
        """Restore persisted playback context into a fresh engine instance."""
        ...

    def start(
        self,
        document: Document,
        position: ReadingPosition,
        *,
        synthesis: SynthesisResult | None = None,
    ) -> PlaybackSnapshot:
        """Start playback for a document position."""
        ...

    def pause(self) -> PlaybackSnapshot:
        """Pause the currently active playback session."""
        ...

    def resume(self) -> PlaybackSnapshot:
        """Resume the currently active playback session."""
        ...

    def stop(self) -> PlaybackSnapshot:
        """Stop playback entirely."""
        ...

    def seek(self, position: ReadingPosition) -> PlaybackSnapshot:
        """Move playback to a new document position."""
        ...

    def snapshot(self) -> PlaybackSnapshot:
        """Return the latest playback snapshot."""
        ...
