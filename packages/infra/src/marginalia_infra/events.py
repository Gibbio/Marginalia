"""In-process event bus placeholder."""

from __future__ import annotations

from collections import defaultdict

from marginalia_core.events.models import DomainEvent, EventName
from marginalia_core.ports.events import EventHandler


class InMemoryEventBus:
    """Minimal in-process event publisher/subscriber."""

    def __init__(self) -> None:
        self._subscribers: dict[EventName, list[EventHandler]] = defaultdict(list)
        self.published_events: list[DomainEvent] = []

    def publish(self, event: DomainEvent) -> None:
        self.published_events.append(event)
        for handler in self._subscribers.get(event.name, []):
            handler(event)

    def subscribe(self, event_name: EventName, handler: EventHandler) -> None:
        self._subscribers[event_name].append(handler)

    def recent(self, limit: int = 10) -> tuple[DomainEvent, ...]:
        if limit <= 0:
            return ()
        return tuple(self.published_events[-limit:])
