use crate::domain::reading_session::ReadingPosition;
use chrono::{DateTime, Utc};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoiceNote {
    pub note_id: String,
    pub session_id: String,
    pub document_id: String,
    pub position: ReadingPosition,
    pub transcript: String,
    pub transcription_provider: String,
    pub language: String,
    pub raw_audio_path: Option<PathBuf>,
    pub created_at: DateTime<Utc>,
}

impl VoiceNote {
    pub fn anchor(&self) -> String {
        self.position.anchor()
    }
}

#[cfg(test)]
mod tests {
    use super::VoiceNote;
    use crate::domain::reading_session::ReadingPosition;
    use chrono::Utc;

    #[test]
    fn voice_note_anchor_comes_from_position() {
        let note = VoiceNote {
            note_id: "note-1".to_string(),
            session_id: "session-1".to_string(),
            document_id: "doc-1".to_string(),
            position: ReadingPosition {
                section_index: 1,
                chunk_index: 4,
                char_offset: 0,
            },
            transcript: "Important thought".to_string(),
            transcription_provider: "fake".to_string(),
            language: "it".to_string(),
            raw_audio_path: None,
            created_at: Utc::now(),
        };

        assert_eq!(note.anchor(), "section:1/chunk:4");
    }
}
