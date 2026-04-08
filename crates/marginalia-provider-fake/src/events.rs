use marginalia_core::events::DomainEvent;
use marginalia_core::ports::events::EventPublisher;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Default)]
pub struct RecordingEventPublisher {
    events: Arc<Mutex<Vec<DomainEvent>>>,
}

impl RecordingEventPublisher {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn published_events(&self) -> Vec<DomainEvent> {
        self.events
            .lock()
            .expect("recording event publisher lock poisoned")
            .clone()
    }
}

impl EventPublisher for RecordingEventPublisher {
    fn publish(&self, event: DomainEvent) {
        self.events
            .lock()
            .expect("recording event publisher lock poisoned")
            .push(event);
    }
}

#[cfg(test)]
mod tests {
    use super::RecordingEventPublisher;
    use marginalia_core::events::{DomainEvent, EventName};
    use marginalia_core::ports::events::EventPublisher;
    use std::collections::HashMap;

    #[test]
    fn recording_event_publisher_keeps_published_events() {
        let publisher = RecordingEventPublisher::new();
        publisher.publish(DomainEvent {
            name: EventName::DocumentIngested,
            payload: HashMap::new(),
            event_id: "event-1".to_string(),
            occurred_at: chrono::Utc::now(),
        });

        let events = publisher.published_events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_id, "event-1");
    }
}
