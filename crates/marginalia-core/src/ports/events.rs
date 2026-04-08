use crate::events::{DomainEvent, EventName};

pub trait EventPublisher {
    fn publish(&self, event: DomainEvent);
}

pub trait EventSubscriber {
    fn subscribe(&mut self, event_name: EventName);
}

impl<T> EventPublisher for &T
where
    T: EventPublisher + ?Sized,
{
    fn publish(&self, event: DomainEvent) {
        (**self).publish(event);
    }
}
