use crate::events::{DomainEvent, EventName};

pub trait EventPublisher {
    fn publish(&self, event: DomainEvent);
}

pub trait EventSubscriber {
    fn subscribe(&mut self, event_name: EventName);
}
