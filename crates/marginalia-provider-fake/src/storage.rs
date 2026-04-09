use marginalia_core::domain::{
    Document, ReadingSession, RewriteDraft, SearchQuery, SearchResult, VoiceNote,
};
use marginalia_core::ports::storage::{
    DocumentRepository, NoteRepository, RewriteDraftRepository, SessionRepository, StorageError,
};
use std::collections::HashMap;

#[derive(Debug, Clone, Default)]
pub struct InMemoryDocumentRepository {
    documents: HashMap<String, Document>,
}

impl InMemoryDocumentRepository {
    pub fn new() -> Self {
        Self::default()
    }
}

impl DocumentRepository for InMemoryDocumentRepository {

    fn save_document(&mut self, document: Document) -> Result<(), StorageError> {
        self.documents.insert(document.document_id.clone(), document);
        Ok(())
    }

    fn get_document(&self, document_id: &str) -> Option<Document> {
        self.documents.get(document_id).cloned()
    }

    fn list_documents(&self) -> Vec<Document> {
        let mut documents = self.documents.values().cloned().collect::<Vec<_>>();
        documents.sort_by(|left, right| right.imported_at.cmp(&left.imported_at));
        documents
    }

    fn search_documents(&self, query: &SearchQuery) -> Vec<SearchResult> {
        let needle = query.normalized_text().to_lowercase();
        if needle.is_empty() {
            return Vec::new();
        }

        let mut results = Vec::new();
        for document in self.documents.values() {
            if let Some(document_id) = query.document_id.as_deref() {
                if document.document_id != document_id {
                    continue;
                }
            }

            for section in &document.sections {
                for chunk in &section.chunks {
                    let haystack = chunk.text.to_lowercase();
                    if !haystack.contains(&needle) {
                        continue;
                    }

                    results.push(SearchResult {
                        entity_kind: "document_chunk".to_string(),
                        entity_id: document.document_id.clone(),
                        score: 1.0,
                        excerpt: chunk.text.clone(),
                        anchor: format!("section:{}/{}", section.index, chunk.anchor()),
                    });
                }
            }
        }

        results.truncate(query.limit);
        results
    }
}

#[derive(Debug, Clone, Default)]
pub struct InMemorySessionRepository {
    sessions: HashMap<String, ReadingSession>,
}

impl InMemorySessionRepository {
    pub fn new() -> Self {
        Self::default()
    }
}

impl SessionRepository for InMemorySessionRepository {

    fn save_session(&mut self, session: ReadingSession) -> Result<(), StorageError> {
        self.sessions.insert(session.session_id.clone(), session);
        Ok(())
    }

    fn get_active_session(&self) -> Option<ReadingSession> {
        let mut sessions = self
            .sessions
            .values()
            .filter(|session| session.is_active)
            .cloned()
            .collect::<Vec<_>>();
        sessions.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
        sessions.into_iter().next()
    }

    fn deactivate_stale_sessions(&mut self, _max_inactive_hours: u32) -> u32 {
        0
    }
}

#[derive(Debug, Clone, Default)]
pub struct InMemoryNoteRepository {
    notes: Vec<VoiceNote>,
}

impl InMemoryNoteRepository {
    pub fn new() -> Self {
        Self::default()
    }
}

impl NoteRepository for InMemoryNoteRepository {

    fn save_note(&mut self, note: VoiceNote) -> Result<(), StorageError> {
        self.notes.push(note);
        self.notes.sort_by(|left, right| left.created_at.cmp(&right.created_at));
        Ok(())
    }

    fn list_notes_for_document(&self, document_id: &str) -> Vec<VoiceNote> {
        self.notes
            .iter()
            .filter(|note| note.document_id == document_id)
            .cloned()
            .collect()
    }

    fn search_notes(&self, query: &SearchQuery) -> Vec<SearchResult> {
        let needle = query.normalized_text().to_lowercase();
        if needle.is_empty() {
            return Vec::new();
        }

        let mut results = self
            .notes
            .iter()
            .filter(|note| {
                query.document_id
                    .as_deref()
                    .map(|document_id| note.document_id == document_id)
                    .unwrap_or(true)
            })
            .filter_map(|note| {
                let haystack = note.transcript.to_lowercase();
                if !haystack.contains(&needle) {
                    return None;
                }

                Some(SearchResult {
                    entity_kind: "voice_note".to_string(),
                    entity_id: note.note_id.clone(),
                    score: 1.0,
                    excerpt: note.transcript.clone(),
                    anchor: note.anchor(),
                })
            })
            .collect::<Vec<_>>();

        results.truncate(query.limit);
        results
    }
}

#[derive(Debug, Clone, Default)]
pub struct InMemoryRewriteDraftRepository {
    drafts: Vec<RewriteDraft>,
}

impl InMemoryRewriteDraftRepository {
    pub fn new() -> Self {
        Self::default()
    }
}

impl RewriteDraftRepository for InMemoryRewriteDraftRepository {

    fn save_draft(&mut self, draft: RewriteDraft) -> Result<(), StorageError> {
        self.drafts.push(draft);
        Ok(())
    }

    fn list_drafts_for_document(&self, document_id: &str) -> Vec<RewriteDraft> {
        self.drafts
            .iter()
            .filter(|draft| draft.document_id == document_id)
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        InMemoryDocumentRepository, InMemoryNoteRepository, InMemorySessionRepository,
    };
    use marginalia_core::domain::{
        Document, DocumentChunk, DocumentSection, ReadingPosition, ReadingSession, SearchQuery,
        VoiceNote,
    };
    use marginalia_core::ports::storage::{DocumentRepository, NoteRepository, SessionRepository};
    use std::path::PathBuf;

    #[test]
    fn document_repository_searches_chunk_text() {
        let mut repository = InMemoryDocumentRepository::new();
        repository.save_document(Document {
            document_id: "doc-1".to_string(),
            title: "Doc".to_string(),
            source_path: PathBuf::from("/tmp/doc.md"),
            sections: vec![DocumentSection {
                index: 0,
                title: "Intro".to_string(),
                chunks: vec![DocumentChunk {
                    index: 0,
                    text: "Alpha beta gamma".to_string(),
                    char_start: 0,
                    char_end: 16,
                }],
                source_anchor: Some("section:0".to_string()),
            }],
            imported_at: chrono::Utc::now(),
        }).unwrap();

        let results = repository.search_documents(&SearchQuery {
            text: "beta".to_string(),
            document_id: None,
            limit: 10,
        });

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].entity_kind, "document_chunk");
    }

    #[test]
    fn session_repository_returns_active_session() {
        let mut repository = InMemorySessionRepository::new();
        let session = ReadingSession::new("session-1", "doc-1");
        repository.save_session(session.clone()).unwrap();

        assert_eq!(repository.get_active_session(), Some(session));
    }

    #[test]
    fn note_repository_searches_transcripts() {
        let mut repository = InMemoryNoteRepository::new();
        repository.save_note(VoiceNote {
            note_id: "note-1".to_string(),
            session_id: "session-1".to_string(),
            document_id: "doc-1".to_string(),
            position: ReadingPosition::default(),
            transcript: "Important passage".to_string(),
            transcription_provider: "fake-dictation".to_string(),
            language: "it".to_string(),
            raw_audio_path: None,
            created_at: chrono::Utc::now(),
        }).unwrap();

        let results = repository.search_notes(&SearchQuery {
            text: "passage".to_string(),
            document_id: Some("doc-1".to_string()),
            limit: 10,
        });

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].entity_kind, "voice_note");
    }
}
