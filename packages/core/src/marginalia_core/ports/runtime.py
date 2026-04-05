"""Runtime session supervision ports."""

from __future__ import annotations

from dataclasses import dataclass, field
from datetime import UTC, datetime
from pathlib import Path
from typing import Protocol


def _utc_now() -> datetime:
    return datetime.now(UTC)


@dataclass(frozen=True, slots=True)
class RuntimeSessionRecord:
    """Persisted metadata for the active foreground runtime loop."""

    process_id: int
    session_id: str
    document_id: str
    command_language: str
    started_at: datetime = field(default_factory=_utc_now)
    entrypoint: str = "play"
    working_directory: Path | None = None


@dataclass(frozen=True, slots=True)
class RuntimeCleanupReport:
    """Best-effort cleanup result for a previously running foreground loop."""

    runtime_found: bool
    record_removed: bool
    terminated_process_ids: tuple[int, ...] = ()
    notes: tuple[str, ...] = ()

    @property
    def cleaned_up(self) -> bool:
        """Return whether startup removed or terminated prior runtime state."""

        return self.record_removed or bool(self.terminated_process_ids)


class RuntimeSupervisor(Protocol):
    """Manage the single supported foreground reading runtime."""

    def activate(self, record: RuntimeSessionRecord) -> None:
        """Persist the current runtime session record."""
        ...

    def current_runtime(self) -> RuntimeSessionRecord | None:
        """Return the currently registered runtime, if any."""
        ...

    def cleanup_existing_runtime(self, *, current_process_id: int) -> RuntimeCleanupReport:
        """Terminate and clear a previously registered runtime when safe."""
        ...

    def clear(self, *, process_id: int | None = None) -> None:
        """Remove the persisted runtime record."""
        ...
