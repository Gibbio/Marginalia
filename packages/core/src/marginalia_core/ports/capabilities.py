"""Shared provider capability models."""

from __future__ import annotations

from dataclasses import dataclass
from enum import Enum


class ProviderExecutionMode(str, Enum):
    """How a provider is expected to execute."""

    LOCAL = "local"
    HYBRID = "hybrid"
    REMOTE = "remote"


@dataclass(frozen=True, slots=True)
class ProviderCapabilities:
    """Capability flags used by fake and future real providers."""

    provider_name: str
    interface_kind: str
    supported_languages: tuple[str, ...] = ("en",)
    supports_streaming: bool = False
    supports_partial_results: bool = False
    supports_timestamps: bool = False
    low_latency_suitable: bool = False
    offline_capable: bool = True
    execution_mode: ProviderExecutionMode = ProviderExecutionMode.LOCAL
