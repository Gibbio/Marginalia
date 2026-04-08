use crate::domain::ReadingSession;
use crate::frontend::{AppSnapshot, SessionSnapshot};
use crate::ports::{PlaybackEngine, PlaybackSnapshot};
use crate::ports::storage::{
    DocumentRepository, NoteRepository, RewriteDraftRepository, SessionRepository,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionQueryError {
    MissingDocument { document_id: String },
    MissingSection { document_id: String, section_index: usize },
    MissingChunk {
        document_id: String,
        section_index: usize,
        chunk_index: usize,
    },
}

pub struct SessionQueryService<S, D, N, R, P>
where
    S: SessionRepository,
    D: DocumentRepository,
    N: NoteRepository,
    R: RewriteDraftRepository,
    P: PlaybackEngine,
{
    session_repository: S,
    document_repository: D,
    note_repository: N,
    draft_repository: R,
    playback_engine: P,
}

impl<S, D, N, R, P> SessionQueryService<S, D, N, R, P>
where
    S: SessionRepository,
    D: DocumentRepository,
    N: NoteRepository,
    R: RewriteDraftRepository,
    P: PlaybackEngine,
{
    pub fn new(
        session_repository: S,
        document_repository: D,
        note_repository: N,
        draft_repository: R,
        playback_engine: P,
    ) -> Self {
        Self {
            session_repository,
            document_repository,
            note_repository,
            draft_repository,
            playback_engine,
        }
    }

    pub fn app_snapshot(&mut self) -> AppSnapshot {
        let session = self.session_repository.get_active_session();
        let documents = self.document_repository.list_documents();
        let latest_document_id = documents.first().map(|document| document.document_id.clone());

        match session {
            Some(session) => {
                let playback = self.hydrate_playback_from_session(&session);

                AppSnapshot {
                    active_session_id: Some(session.session_id.clone()),
                    document_count: documents.len(),
                    latest_document_id,
                    playback_state: Some(playback.state.as_str().to_string()),
                    runtime_status: session.runtime_status.clone(),
                    state: session.state.as_str().to_string(),
                }
            }
            None => {
                self.playback_engine.hydrate(None);
                let playback = self.playback_engine.snapshot();

                AppSnapshot {
                    active_session_id: None,
                    document_count: documents.len(),
                    latest_document_id,
                    playback_state: Some(playback.state.as_str().to_string()),
                    runtime_status: None,
                    state: "idle".to_string(),
                }
            }
        }
    }

    pub fn session_snapshot(&mut self) -> Result<Option<SessionSnapshot>, SessionQueryError> {
        let Some(mut session) = self.session_repository.get_active_session() else {
            self.playback_engine.hydrate(None);
            return Ok(None);
        };

        let document =
            self.document_repository
                .get_document(&session.document_id)
                .ok_or_else(|| SessionQueryError::MissingDocument {
                    document_id: session.document_id.clone(),
                })?;

        let section = document.get_section(session.position.section_index).ok_or_else(|| {
            SessionQueryError::MissingSection {
                document_id: session.document_id.clone(),
                section_index: session.position.section_index,
            }
        })?;

        let chunk = document
            .get_chunk(session.position.section_index, session.position.chunk_index)
            .ok_or_else(|| SessionQueryError::MissingChunk {
                document_id: session.document_id.clone(),
                section_index: session.position.section_index,
                chunk_index: session.position.chunk_index,
            })?;

        let notes = self
            .note_repository
            .list_notes_for_document(&document.document_id);
        let _drafts = self
            .draft_repository
            .list_drafts_for_document(&document.document_id);
        let playback = self.hydrated_playback_snapshot(&mut session);

        Ok(Some(SessionSnapshot {
            anchor: session.position.anchor(),
            chunk_index: session.position.chunk_index,
            chunk_text: chunk.text.clone(),
            command_listening_active: session.command_listening_active,
            command_stt_provider: session.command_stt_provider.clone(),
            document_id: document.document_id.clone(),
            notes_count: notes.len(),
            playback_provider: session.playback_provider.clone(),
            playback_state: playback.state.as_str().to_string(),
            section_count: document.chapter_count(),
            section_index: session.position.section_index,
            section_title: section.title.clone(),
            session_id: session.session_id.clone(),
            state: session.state.as_str().to_string(),
            tts_provider: session.tts_provider.clone(),
            voice: session.voice.clone(),
        }))
    }

    fn hydrate_playback_from_session(&mut self, session: &ReadingSession) -> PlaybackSnapshot {
        self.playback_engine.hydrate(Some(PlaybackSnapshot {
            state: session.playback_state,
            last_action: session
                .last_command
                .clone()
                .unwrap_or_else(|| "session-state".to_string()),
            document_id: Some(session.document_id.clone()),
            anchor: Some(session.position.anchor()),
            progress_units: 0,
            audio_reference: session.audio_reference.clone(),
            provider_name: session.playback_provider.clone(),
            process_id: session.playback_process_id,
        }));

        self.playback_engine.snapshot()
    }

    fn hydrated_playback_snapshot(&mut self, session: &mut ReadingSession) -> PlaybackSnapshot {
        let snapshot = self.hydrate_playback_from_session(session);

        session.playback_state = snapshot.state;
        session.audio_reference = snapshot
            .audio_reference
            .clone()
            .or_else(|| session.audio_reference.clone());
        session.playback_provider = snapshot
            .provider_name
            .clone()
            .or_else(|| session.playback_provider.clone());
        session.playback_process_id = snapshot.process_id.or(session.playback_process_id);
        session.touch();
        self.session_repository.save_session(session.clone());

        snapshot
    }
}

#[cfg(test)]
mod tests {
    use super::{SessionQueryError, SessionQueryService};
    use crate::domain::{
        Document, DocumentChunk, DocumentSection, PlaybackState, ReadingPosition, ReadingSession,
        SearchQuery, SearchResult, VoiceNote,
    };
    use crate::frontend::{AppSnapshot, SessionSnapshot};
    use crate::ports::{
        PlaybackEngine, PlaybackSnapshot, ProviderCapabilities, SynthesisResult,
    };
    use crate::ports::storage::{
        DocumentRepository, NoteRepository, RewriteDraftRepository, SessionRepository,
    };
    use crate::domain::RewriteDraft;
    use chrono::Utc;
    use std::path::PathBuf;

    struct StubDocumentRepository {
        documents: Vec<Document>,
    }

    impl DocumentRepository for StubDocumentRepository {
        fn ensure_schema(&mut self) {}

        fn save_document(&mut self, document: Document) {
            self.documents.push(document);
        }

        fn get_document(&self, document_id: &str) -> Option<Document> {
            self.documents
                .iter()
                .find(|document| document.document_id == document_id)
                .cloned()
        }

        fn list_documents(&self) -> Vec<Document> {
            self.documents.clone()
        }

        fn search_documents(&self, _query: &SearchQuery) -> Vec<SearchResult> {
            Vec::new()
        }
    }

    struct StubSessionRepository {
        active_session: Option<ReadingSession>,
        saved_session: Option<ReadingSession>,
    }

    impl SessionRepository for StubSessionRepository {
        fn ensure_schema(&mut self) {}

        fn save_session(&mut self, session: ReadingSession) {
            self.saved_session = Some(session.clone());
            self.active_session = Some(session);
        }

        fn get_active_session(&self) -> Option<ReadingSession> {
            self.active_session.clone()
        }

        fn deactivate_stale_sessions(&mut self, _max_inactive_hours: u32) -> u32 {
            0
        }
    }

    struct StubNoteRepository {
        notes: Vec<VoiceNote>,
    }

    impl NoteRepository for StubNoteRepository {
        fn ensure_schema(&mut self) {}

        fn save_note(&mut self, note: VoiceNote) {
            self.notes.push(note);
        }

        fn list_notes_for_document(&self, document_id: &str) -> Vec<VoiceNote> {
            self.notes
                .iter()
                .filter(|note| note.document_id == document_id)
                .cloned()
                .collect()
        }

        fn search_notes(&self, _query: &SearchQuery) -> Vec<SearchResult> {
            Vec::new()
        }
    }

    struct StubDraftRepository;

    impl RewriteDraftRepository for StubDraftRepository {
        fn ensure_schema(&mut self) {}

        fn save_draft(&mut self, _draft: RewriteDraft) {}

        fn list_drafts_for_document(&self, _document_id: &str) -> Vec<RewriteDraft> {
            Vec::new()
        }
    }

    struct StubPlaybackEngine {
        last_hydrated: Option<PlaybackSnapshot>,
        snapshot: PlaybackSnapshot,
    }

    impl PlaybackEngine for StubPlaybackEngine {
        fn describe_capabilities(&self) -> ProviderCapabilities {
            ProviderCapabilities {
                provider_name: "stub-playback".to_string(),
                interface_kind: "playback".to_string(),
                ..ProviderCapabilities::default()
            }
        }

        fn hydrate(&mut self, snapshot: Option<PlaybackSnapshot>) {
            self.last_hydrated = snapshot;
        }

        fn start(
            &mut self,
            _document: &Document,
            _position: &ReadingPosition,
            _synthesis: Option<SynthesisResult>,
        ) -> PlaybackSnapshot {
            self.snapshot.clone()
        }

        fn pause(&mut self) -> PlaybackSnapshot {
            self.snapshot.clone()
        }

        fn resume(&mut self) -> PlaybackSnapshot {
            self.snapshot.clone()
        }

        fn stop(&mut self) -> PlaybackSnapshot {
            self.snapshot.clone()
        }

        fn seek(&mut self, _position: &ReadingPosition) -> PlaybackSnapshot {
            self.snapshot.clone()
        }

        fn snapshot(&self) -> PlaybackSnapshot {
            self.snapshot.clone()
        }
    }

    fn sample_document() -> Document {
        Document {
            document_id: "doc-1".to_string(),
            title: "Document".to_string(),
            source_path: PathBuf::from("/tmp/doc.md"),
            sections: vec![DocumentSection {
                index: 0,
                title: "Intro".to_string(),
                chunks: vec![DocumentChunk {
                    index: 0,
                    text: "Hello world".to_string(),
                    char_start: 0,
                    char_end: 11,
                }],
                source_anchor: Some("section:0".to_string()),
            }],
            imported_at: Utc::now(),
        }
    }

    fn sample_session() -> ReadingSession {
        let mut session = ReadingSession::new("session-1", "doc-1");
        session.state = crate::domain::ReaderState::Reading;
        session.playback_state = PlaybackState::Playing;
        session.playback_provider = Some("stub-playback".to_string());
        session.tts_provider = Some("kokoro".to_string());
        session.command_stt_provider = Some("vosk".to_string());
        session.voice = Some("if_sara".to_string());
        session.command_listening_active = true;
        session
    }

    fn sample_note() -> VoiceNote {
        VoiceNote {
            note_id: "note-1".to_string(),
            session_id: "session-1".to_string(),
            document_id: "doc-1".to_string(),
            position: ReadingPosition::default(),
            transcript: "Important".to_string(),
            transcription_provider: "whisper".to_string(),
            language: "it".to_string(),
            raw_audio_path: None,
            created_at: Utc::now(),
        }
    }

    fn sample_playback_snapshot() -> PlaybackSnapshot {
        PlaybackSnapshot {
            state: PlaybackState::Playing,
            last_action: "session-state".to_string(),
            document_id: Some("doc-1".to_string()),
            anchor: Some("section:0/chunk:0".to_string()),
            progress_units: 0,
            audio_reference: Some("/tmp/audio.wav".to_string()),
            provider_name: Some("stub-playback".to_string()),
            process_id: Some(42),
        }
    }

    #[test]
    fn app_snapshot_reports_idle_when_no_active_session() {
        let mut service = SessionQueryService::new(
            StubSessionRepository {
                active_session: None,
                saved_session: None,
            },
            StubDocumentRepository {
                documents: vec![sample_document()],
            },
            StubNoteRepository { notes: Vec::new() },
            StubDraftRepository,
            StubPlaybackEngine {
                last_hydrated: None,
                snapshot: PlaybackSnapshot {
                    state: PlaybackState::Stopped,
                    last_action: "idle".to_string(),
                    document_id: None,
                    anchor: None,
                    progress_units: 0,
                    audio_reference: None,
                    provider_name: Some("stub-playback".to_string()),
                    process_id: None,
                },
            },
        );

        let snapshot: AppSnapshot = service.app_snapshot();

        assert_eq!(snapshot.state, "idle");
        assert_eq!(snapshot.document_count, 1);
        assert_eq!(snapshot.playback_state.as_deref(), Some("stopped"));
        assert_eq!(snapshot.latest_document_id.as_deref(), Some("doc-1"));
    }

    #[test]
    fn session_snapshot_projects_active_session() {
        let mut service = SessionQueryService::new(
            StubSessionRepository {
                active_session: Some(sample_session()),
                saved_session: None,
            },
            StubDocumentRepository {
                documents: vec![sample_document()],
            },
            StubNoteRepository {
                notes: vec![sample_note()],
            },
            StubDraftRepository,
            StubPlaybackEngine {
                last_hydrated: None,
                snapshot: sample_playback_snapshot(),
            },
        );

        let snapshot: SessionSnapshot = service.session_snapshot().unwrap().unwrap();

        assert_eq!(snapshot.session_id, "session-1");
        assert_eq!(snapshot.document_id, "doc-1");
        assert_eq!(snapshot.section_title, "Intro");
        assert_eq!(snapshot.chunk_text, "Hello world");
        assert_eq!(snapshot.state, "reading");
        assert_eq!(snapshot.playback_state, "playing");
        assert_eq!(snapshot.notes_count, 1);
        assert!(snapshot.command_listening_active);
    }

    #[test]
    fn session_snapshot_errors_when_document_is_missing() {
        let mut service = SessionQueryService::new(
            StubSessionRepository {
                active_session: Some(sample_session()),
                saved_session: None,
            },
            StubDocumentRepository {
                documents: Vec::new(),
            },
            StubNoteRepository { notes: Vec::new() },
            StubDraftRepository,
            StubPlaybackEngine {
                last_hydrated: None,
                snapshot: sample_playback_snapshot(),
            },
        );

        let error = service.session_snapshot().unwrap_err();

        assert_eq!(
            error,
            SessionQueryError::MissingDocument {
                document_id: "doc-1".to_string(),
            }
        );
    }
}
