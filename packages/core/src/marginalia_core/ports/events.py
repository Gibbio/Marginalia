"""Event publishing ports."""

from __future__ import annotations

from collections.abc import Callable
from typing import Protocol

from marginalia_core.events.models import DomainEvent, EventName

EventHandler = Callable[[DomainEvent], None]


class EventPublisher(Protocol):
    """Publish domain events."""

    def publish(self, event: DomainEvent) -> None:
        """Publish a single event."""
        ...


class EventSubscriber(Protocol):
    """Subscribe to specific event types."""

    def subscribe(self, event_name: EventName, handler: EventHandler) -> None:
        """Register an event handler."""
        ...
