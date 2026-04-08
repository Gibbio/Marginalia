use chrono::{DateTime, Utc};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentListItem {
    pub chapter_count: usize,
    pub chunk_count: usize,
    pub document_id: String,
    pub title: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppSnapshot {
    pub active_session_id: Option<String>,
    pub document_count: usize,
    pub latest_document_id: Option<String>,
    pub playback_state: Option<String>,
    pub runtime_status: Option<String>,
    pub state: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionSnapshot {
    pub anchor: String,
    pub chunk_index: usize,
    pub chunk_text: String,
    pub command_listening_active: bool,
    pub command_stt_provider: Option<String>,
    pub document_id: String,
    pub notes_count: usize,
    pub playback_provider: Option<String>,
    pub playback_state: String,
    pub section_count: usize,
    pub section_index: usize,
    pub section_title: String,
    pub session_id: String,
    pub state: String,
    pub tts_provider: Option<String>,
    pub voice: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentChunkView {
    pub anchor: String,
    pub char_end: usize,
    pub char_start: usize,
    pub index: usize,
    pub is_active: bool,
    pub is_read: bool,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentSectionView {
    pub chunk_count: usize,
    pub chunks: Vec<DocumentChunkView>,
    pub index: usize,
    pub source_anchor: Option<String>,
    pub title: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentView {
    pub active_chunk_index: Option<usize>,
    pub active_section_index: Option<usize>,
    pub chapter_count: usize,
    pub chunk_count: usize,
    pub document_id: String,
    pub sections: Vec<DocumentSectionView>,
    pub source_path: String,
    pub title: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteView {
    pub anchor: String,
    pub created_at: DateTime<Utc>,
    pub document_id: String,
    pub language: String,
    pub note_id: String,
    pub section_index: usize,
    pub chunk_index: usize,
    pub session_id: String,
    pub transcript: String,
    pub transcription_provider: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotesSnapshot {
    pub document_id: String,
    pub notes: Vec<NoteView>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SearchResultView {
    pub anchor: String,
    pub entity_id: String,
    pub entity_kind: String,
    pub excerpt: String,
    pub score: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SearchResultsSnapshot {
    pub query: String,
    pub results: Vec<SearchResultView>,
}

#[cfg(test)]
mod tests {
    use super::{DocumentChunkView, DocumentSectionView, DocumentView};

    #[test]
    fn document_view_keeps_section_and_chunk_counts() {
        let view = DocumentView {
            active_chunk_index: Some(0),
            active_section_index: Some(0),
            chapter_count: 1,
            chunk_count: 1,
            document_id: "doc-1".to_string(),
            sections: vec![DocumentSectionView {
                chunk_count: 1,
                chunks: vec![DocumentChunkView {
                    anchor: "section:0/chunk:0".to_string(),
                    char_end: 10,
                    char_start: 0,
                    index: 0,
                    is_active: true,
                    is_read: false,
                    text: "Alpha".to_string(),
                }],
                index: 0,
                source_anchor: Some("section:0".to_string()),
                title: "Intro".to_string(),
            }],
            source_path: "/tmp/doc.md".to_string(),
            title: "Doc".to_string(),
        };

        assert_eq!(view.chapter_count, 1);
        assert_eq!(view.sections[0].chunk_count, 1);
        assert!(view.sections[0].chunks[0].is_active);
    }
}
