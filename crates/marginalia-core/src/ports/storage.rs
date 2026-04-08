use crate::domain::{
    Document, ReadingSession, RewriteDraft, SearchQuery, SearchResult, VoiceNote,
};

pub trait DocumentRepository {
    fn ensure_schema(&mut self);
    fn save_document(&mut self, document: Document);
    fn get_document(&self, document_id: &str) -> Option<Document>;
    fn list_documents(&self) -> Vec<Document>;
    fn search_documents(&self, query: &SearchQuery) -> Vec<SearchResult>;
}

pub trait SessionRepository {
    fn ensure_schema(&mut self);
    fn save_session(&mut self, session: ReadingSession);
    fn get_active_session(&self) -> Option<ReadingSession>;
    fn deactivate_stale_sessions(&mut self, max_inactive_hours: u32) -> u32;
}

pub trait NoteRepository {
    fn ensure_schema(&mut self);
    fn save_note(&mut self, note: VoiceNote);
    fn list_notes_for_document(&self, document_id: &str) -> Vec<VoiceNote>;
    fn search_notes(&self, query: &SearchQuery) -> Vec<SearchResult>;
}

pub trait RewriteDraftRepository {
    fn ensure_schema(&mut self);
    fn save_draft(&mut self, draft: RewriteDraft);
    fn list_drafts_for_document(&self, document_id: &str) -> Vec<RewriteDraft>;
}
