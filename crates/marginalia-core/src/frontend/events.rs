use chrono::{DateTime, Utc};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrontendEventName {
    NoteSaved,
    PlaybackStateChanged,
    ProviderWarningEmitted,
    RuntimeFailed,
    RuntimeStopped,
    SessionProgressUpdated,
    SessionStarted,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrontendEvent {
    pub name: String,
    pub payload: HashMap<String, String>,
    pub event_id: String,
    pub occurred_at: DateTime<Utc>,
    pub protocol_version: u32,
}
