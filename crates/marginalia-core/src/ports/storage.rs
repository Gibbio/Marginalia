use crate::domain::{
    Document, ReadingSession, RewriteDraft, SearchQuery, SearchResult, VoiceNote,
};
use std::fmt::{Display, Formatter};

// ---------------------------------------------------------------------------
// StorageError
// ---------------------------------------------------------------------------

/// Opaque error returned by storage write operations.
/// Wraps the underlying error message so the caller can log or report it
/// without coupling to a specific storage backend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageError(pub String);

impl Display for StorageError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "storage error: {}", self.0)
    }
}

impl std::error::Error for StorageError {}

// ---------------------------------------------------------------------------
// Repository traits
// ---------------------------------------------------------------------------

pub trait DocumentRepository {
    fn ensure_schema(&mut self);
    fn save_document(&mut self, document: Document) -> Result<(), StorageError>;
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

    fn save_document(&mut self, document: Document) -> Result<(), StorageError> {
        (**self).save_document(document)
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
    fn save_session(&mut self, session: ReadingSession) -> Result<(), StorageError>;
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

    fn save_session(&mut self, session: ReadingSession) -> Result<(), StorageError> {
        (**self).save_session(session)
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
    fn save_note(&mut self, note: VoiceNote) -> Result<(), StorageError>;
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

    fn save_note(&mut self, note: VoiceNote) -> Result<(), StorageError> {
        (**self).save_note(note)
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
    fn save_draft(&mut self, draft: RewriteDraft) -> Result<(), StorageError>;
    fn list_drafts_for_document(&self, document_id: &str) -> Vec<RewriteDraft>;
}

impl<T> RewriteDraftRepository for &mut T
where
    T: RewriteDraftRepository + ?Sized,
{
    fn ensure_schema(&mut self) {
        (**self).ensure_schema();
    }

    fn save_draft(&mut self, draft: RewriteDraft) -> Result<(), StorageError> {
        (**self).save_draft(draft)
    }

    fn list_drafts_for_document(&self, document_id: &str) -> Vec<RewriteDraft> {
        (**self).list_drafts_for_document(document_id)
    }
}
