"""Capability DTOs exposed to frontends."""

from __future__ import annotations

from dataclasses import dataclass


@dataclass(frozen=True, slots=True)
class BackendCapabilities:
    """Backend-declared features and supported contract surface."""

    protocol_version: int
    commands: tuple[str, ...]
    queries: tuple[str, ...]
    transports: tuple[str, ...]
    frontend_event_stream_supported: bool
    dictation_enabled: bool
    rewrite_enabled: bool
    summary_enabled: bool
