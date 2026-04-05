"""Common service result object."""

from __future__ import annotations

from dataclasses import dataclass, field
from enum import Enum
from typing import Any


class OperationStatus(str, Enum):
    """High-level outcome status for CLI-safe service calls."""

    OK = "ok"
    PLANNED = "planned"
    ERROR = "error"


@dataclass(frozen=True, slots=True)
class OperationResult:
    """Service return type designed for both humans and CLI JSON output."""

    status: OperationStatus
    message: str
    data: dict[str, Any] = field(default_factory=dict)

    @classmethod
    def ok(cls, message: str, *, data: dict[str, Any] | None = None) -> OperationResult:
        return cls(status=OperationStatus.OK, message=message, data=data or {})

    @classmethod
    def planned(cls, message: str, *, data: dict[str, Any] | None = None) -> OperationResult:
        return cls(status=OperationStatus.PLANNED, message=message, data=data or {})

    @classmethod
    def error(cls, message: str, *, data: dict[str, Any] | None = None) -> OperationResult:
        return cls(status=OperationStatus.ERROR, message=message, data=data or {})

    def to_dict(self) -> dict[str, Any]:
        return {"status": self.status.value, "message": self.message, "data": self.data}
