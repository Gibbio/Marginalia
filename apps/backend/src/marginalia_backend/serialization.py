"""Helpers for transport-safe serialization."""

from __future__ import annotations

from dataclasses import asdict, is_dataclass
from datetime import datetime
from enum import Enum
from pathlib import Path
from typing import Any


def to_transport_value(value: object) -> object:
    """Convert backend values to JSON-safe primitives."""

    if value is None or isinstance(value, (bool, int, float, str)):
        return value
    if isinstance(value, Path):
        return str(value)
    if isinstance(value, datetime):
        return value.isoformat()
    if isinstance(value, Enum):
        return value.value
    if is_dataclass(value) and not isinstance(value, type):
        return to_transport_value(asdict(value))
    if isinstance(value, dict):
        return {str(key): to_transport_value(item) for key, item in value.items()}
    if isinstance(value, (list, tuple, set)):
        return [to_transport_value(item) for item in value]
    return str(value)


def to_transport_dict(value: dict[str, Any]) -> dict[str, object]:
    """Convert a mapping to a transport-safe payload."""

    return {
        key: to_transport_value(item)
        for key, item in value.items()
    }
