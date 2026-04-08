use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RewriteStatus {
    Requested,
    Generated,
    Dismissed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RewriteDraft {
    pub draft_id: String,
    pub document_id: String,
    pub section_index: usize,
    pub source_anchor: String,
    pub source_excerpt: String,
    pub note_transcripts: Vec<String>,
    pub rewritten_text: String,
    pub provider_name: String,
    pub status: RewriteStatus,
    pub created_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::{RewriteDraft, RewriteStatus};
    use chrono::Utc;

    #[test]
    fn rewrite_draft_keeps_note_transcripts() {
        let draft = RewriteDraft {
            draft_id: "draft-1".to_string(),
            document_id: "doc-1".to_string(),
            section_index: 0,
            source_anchor: "section:0/chunk:0".to_string(),
            source_excerpt: "Alpha".to_string(),
            note_transcripts: vec!["Note one".to_string(), "Note two".to_string()],
            rewritten_text: "Rewritten".to_string(),
            provider_name: "fake".to_string(),
            status: RewriteStatus::Requested,
            created_at: Utc::now(),
        };

        assert_eq!(draft.note_transcripts.len(), 2);
        assert_eq!(draft.status, RewriteStatus::Requested);
    }
}
