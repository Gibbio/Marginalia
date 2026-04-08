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

impl<T> DocumentRepository for &mut T
where
    T: DocumentRepository + ?Sized,
{
    fn ensure_schema(&mut self) {
        (**self).ensure_schema();
    }

    fn save_document(&mut self, document: Document) {
        (**self).save_document(document);
    }

    fn get_document(&self, document_id: &str) -> Option<Document> {
        (**self).get_document(document_id)
    }

    fn list_documents(&self) -> Vec<Document> {
        (**self).list_documents()
    }

    fn search_documents(&self, query: &SearchQuery) -> Vec<SearchResult> {
        (**self).search_documents(query)
    }
}

pub trait SessionRepository {
    fn ensure_schema(&mut self);
    fn save_session(&mut self, session: ReadingSession);
    fn get_active_session(&self) -> Option<ReadingSession>;
    fn deactivate_stale_sessions(&mut self, max_inactive_hours: u32) -> u32;
}

impl<T> SessionRepository for &mut T
where
    T: SessionRepository + ?Sized,
{
    fn ensure_schema(&mut self) {
        (**self).ensure_schema();
    }

    fn save_session(&mut self, session: ReadingSession) {
        (**self).save_session(session);
    }

    fn get_active_session(&self) -> Option<ReadingSession> {
        (**self).get_active_session()
    }

    fn deactivate_stale_sessions(&mut self, max_inactive_hours: u32) -> u32 {
        (**self).deactivate_stale_sessions(max_inactive_hours)
    }
}

pub trait NoteRepository {
    fn ensure_schema(&mut self);
    fn save_note(&mut self, note: VoiceNote);
    fn list_notes_for_document(&self, document_id: &str) -> Vec<VoiceNote>;
    fn search_notes(&self, query: &SearchQuery) -> Vec<SearchResult>;
}

impl<T> NoteRepository for &mut T
where
    T: NoteRepository + ?Sized,
{
    fn ensure_schema(&mut self) {
        (**self).ensure_schema();
    }

    fn save_note(&mut self, note: VoiceNote) {
        (**self).save_note(note);
    }

    fn list_notes_for_document(&self, document_id: &str) -> Vec<VoiceNote> {
        (**self).list_notes_for_document(document_id)
    }

    fn search_notes(&self, query: &SearchQuery) -> Vec<SearchResult> {
        (**self).search_notes(query)
    }
}

pub trait RewriteDraftRepository {
    fn ensure_schema(&mut self);
    fn save_draft(&mut self, draft: RewriteDraft);
    fn list_drafts_for_document(&self, document_id: &str) -> Vec<RewriteDraft>;
}

impl<T> RewriteDraftRepository for &mut T
where
    T: RewriteDraftRepository + ?Sized,
{
    fn ensure_schema(&mut self) {
        (**self).ensure_schema();
    }

    fn save_draft(&mut self, draft: RewriteDraft) {
        (**self).save_draft(draft);
    }

    fn list_drafts_for_document(&self, document_id: &str) -> Vec<RewriteDraft> {
        (**self).list_drafts_for_document(document_id)
    }
}
