"""Transport-ready request and response envelopes."""

from __future__ import annotations

from dataclasses import dataclass, field
from enum import Enum
from uuid import uuid4

FRONTEND_PROTOCOL_VERSION = 1


class FrontendResponseStatus(str, Enum):
    """Transport-safe response status values."""

    ERROR = "error"
    OK = "ok"


@dataclass(frozen=True, slots=True)
class FrontendRequest:
    """Frontend request envelope."""

    request_type: str
    name: str
    payload: dict[str, object] = field(default_factory=dict)
    request_id: str = field(default_factory=lambda: str(uuid4()))
    protocol_version: int = FRONTEND_PROTOCOL_VERSION

    @classmethod
    def from_dict(cls, raw: dict[str, object]) -> FrontendRequest:
        """Build a request from a decoded JSON object."""

        request_type = str(raw.get("type", "")).strip()
        name = str(raw.get("name", "")).strip()
        payload = raw.get("payload", {})
        request_id = raw.get("id")
        protocol_version = raw.get("protocol_version", FRONTEND_PROTOCOL_VERSION)
        if not isinstance(payload, dict):
            raise ValueError("Request payload must be an object.")
        if not request_type:
            raise ValueError("Request type is required.")
        if not name:
            raise ValueError("Request name is required.")
        if not isinstance(protocol_version, int):
            raise ValueError("protocol_version must be an integer.")
        return cls(
            request_type=request_type,
            name=name,
            payload=payload,
            request_id=str(request_id or uuid4()),
            protocol_version=protocol_version,
        )


@dataclass(frozen=True, slots=True)
class FrontendResponse:
    """Backend response envelope."""

    status: FrontendResponseStatus
    name: str
    message: str
    payload: dict[str, object] = field(default_factory=dict)
    request_id: str | None = None
    protocol_version: int = FRONTEND_PROTOCOL_VERSION
