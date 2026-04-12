use chrono::{DateTime, Utc};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventName {
    DocumentIngested,
    ReadingStarted,
    ReadingPaused,
    ReadingResumed,
    ReadingProgressed,
    ChapterRestarted,
    ChapterAdvanced,
    NoteRecordingStarted,
    NoteRecordingStopped,
    NoteSaved,
    RewriteRequested,
    RewriteCompleted,
    SummaryRequested,
    SummaryCompleted,
    PlaybackStarted,
    PlaybackPaused,
    PlaybackResumed,
    PlaybackStopped,
    ReadingCompleted,
    CommandDispatched,
    ErrorRaised,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainEvent {
    pub name: EventName,
    pub payload: HashMap<String, String>,
    pub event_id: String,
    pub occurred_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::{DomainEvent, EventName};
    use chrono::Utc;
    use std::collections::HashMap;

    #[test]
    fn domain_event_holds_name_and_payload() {
        let mut payload = HashMap::new();
        payload.insert("document_id".to_string(), "doc-1".to_string());
        let event = DomainEvent {
            name: EventName::DocumentIngested,
            payload,
            event_id: "event-1".to_string(),
            occurred_at: Utc::now(),
        };

        assert_eq!(event.payload.get("document_id"), Some(&"doc-1".to_string()));
    }
}
