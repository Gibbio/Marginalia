"""Frontend-facing contracts for backend clients."""

from marginalia_core.application.frontend.capabilities import BackendCapabilities
from marginalia_core.application.frontend.commands import FrontendCommandName
from marginalia_core.application.frontend.envelopes import (
    FRONTEND_PROTOCOL_VERSION,
    FrontendRequest,
    FrontendResponse,
    FrontendResponseStatus,
)
from marginalia_core.application.frontend.events import FrontendEvent, FrontendEventName
from marginalia_core.application.frontend.gateway import FrontendGateway
from marginalia_core.application.frontend.queries import FrontendQueryName
from marginalia_core.application.frontend.snapshots import (
    AppSnapshot,
    DocumentListItem,
    SessionSnapshot,
)

__all__ = [
    "AppSnapshot",
    "BackendCapabilities",
    "DocumentListItem",
    "FRONTEND_PROTOCOL_VERSION",
    "FrontendCommandName",
    "FrontendEvent",
    "FrontendEventName",
    "FrontendGateway",
    "FrontendQueryName",
    "FrontendRequest",
    "FrontendResponse",
    "FrontendResponseStatus",
    "SessionSnapshot",
]
