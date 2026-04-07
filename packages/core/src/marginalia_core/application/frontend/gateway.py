"""Frontend gateway protocol."""

from __future__ import annotations

from typing import Protocol

from marginalia_core.application.frontend.capabilities import BackendCapabilities
from marginalia_core.application.frontend.envelopes import FrontendRequest, FrontendResponse
from marginalia_core.application.frontend.events import FrontendEvent


class FrontendGateway(Protocol):
    """Single backend surface for client frontends."""

    def capabilities(self) -> BackendCapabilities: ...

    def execute_command(self, request: FrontendRequest) -> FrontendResponse: ...

    def execute_query(self, request: FrontendRequest) -> FrontendResponse: ...

    def recent_events(self, limit: int = 50) -> tuple[FrontendEvent, ...]: ...
