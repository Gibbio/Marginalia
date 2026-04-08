use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackState {
    Stopped,
    Playing,
    Paused,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReaderState {
    Idle,
    Reading,
    Paused,
    ListeningForCommand,
    RecordingNote,
    ProcessingRewrite,
    ReadingRewrite,
    Error,
}

impl PlaybackState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Stopped => "stopped",
            Self::Playing => "playing",
            Self::Paused => "paused",
        }
    }
}

impl ReaderState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Reading => "reading",
            Self::Paused => "paused",
            Self::ListeningForCommand => "listening_for_command",
            Self::RecordingNote => "recording_note",
            Self::ProcessingRewrite => "processing_rewrite",
            Self::ReadingRewrite => "reading_rewrite",
            Self::Error => "error",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ReadingPosition {
    pub section_index: usize,
    pub chunk_index: usize,
    pub char_offset: usize,
}

impl ReadingPosition {
    pub fn anchor(&self) -> String {
        format!("section:{}/chunk:{}", self.section_index, self.chunk_index)
    }

    pub fn from_anchor(anchor: &str) -> Self {
        let mut position = Self::default();

        for item in anchor.split('/') {
            let mut parts = item.splitn(2, ':');
            let key = parts.next().unwrap_or_default();
            let value = parts.next().unwrap_or_default();

            match key {
                "section" => {
                    if let Ok(parsed) = value.parse::<usize>() {
                        position.section_index = parsed;
                    }
                }
                "chunk" => {
                    if let Ok(parsed) = value.parse::<usize>() {
                        position.chunk_index = parsed;
                    }
                }
                _ => {}
            }
        }

        position
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadingSession {
    pub session_id: String,
    pub document_id: String,
    pub state: ReaderState,
    pub playback_state: PlaybackState,
    pub position: ReadingPosition,
    pub active_note_id: Option<String>,
    pub last_command: Option<String>,
    pub last_command_source: Option<String>,
    pub last_recognized_command: Option<String>,
    pub voice: Option<String>,
    pub tts_provider: Option<String>,
    pub command_stt_provider: Option<String>,
    pub playback_provider: Option<String>,
    pub command_listening_active: bool,
    pub command_language: Option<String>,
    pub audio_reference: Option<String>,
    pub playback_process_id: Option<u32>,
    pub runtime_process_id: Option<u32>,
    pub runtime_status: Option<String>,
    pub runtime_error: Option<String>,
    pub startup_cleanup_summary: Option<String>,
    pub is_active: bool,
    pub updated_at: DateTime<Utc>,
}

impl ReadingSession {
    pub fn new(session_id: impl Into<String>, document_id: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
            document_id: document_id.into(),
            state: ReaderState::Idle,
            playback_state: PlaybackState::Stopped,
            position: ReadingPosition::default(),
            active_note_id: None,
            last_command: None,
            last_command_source: None,
            last_recognized_command: None,
            voice: None,
            tts_provider: None,
            command_stt_provider: None,
            playback_provider: None,
            command_listening_active: false,
            command_language: None,
            audio_reference: None,
            playback_process_id: None,
            runtime_process_id: None,
            runtime_status: None,
            runtime_error: None,
            startup_cleanup_summary: None,
            is_active: true,
            updated_at: Utc::now(),
        }
    }

    pub fn touch(&mut self) {
        self.updated_at = Utc::now();
    }
}

#[cfg(test)]
mod tests {
    use super::{PlaybackState, ReaderState, ReadingPosition, ReadingSession};

    #[test]
    fn reading_position_round_trips_anchor() {
        let position = ReadingPosition {
            section_index: 3,
            chunk_index: 7,
            char_offset: 2,
        };

        assert_eq!(position.anchor(), "section:3/chunk:7");
        assert_eq!(ReadingPosition::from_anchor(&position.anchor()).section_index, 3);
        assert_eq!(ReadingPosition::from_anchor(&position.anchor()).chunk_index, 7);
    }

    #[test]
    fn reading_session_new_uses_expected_defaults() {
        let session = ReadingSession::new("session-1", "doc-1");

        assert_eq!(session.session_id, "session-1");
        assert_eq!(session.document_id, "doc-1");
        assert_eq!(session.state, ReaderState::Idle);
        assert_eq!(session.playback_state, PlaybackState::Stopped);
        assert!(session.is_active);
        assert!(!session.command_listening_active);
    }
}
