mod frontend;

use marginalia_core::application::{
    DocumentIngestionOutcome, DocumentIngestionService, IngestionError, SessionQueryError,
    SessionQueryService,
};
use marginalia_core::domain::{
    ReadingPosition, ReadingSession, ReaderState, VoiceNote, DEFAULT_CHUNK_TARGET_CHARS,
};
use marginalia_core::events::{DomainEvent, EventName};
use marginalia_core::frontend::{
    AppSnapshot, DocumentChunkView, DocumentListItem, DocumentSectionView, DocumentView,
    SessionSnapshot,
};
use marginalia_core::ports::{
    CommandRecognizer, DictationTranscriber, PlaybackEngine, RewriteGenerator, SpeechInterruptMonitor,
    SpeechSynthesizer, SynthesisError, SynthesisRequest, TopicSummarizer,
};
use marginalia_core::ports::storage::{
    DocumentRepository, NoteRepository, SessionRepository,
};
use marginalia_import_text::TextDocumentImporter;
use marginalia_provider_fake::{
    FakeCommandRecognizer, FakeDictationTranscriber, FakePlaybackEngine, FakeRewriteGenerator,
    FakeSpeechSynthesizer, FakeTopicSummarizer, InMemoryDocumentRepository,
    InMemoryNoteRepository, InMemoryRewriteDraftRepository, InMemorySessionRepository,
    RecordingEventPublisher,
};
use marginalia_storage_sqlite::{
    SQLiteDatabase, SQLiteDocumentRepository, SQLiteNoteRepository,
    SQLiteRewriteDraftRepository, SQLiteSessionRepository,
};
use std::collections::HashMap;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

pub use frontend::RuntimeFrontendResponse;

static SESSION_COUNTER: AtomicU64 = AtomicU64::new(1);
static NOTE_COUNTER: AtomicU64 = AtomicU64::new(1);
static EVENT_COUNTER: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeConfig {
    pub chunk_target_chars: usize,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            chunk_target_chars: DEFAULT_CHUNK_TARGET_CHARS,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeError {
    MissingActiveSession,
    MissingDocument { document_id: String },
    EmptyDocument { document_id: String },
    Synthesis(SynthesisError),
    Query(SessionQueryError),
}

impl Display for RuntimeError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingActiveSession => write!(f, "No active session is available in the runtime."),
            Self::MissingDocument { document_id } => {
                write!(f, "Document {} was not found in the runtime.", document_id)
            }
            Self::EmptyDocument { document_id } => {
                write!(f, "Document {} has no readable chunks.", document_id)
            }
            Self::Synthesis(error) => write!(f, "Speech synthesis failed: {error}"),
            Self::Query(error) => write!(f, "Runtime query failed: {:?}", error),
        }
    }
}

impl Error for RuntimeError {}

impl From<SessionQueryError> for RuntimeError {
    fn from(value: SessionQueryError) -> Self {
        Self::Query(value)
    }
}

impl From<SynthesisError> for RuntimeError {
    fn from(value: SynthesisError) -> Self {
        Self::Synthesis(value)
    }
}

pub struct FakeRuntime {
    config: RuntimeConfig,
    document_repository: InMemoryDocumentRepository,
    session_repository: InMemorySessionRepository,
    note_repository: InMemoryNoteRepository,
    draft_repository: InMemoryRewriteDraftRepository,
    importer: TextDocumentImporter,
    event_publisher: RecordingEventPublisher,
    playback_engine: Box<dyn PlaybackEngine + Send>,
    tts: Box<dyn SpeechSynthesizer + Send>,
    command_recognizer: Box<dyn CommandRecognizer + Send>,
    dictation_transcriber: Box<dyn DictationTranscriber + Send>,
    rewrite_generator: Box<dyn RewriteGenerator + Send>,
    topic_summarizer: Box<dyn TopicSummarizer + Send>,
    provider_doctor_blobs: HashMap<String, serde_json::Value>,
}

pub struct SqliteRuntime {
    config: RuntimeConfig,
    database: SQLiteDatabase,
    document_repository: SQLiteDocumentRepository,
    session_repository: SQLiteSessionRepository,
    note_repository: SQLiteNoteRepository,
    draft_repository: SQLiteRewriteDraftRepository,
    importer: TextDocumentImporter,
    event_publisher: RecordingEventPublisher,
    playback_engine: Box<dyn PlaybackEngine + Send>,
    tts: Box<dyn SpeechSynthesizer + Send>,
    command_recognizer: Box<dyn CommandRecognizer + Send>,
    dictation_transcriber: Box<dyn DictationTranscriber + Send>,
    rewrite_generator: Box<dyn RewriteGenerator + Send>,
    topic_summarizer: Box<dyn TopicSummarizer + Send>,
    provider_doctor_blobs: HashMap<String, serde_json::Value>,
}

impl Default for FakeRuntime {
    fn default() -> Self {
        Self {
            config: RuntimeConfig::default(),
            document_repository: InMemoryDocumentRepository::new(),
            session_repository: InMemorySessionRepository::new(),
            note_repository: InMemoryNoteRepository::new(),
            draft_repository: InMemoryRewriteDraftRepository::new(),
            importer: TextDocumentImporter,
            event_publisher: RecordingEventPublisher::new(),
            playback_engine: Box::new(FakePlaybackEngine::new()),
            tts: Box::new(FakeSpeechSynthesizer::new()),
            command_recognizer: Box::new(FakeCommandRecognizer::default()),
            dictation_transcriber: Box::new(FakeDictationTranscriber::default()),
            rewrite_generator: Box::new(FakeRewriteGenerator::new()),
            topic_summarizer: Box::new(FakeTopicSummarizer::new()),
            provider_doctor_blobs: HashMap::new(),
        }
    }
}

impl FakeRuntime {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_config(config: RuntimeConfig) -> Self {
        Self {
            config,
            ..Self::default()
        }
    }

    pub fn config(&self) -> RuntimeConfig {
        self.config
    }

    pub fn set_playback_engine(
        &mut self,
        playback_engine: impl PlaybackEngine + Send + 'static,
    ) {
        self.playback_engine = Box::new(playback_engine);
    }

    pub fn set_speech_synthesizer(
        &mut self,
        synthesizer: impl SpeechSynthesizer + Send + 'static,
    ) {
        self.tts = Box::new(synthesizer);
    }

    pub fn ingest_path(
        &mut self,
        source_path: &Path,
    ) -> Result<DocumentIngestionOutcome, IngestionError> {
        let mut service = DocumentIngestionService::new(
            &mut self.document_repository,
            &self.importer,
            self.event_publisher.clone(),
            self.config.chunk_target_chars,
        );
        service.ingest_path(source_path)
    }

    pub fn start_session(
        &mut self,
        document_id: &str,
    ) -> Result<ReadingSession, RuntimeError> {
        let document = self
            .document_repository
            .get_document(document_id)
            .ok_or_else(|| RuntimeError::MissingDocument {
                document_id: document_id.to_string(),
            })?;
        let position = ReadingPosition::default();
        let chunk = document
            .get_chunk(position.section_index, position.chunk_index)
            .ok_or_else(|| RuntimeError::EmptyDocument {
                document_id: document_id.to_string(),
            })?;

        let tts_provider = self.tts.describe_capabilities().provider_name;
        let synthesis = self.tts.synthesize(SynthesisRequest {
            text: chunk.text.clone(),
            voice: Some("narrator".to_string()),
            language: "it".to_string(),
        })?;
        let playback = self
            .playback_engine
            .start(&document, &position, Some(synthesis));

        let session_id = format!(
            "session-{}",
            SESSION_COUNTER.fetch_add(1, Ordering::Relaxed)
        );
        let mut session = ReadingSession::new(session_id, document.document_id.clone());
        session.state = ReaderState::Reading;
        session.playback_state = playback.state;
        session.position = position;
        session.last_command = Some("start_session".to_string());
        session.last_command_source = Some("runtime".to_string());
        session.voice = Some("narrator".to_string());
        session.tts_provider = Some(tts_provider);
        session.command_stt_provider = Some(self.command_recognizer.describe_capabilities().provider_name);
        session.playback_provider = playback.provider_name.clone();
        session.command_listening_active = true;
        session.command_language = Some("it".to_string());
        session.audio_reference = playback.audio_reference.clone();
        session.playback_process_id = playback.process_id;
        session.runtime_status = Some("active".to_string());
        session.touch();
        if let Err(e) = self.session_repository.save_session(session.clone()) {
            eprintln!("WARNING: failed to save session: {e}");
        }

        self.publish_runtime_event(
            EventName::ReadingStarted,
            HashMap::from([
                ("session_id".to_string(), session.session_id.clone()),
                ("document_id".to_string(), session.document_id.clone()),
                ("anchor".to_string(), session.position.anchor()),
            ]),
        );

        Ok(session)
    }

    pub fn app_snapshot(&mut self) -> AppSnapshot {
        let mut service = SessionQueryService::new(
            &mut self.session_repository,
            &mut self.document_repository,
            &mut self.note_repository,
            &mut self.draft_repository,
            &mut *self.playback_engine,
        );
        service.app_snapshot()
    }

    pub fn session_snapshot(&mut self) -> Result<Option<SessionSnapshot>, RuntimeError> {
        let mut service = SessionQueryService::new(
            &mut self.session_repository,
            &mut self.document_repository,
            &mut self.note_repository,
            &mut self.draft_repository,
            &mut *self.playback_engine,
        );
        service.session_snapshot().map_err(RuntimeError::from)
    }

    pub fn published_events(&self) -> Vec<DomainEvent> {
        self.event_publisher.published_events()
    }

    pub fn list_documents(&self) -> Vec<DocumentListItem> {
        self.document_repository
            .list_documents()
            .into_iter()
            .map(|document| DocumentListItem {
                chapter_count: document.chapter_count(),
                chunk_count: document.total_chunk_count(),
                document_id: document.document_id,
                title: document.title,
            })
            .collect()
    }

    pub fn document_view(&self, document_id: Option<&str>) -> Option<DocumentView> {
        build_document_view(
            &self.document_repository,
            &self.session_repository,
            document_id,
        )
    }

    pub fn pause_session(&mut self) -> Result<(), RuntimeError> {
        let mut session = self
            .session_repository
            .get_active_session()
            .ok_or(RuntimeError::MissingActiveSession)?;
        let playback = self.playback_engine.pause();
        session.state = ReaderState::Paused;
        session.playback_state = playback.state;
        session.last_command = Some("pause_session".to_string());
        session.runtime_status = Some("paused".to_string());
        session.touch();
        if let Err(e) = self.session_repository.save_session(session) {
            eprintln!("WARNING: failed to save session: {e}");
        }
        Ok(())
    }

    pub fn resume_session(&mut self) -> Result<(), RuntimeError> {
        let mut session = self
            .session_repository
            .get_active_session()
            .ok_or(RuntimeError::MissingActiveSession)?;
        let playback = self.playback_engine.resume();
        session.state = ReaderState::Reading;
        session.playback_state = playback.state;
        session.last_command = Some("resume_session".to_string());
        session.runtime_status = Some("active".to_string());
        session.touch();
        if let Err(e) = self.session_repository.save_session(session) {
            eprintln!("WARNING: failed to save session: {e}");
        }
        Ok(())
    }

    pub fn stop_session(&mut self) -> Result<(), RuntimeError> {
        let mut session = self
            .session_repository
            .get_active_session()
            .ok_or(RuntimeError::MissingActiveSession)?;
        let playback = self.playback_engine.stop();
        session.state = ReaderState::Idle;
        session.playback_state = playback.state;
        session.last_command = Some("stop_session".to_string());
        session.runtime_status = Some("stopped".to_string());
        session.command_listening_active = false;
        session.is_active = false;
        session.touch();
        if let Err(e) = self.session_repository.save_session(session) {
            eprintln!("WARNING: failed to save session: {e}");
        }
        Ok(())
    }

    pub fn next_chunk(&mut self) -> Result<(), RuntimeError> {
        self.seek_relative_chunk(1)
    }

    pub fn previous_chunk(&mut self) -> Result<(), RuntimeError> {
        self.seek_relative_chunk(-1)
    }

    pub fn next_chapter(&mut self) -> Result<(), RuntimeError> {
        self.seek_chapter(1, false)
    }

    pub fn previous_chapter(&mut self) -> Result<(), RuntimeError> {
        self.seek_chapter(-1, false)
    }

    pub fn restart_chapter(&mut self) -> Result<(), RuntimeError> {
        self.seek_chapter(0, true)
    }

    pub fn repeat_chunk(&mut self) -> Result<(), RuntimeError> {
        self.replay_current_position("repeat_chunk")
    }

    pub fn create_note(&mut self, text: &str) -> Result<VoiceNote, RuntimeError> {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Err(RuntimeError::MissingActiveSession);
        }

        let mut session = self
            .session_repository
            .get_active_session()
            .ok_or(RuntimeError::MissingActiveSession)?;
        let note = VoiceNote {
            note_id: format!("note-{}", NOTE_COUNTER.fetch_add(1, Ordering::Relaxed)),
            session_id: session.session_id.clone(),
            document_id: session.document_id.clone(),
            position: session.position.clone(),
            transcript: trimmed.to_string(),
            transcription_provider: "manual".to_string(),
            language: session
                .command_language
                .clone()
                .unwrap_or_else(|| "und".to_string()),
            raw_audio_path: None,
            created_at: chrono::Utc::now(),
        };
        if let Err(e) = self.note_repository.save_note(note.clone()) {
            eprintln!("WARNING: failed to save note: {e}");
        }
        session.last_command = Some("create_note".to_string());
        session.touch();
        if let Err(e) = self.session_repository.save_session(session.clone()) {
            eprintln!("WARNING: failed to save session: {e}");
        }
        self.publish_runtime_event(
            EventName::NoteSaved,
            HashMap::from([
                ("note_id".to_string(), note.note_id.clone()),
                ("document_id".to_string(), note.document_id.clone()),
                ("anchor".to_string(), note.anchor()),
            ]),
        );
        Ok(note)
    }

    pub fn doctor_report(&self) -> serde_json::Value {
        let playback_name = self.playback_engine.describe_capabilities().provider_name;
        let tts_name = self.tts.describe_capabilities().provider_name;
        let stt_name = self.command_recognizer.describe_capabilities().provider_name;
        let dictation_name = self.dictation_transcriber.describe_capabilities().provider_name;

        let mut checks = serde_json::json!({
            "playback": { "ready": true, "command": "beta-runtime" },
            "kokoro": { "ready": false },
            "piper": { "ready": false },
            "vosk": { "ready": false },
            "whisper_dictation_stt": { "ready": false },
        });
        if let Some(map) = checks.as_object_mut() {
            for (key, blob) in &self.provider_doctor_blobs {
                map.insert(key.clone(), blob.clone());
            }
        }

        serde_json::json!({
            "providers": {
                "tts": tts_name,
                "command_stt": stt_name,
                "dictation_stt": dictation_name,
                "playback": playback_name,
            },
            "resolved_providers": {
                "tts": tts_name,
                "command_stt": stt_name,
                "dictation_stt": dictation_name,
                "playback": playback_name,
            },
            "provider_checks": checks,
        })
    }

    pub fn set_command_recognizer(&mut self, recognizer: impl CommandRecognizer + Send + 'static) {
        self.command_recognizer = Box::new(recognizer);
    }

    pub fn open_command_monitor(&mut self) -> Box<dyn SpeechInterruptMonitor> {
        self.command_recognizer.open_interrupt_monitor()
    }

    pub fn set_dictation_transcriber(
        &mut self,
        transcriber: impl DictationTranscriber + Send + 'static,
    ) {
        self.dictation_transcriber = Box::new(transcriber);
    }

    pub fn dictation_transcriber(&self) -> &dyn DictationTranscriber {
        self.dictation_transcriber.as_ref()
    }

    pub fn rewrite_generator(&self) -> &dyn RewriteGenerator {
        self.rewrite_generator.as_ref()
    }

    pub fn topic_summarizer(&self) -> &dyn TopicSummarizer {
        self.topic_summarizer.as_ref()
    }

    fn publish_runtime_event(&self, name: EventName, payload: HashMap<String, String>) {
        use marginalia_core::ports::events::EventPublisher;

        self.event_publisher.publish(DomainEvent {
            name,
            payload,
            event_id: format!("event-{}", EVENT_COUNTER.fetch_add(1, Ordering::Relaxed)),
            occurred_at: chrono::Utc::now(),
        });
    }

    fn seek_relative_chunk(&mut self, delta: isize) -> Result<(), RuntimeError> {
        let mut session = self
            .session_repository
            .get_active_session()
            .ok_or(RuntimeError::MissingActiveSession)?;
        let document = self
            .document_repository
            .get_document(&session.document_id)
            .ok_or_else(|| RuntimeError::MissingDocument {
                document_id: session.document_id.clone(),
            })?;

        let mut positions = Vec::new();
        for section in &document.sections {
            for chunk in &section.chunks {
                positions.push((section.index, chunk.index));
            }
        }
        let current_index = positions
            .iter()
            .position(|(section_index, chunk_index)| {
                *section_index == session.position.section_index
                    && *chunk_index == session.position.chunk_index
            })
            .ok_or_else(|| RuntimeError::EmptyDocument {
                document_id: session.document_id.clone(),
            })?;

        let target_index = if delta < 0 {
            current_index.saturating_sub(delta.unsigned_abs())
        } else {
            (current_index + delta as usize).min(positions.len().saturating_sub(1))
        };
        let (section_index, chunk_index) = positions[target_index];

        session.position.section_index = section_index;
        session.position.chunk_index = chunk_index;
        session.position.char_offset = 0;
        self.replay_session_at_position(session, "seek_chunk")
    }

    fn seek_chapter(&mut self, delta: isize, restart_current: bool) -> Result<(), RuntimeError> {
        let mut session = self
            .session_repository
            .get_active_session()
            .ok_or(RuntimeError::MissingActiveSession)?;
        let document = self
            .document_repository
            .get_document(&session.document_id)
            .ok_or_else(|| RuntimeError::MissingDocument {
                document_id: session.document_id.clone(),
            })?;

        let current = session.position.section_index as isize;
        let target_section = if restart_current {
            current
        } else {
            (current + delta).clamp(0, document.sections.len().saturating_sub(1) as isize)
        } as usize;

        session.position.section_index = target_section;
        session.position.chunk_index = 0;
        session.position.char_offset = 0;
        self.replay_session_at_position(session, "seek_chapter")
    }

    fn replay_current_position(&mut self, command_name: &str) -> Result<(), RuntimeError> {
        let session = self
            .session_repository
            .get_active_session()
            .ok_or(RuntimeError::MissingActiveSession)?;
        self.replay_session_at_position(session, command_name)
    }

    fn replay_session_at_position(
        &mut self,
        mut session: ReadingSession,
        command_name: &str,
    ) -> Result<(), RuntimeError> {
        let document = self
            .document_repository
            .get_document(&session.document_id)
            .ok_or_else(|| RuntimeError::MissingDocument {
                document_id: session.document_id.clone(),
            })?;
        let chunk = document
            .get_chunk(session.position.section_index, session.position.chunk_index)
            .ok_or_else(|| RuntimeError::EmptyDocument {
                document_id: session.document_id.clone(),
            })?;

        let synthesis = self.tts.synthesize(SynthesisRequest {
            text: chunk.text.clone(),
            voice: session.voice.clone().or(Some("narrator".to_string())),
            language: session
                .command_language
                .clone()
                .unwrap_or_else(|| "it".to_string()),
        })?;
        let playback = self
            .playback_engine
            .start(&document, &session.position, Some(synthesis));

        session.state = ReaderState::Reading;
        session.playback_state = playback.state;
        session.last_command = Some(command_name.to_string());
        session.audio_reference = playback.audio_reference.clone();
        session.playback_process_id = playback.process_id;
        session.runtime_status = Some("active".to_string());
        session.touch();
        if let Err(e) = self.session_repository.save_session(session) {
            eprintln!("WARNING: failed to save session: {e}");
        }
        Ok(())
    }
}

fn build_document_view<D, S>(
    document_repository: &D,
    session_repository: &S,
    document_id: Option<&str>,
) -> Option<DocumentView>
where
    D: DocumentRepository,
    S: SessionRepository,
{
    let active_session = session_repository.get_active_session();
    let target_document_id = document_id
        .map(ToString::to_string)
        .or_else(|| active_session.as_ref().map(|session| session.document_id.clone()))
        .or_else(|| {
            document_repository
                .list_documents()
                .into_iter()
                .next()
                .map(|document| document.document_id)
        })?;

    let document = document_repository.get_document(&target_document_id)?;
    let active_section_index = active_session
        .as_ref()
        .filter(|session| session.document_id == document.document_id)
        .map(|session| session.position.section_index);
    let active_chunk_index = active_session
        .as_ref()
        .filter(|session| session.document_id == document.document_id)
        .map(|session| session.position.chunk_index);

    Some(DocumentView {
        active_chunk_index,
        active_section_index,
        chapter_count: document.chapter_count(),
        chunk_count: document.total_chunk_count(),
        document_id: document.document_id.clone(),
        sections: document
            .sections
            .iter()
            .map(|section| DocumentSectionView {
                chunk_count: section.chunk_count(),
                chunks: section
                    .chunks
                    .iter()
                    .map(|chunk| {
                        let is_active = active_section_index == Some(section.index)
                            && active_chunk_index == Some(chunk.index);
                        let is_read = active_section_index
                            .map(|active_section| {
                                section.index < active_section
                                    || (section.index == active_section
                                        && active_chunk_index
                                            .map(|active_chunk| chunk.index < active_chunk)
                                            .unwrap_or(false))
                            })
                            .unwrap_or(false);

                        DocumentChunkView {
                            anchor: format!("section:{}/chunk:{}", section.index, chunk.index),
                            char_end: chunk.char_end,
                            char_start: chunk.char_start,
                            index: chunk.index,
                            is_active,
                            is_read,
                            text: chunk.text.clone(),
                        }
                    })
                    .collect(),
                index: section.index,
                source_anchor: section.source_anchor.clone(),
                title: section.title.clone(),
            })
            .collect(),
        source_path: document.source_path.display().to_string(),
        title: document.title,
    })
}

impl SqliteRuntime {
    pub fn open_in_memory() -> rusqlite::Result<Self> {
        Self::open_in_memory_with_config(RuntimeConfig::default())
    }

    pub fn open_in_memory_with_config(config: RuntimeConfig) -> rusqlite::Result<Self> {
        let database = SQLiteDatabase::open_in_memory()?;
        let connection = database.connection();

        Ok(Self {
            config,
            database,
            document_repository: SQLiteDocumentRepository::new(connection.clone()),
            session_repository: SQLiteSessionRepository::new(connection.clone()),
            note_repository: SQLiteNoteRepository::new(connection.clone()),
            draft_repository: SQLiteRewriteDraftRepository::new(connection),
            importer: TextDocumentImporter,
            event_publisher: RecordingEventPublisher::new(),
            playback_engine: Box::new(FakePlaybackEngine::new()),
            tts: Box::new(FakeSpeechSynthesizer::new()),
            command_recognizer: Box::new(FakeCommandRecognizer::default()),
            dictation_transcriber: Box::new(FakeDictationTranscriber::default()),
            rewrite_generator: Box::new(FakeRewriteGenerator::new()),
            topic_summarizer: Box::new(FakeTopicSummarizer::new()),
            provider_doctor_blobs: HashMap::new(),
        })
    }

    pub fn open(path: impl AsRef<Path>) -> rusqlite::Result<Self> {
        Self::open_with_config(path, RuntimeConfig::default())
    }

    pub fn open_with_config(
        path: impl AsRef<Path>,
        config: RuntimeConfig,
    ) -> rusqlite::Result<Self> {
        let database = SQLiteDatabase::open(path)?;
        let connection = database.connection();

        Ok(Self {
            config,
            database,
            document_repository: SQLiteDocumentRepository::new(connection.clone()),
            session_repository: SQLiteSessionRepository::new(connection.clone()),
            note_repository: SQLiteNoteRepository::new(connection.clone()),
            draft_repository: SQLiteRewriteDraftRepository::new(connection),
            importer: TextDocumentImporter,
            event_publisher: RecordingEventPublisher::new(),
            playback_engine: Box::new(FakePlaybackEngine::new()),
            tts: Box::new(FakeSpeechSynthesizer::new()),
            command_recognizer: Box::new(FakeCommandRecognizer::default()),
            dictation_transcriber: Box::new(FakeDictationTranscriber::default()),
            rewrite_generator: Box::new(FakeRewriteGenerator::new()),
            topic_summarizer: Box::new(FakeTopicSummarizer::new()),
            provider_doctor_blobs: HashMap::new(),
        })
    }

    pub fn config(&self) -> RuntimeConfig {
        self.config
    }

    pub fn set_playback_engine(
        &mut self,
        playback_engine: impl PlaybackEngine + Send + 'static,
    ) {
        self.playback_engine = Box::new(playback_engine);
    }

    pub fn set_speech_synthesizer(
        &mut self,
        synthesizer: impl SpeechSynthesizer + Send + 'static,
    ) {
        self.tts = Box::new(synthesizer);
    }

    pub fn set_provider_doctor_blob(&mut self, key: impl Into<String>, blob: serde_json::Value) {
        self.provider_doctor_blobs.insert(key.into(), blob);
    }

    pub fn database(&self) -> &SQLiteDatabase {
        &self.database
    }

    pub fn ingest_path(
        &mut self,
        source_path: &Path,
    ) -> Result<DocumentIngestionOutcome, IngestionError> {
        let mut service = DocumentIngestionService::new(
            &mut self.document_repository,
            &self.importer,
            self.event_publisher.clone(),
            self.config.chunk_target_chars,
        );
        service.ingest_path(source_path)
    }

    pub fn start_session(&mut self, document_id: &str) -> Result<ReadingSession, RuntimeError> {
        let document = self
            .document_repository
            .get_document(document_id)
            .ok_or_else(|| RuntimeError::MissingDocument {
                document_id: document_id.to_string(),
            })?;
        let position = ReadingPosition::default();
        let chunk = document
            .get_chunk(position.section_index, position.chunk_index)
            .ok_or_else(|| RuntimeError::EmptyDocument {
                document_id: document_id.to_string(),
            })?;

        let tts_provider = self.tts.describe_capabilities().provider_name;
        let synthesis = self.tts.synthesize(SynthesisRequest {
            text: chunk.text.clone(),
            voice: Some("narrator".to_string()),
            language: "it".to_string(),
        })?;
        let playback = self
            .playback_engine
            .start(&document, &position, Some(synthesis));

        let session_id = format!(
            "session-{}",
            SESSION_COUNTER.fetch_add(1, Ordering::Relaxed)
        );
        let mut session = ReadingSession::new(session_id, document.document_id.clone());
        session.state = ReaderState::Reading;
        session.playback_state = playback.state;
        session.position = position;
        session.last_command = Some("start_session".to_string());
        session.last_command_source = Some("runtime".to_string());
        session.voice = Some("narrator".to_string());
        session.tts_provider = Some(tts_provider);
        session.command_stt_provider = Some(self.command_recognizer.describe_capabilities().provider_name);
        session.playback_provider = playback.provider_name.clone();
        session.command_listening_active = true;
        session.command_language = Some("it".to_string());
        session.audio_reference = playback.audio_reference.clone();
        session.playback_process_id = playback.process_id;
        session.runtime_status = Some("active".to_string());
        session.touch();
        if let Err(e) = self.session_repository.save_session(session.clone()) {
            eprintln!("WARNING: failed to save session: {e}");
        }

        self.publish_runtime_event(
            EventName::ReadingStarted,
            HashMap::from([
                ("session_id".to_string(), session.session_id.clone()),
                ("document_id".to_string(), session.document_id.clone()),
                ("anchor".to_string(), session.position.anchor()),
            ]),
        );

        Ok(session)
    }

    pub fn app_snapshot(&mut self) -> AppSnapshot {
        let mut service = SessionQueryService::new(
            &mut self.session_repository,
            &mut self.document_repository,
            &mut self.note_repository,
            &mut self.draft_repository,
            &mut *self.playback_engine,
        );
        service.app_snapshot()
    }

    pub fn session_snapshot(&mut self) -> Result<Option<SessionSnapshot>, RuntimeError> {
        let mut service = SessionQueryService::new(
            &mut self.session_repository,
            &mut self.document_repository,
            &mut self.note_repository,
            &mut self.draft_repository,
            &mut *self.playback_engine,
        );
        service.session_snapshot().map_err(RuntimeError::from)
    }

    pub fn published_events(&self) -> Vec<DomainEvent> {
        self.event_publisher.published_events()
    }

    pub fn list_documents(&self) -> Vec<DocumentListItem> {
        self.document_repository
            .list_documents()
            .into_iter()
            .map(|document| DocumentListItem {
                chapter_count: document.chapter_count(),
                chunk_count: document.total_chunk_count(),
                document_id: document.document_id,
                title: document.title,
            })
            .collect()
    }

    pub fn document_view(&self, document_id: Option<&str>) -> Option<DocumentView> {
        build_document_view(
            &self.document_repository,
            &self.session_repository,
            document_id,
        )
    }

    pub fn pause_session(&mut self) -> Result<(), RuntimeError> {
        let mut session = self
            .session_repository
            .get_active_session()
            .ok_or(RuntimeError::MissingActiveSession)?;
        let playback = self.playback_engine.pause();
        session.state = ReaderState::Paused;
        session.playback_state = playback.state;
        session.last_command = Some("pause_session".to_string());
        session.runtime_status = Some("paused".to_string());
        session.touch();
        if let Err(e) = self.session_repository.save_session(session) {
            eprintln!("WARNING: failed to save session: {e}");
        }
        Ok(())
    }

    pub fn resume_session(&mut self) -> Result<(), RuntimeError> {
        let mut session = self
            .session_repository
            .get_active_session()
            .ok_or(RuntimeError::MissingActiveSession)?;
        let playback = self.playback_engine.resume();
        session.state = ReaderState::Reading;
        session.playback_state = playback.state;
        session.last_command = Some("resume_session".to_string());
        session.runtime_status = Some("active".to_string());
        session.touch();
        if let Err(e) = self.session_repository.save_session(session) {
            eprintln!("WARNING: failed to save session: {e}");
        }
        Ok(())
    }

    pub fn stop_session(&mut self) -> Result<(), RuntimeError> {
        let mut session = self
            .session_repository
            .get_active_session()
            .ok_or(RuntimeError::MissingActiveSession)?;
        let playback = self.playback_engine.stop();
        session.state = ReaderState::Idle;
        session.playback_state = playback.state;
        session.last_command = Some("stop_session".to_string());
        session.runtime_status = Some("stopped".to_string());
        session.command_listening_active = false;
        session.is_active = false;
        session.touch();
        if let Err(e) = self.session_repository.save_session(session) {
            eprintln!("WARNING: failed to save session: {e}");
        }
        Ok(())
    }

    pub fn next_chunk(&mut self) -> Result<(), RuntimeError> {
        self.seek_relative_chunk(1)
    }

    pub fn previous_chunk(&mut self) -> Result<(), RuntimeError> {
        self.seek_relative_chunk(-1)
    }

    pub fn next_chapter(&mut self) -> Result<(), RuntimeError> {
        self.seek_chapter(1, false)
    }

    pub fn previous_chapter(&mut self) -> Result<(), RuntimeError> {
        self.seek_chapter(-1, false)
    }

    pub fn restart_chapter(&mut self) -> Result<(), RuntimeError> {
        self.seek_chapter(0, true)
    }

    pub fn repeat_chunk(&mut self) -> Result<(), RuntimeError> {
        self.replay_current_position("repeat_chunk")
    }

    pub fn create_note(&mut self, text: &str) -> Result<VoiceNote, RuntimeError> {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Err(RuntimeError::MissingActiveSession);
        }

        let mut session = self
            .session_repository
            .get_active_session()
            .ok_or(RuntimeError::MissingActiveSession)?;
        let note = VoiceNote {
            note_id: format!("note-{}", NOTE_COUNTER.fetch_add(1, Ordering::Relaxed)),
            session_id: session.session_id.clone(),
            document_id: session.document_id.clone(),
            position: session.position.clone(),
            transcript: trimmed.to_string(),
            transcription_provider: "manual".to_string(),
            language: session
                .command_language
                .clone()
                .unwrap_or_else(|| "und".to_string()),
            raw_audio_path: None,
            created_at: chrono::Utc::now(),
        };
        if let Err(e) = self.note_repository.save_note(note.clone()) {
            eprintln!("WARNING: failed to save note: {e}");
        }
        session.last_command = Some("create_note".to_string());
        session.touch();
        if let Err(e) = self.session_repository.save_session(session.clone()) {
            eprintln!("WARNING: failed to save session: {e}");
        }
        self.publish_runtime_event(
            EventName::NoteSaved,
            HashMap::from([
                ("note_id".to_string(), note.note_id.clone()),
                ("document_id".to_string(), note.document_id.clone()),
                ("anchor".to_string(), note.anchor()),
            ]),
        );
        Ok(note)
    }

    pub fn doctor_report(&self) -> serde_json::Value {
        let playback_name = self.playback_engine.describe_capabilities().provider_name;
        let tts_name = self.tts.describe_capabilities().provider_name;
        let stt_name = self.command_recognizer.describe_capabilities().provider_name;
        let dictation_name = self.dictation_transcriber.describe_capabilities().provider_name;

        let mut checks = serde_json::json!({
            "playback": { "ready": true, "command": "beta-runtime" },
            "kokoro": { "ready": false },
            "piper": { "ready": false },
            "vosk": { "ready": false },
            "whisper_dictation_stt": { "ready": false },
        });
        if let Some(map) = checks.as_object_mut() {
            for (key, blob) in &self.provider_doctor_blobs {
                map.insert(key.clone(), blob.clone());
            }
        }

        serde_json::json!({
            "providers": {
                "tts": tts_name,
                "command_stt": stt_name,
                "dictation_stt": dictation_name,
                "playback": playback_name,
            },
            "resolved_providers": {
                "tts": tts_name,
                "command_stt": stt_name,
                "dictation_stt": dictation_name,
                "playback": playback_name,
            },
            "provider_checks": checks,
        })
    }

    pub fn set_command_recognizer(&mut self, recognizer: impl CommandRecognizer + Send + 'static) {
        self.command_recognizer = Box::new(recognizer);
    }

    pub fn open_command_monitor(&mut self) -> Box<dyn SpeechInterruptMonitor> {
        self.command_recognizer.open_interrupt_monitor()
    }

    pub fn set_dictation_transcriber(
        &mut self,
        transcriber: impl DictationTranscriber + Send + 'static,
    ) {
        self.dictation_transcriber = Box::new(transcriber);
    }

    pub fn dictation_transcriber(&self) -> &dyn DictationTranscriber {
        self.dictation_transcriber.as_ref()
    }

    pub fn rewrite_generator(&self) -> &dyn RewriteGenerator {
        self.rewrite_generator.as_ref()
    }

    pub fn topic_summarizer(&self) -> &dyn TopicSummarizer {
        self.topic_summarizer.as_ref()
    }

    fn publish_runtime_event(&self, name: EventName, payload: HashMap<String, String>) {
        use marginalia_core::ports::events::EventPublisher;

        self.event_publisher.publish(DomainEvent {
            name,
            payload,
            event_id: format!("event-{}", EVENT_COUNTER.fetch_add(1, Ordering::Relaxed)),
            occurred_at: chrono::Utc::now(),
        });
    }

    fn seek_relative_chunk(&mut self, delta: isize) -> Result<(), RuntimeError> {
        let mut session = self
            .session_repository
            .get_active_session()
            .ok_or(RuntimeError::MissingActiveSession)?;
        let document = self
            .document_repository
            .get_document(&session.document_id)
            .ok_or_else(|| RuntimeError::MissingDocument {
                document_id: session.document_id.clone(),
            })?;

        let mut positions = Vec::new();
        for section in &document.sections {
            for chunk in &section.chunks {
                positions.push((section.index, chunk.index));
            }
        }
        let current_index = positions
            .iter()
            .position(|(section_index, chunk_index)| {
                *section_index == session.position.section_index
                    && *chunk_index == session.position.chunk_index
            })
            .ok_or_else(|| RuntimeError::EmptyDocument {
                document_id: session.document_id.clone(),
            })?;

        let target_index = if delta < 0 {
            current_index.saturating_sub(delta.unsigned_abs())
        } else {
            (current_index + delta as usize).min(positions.len().saturating_sub(1))
        };
        let (section_index, chunk_index) = positions[target_index];

        session.position.section_index = section_index;
        session.position.chunk_index = chunk_index;
        session.position.char_offset = 0;
        self.replay_session_at_position(session, "seek_chunk")
    }

    fn seek_chapter(&mut self, delta: isize, restart_current: bool) -> Result<(), RuntimeError> {
        let mut session = self
            .session_repository
            .get_active_session()
            .ok_or(RuntimeError::MissingActiveSession)?;
        let document = self
            .document_repository
            .get_document(&session.document_id)
            .ok_or_else(|| RuntimeError::MissingDocument {
                document_id: session.document_id.clone(),
            })?;

        let current = session.position.section_index as isize;
        let target_section = if restart_current {
            current
        } else {
            (current + delta).clamp(0, document.sections.len().saturating_sub(1) as isize)
        } as usize;

        session.position.section_index = target_section;
        session.position.chunk_index = 0;
        session.position.char_offset = 0;
        self.replay_session_at_position(session, "seek_chapter")
    }

    fn replay_current_position(&mut self, command_name: &str) -> Result<(), RuntimeError> {
        let session = self
            .session_repository
            .get_active_session()
            .ok_or(RuntimeError::MissingActiveSession)?;
        self.replay_session_at_position(session, command_name)
    }

    fn replay_session_at_position(
        &mut self,
        mut session: ReadingSession,
        command_name: &str,
    ) -> Result<(), RuntimeError> {
        let document = self
            .document_repository
            .get_document(&session.document_id)
            .ok_or_else(|| RuntimeError::MissingDocument {
                document_id: session.document_id.clone(),
            })?;
        let chunk = document
            .get_chunk(session.position.section_index, session.position.chunk_index)
            .ok_or_else(|| RuntimeError::EmptyDocument {
                document_id: session.document_id.clone(),
            })?;

        let synthesis = self.tts.synthesize(SynthesisRequest {
            text: chunk.text.clone(),
            voice: session.voice.clone().or(Some("narrator".to_string())),
            language: session
                .command_language
                .clone()
                .unwrap_or_else(|| "it".to_string()),
        })?;
        let playback = self
            .playback_engine
            .start(&document, &session.position, Some(synthesis));

        session.state = ReaderState::Reading;
        session.playback_state = playback.state;
        session.last_command = Some(command_name.to_string());
        session.audio_reference = playback.audio_reference.clone();
        session.playback_process_id = playback.process_id;
        session.runtime_status = Some("active".to_string());
        session.touch();
        if let Err(e) = self.session_repository.save_session(session) {
            eprintln!("WARNING: failed to save session: {e}");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{FakeRuntime, SqliteRuntime};
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_path(extension: &str) -> PathBuf {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("marginalia-runtime-test-{}.{}", timestamp, extension))
    }

    #[test]
    fn fake_runtime_can_ingest_and_report_idle_snapshot() {
        let path = temp_path("md");
        fs::write(&path, "# Intro\n\nAlpha beta gamma.").unwrap();

        let mut runtime = FakeRuntime::new();
        let outcome = runtime.ingest_path(&path).unwrap();
        let snapshot = runtime.app_snapshot();

        assert!(outcome.document.title.starts_with("Marginalia Runtime Test"));
        assert_eq!(snapshot.state, "idle");
        assert_eq!(snapshot.document_count, 1);

        let _ = fs::remove_file(path);
    }

    #[test]
    fn fake_runtime_can_start_session_and_project_session_snapshot() {
        let path = temp_path("md");
        fs::write(&path, "# Intro\n\nAlpha beta gamma.").unwrap();

        let mut runtime = FakeRuntime::new();
        let outcome = runtime.ingest_path(&path).unwrap();
        let session = runtime.start_session(&outcome.document.document_id).unwrap();
        let snapshot = runtime.session_snapshot().unwrap().unwrap();

        assert_eq!(session.document_id, outcome.document.document_id);
        assert_eq!(snapshot.state, "reading");
        assert_eq!(snapshot.playback_state, "playing");
        assert_eq!(snapshot.document_id, outcome.document.document_id);

        let _ = fs::remove_file(path);
    }

    #[test]
    fn fake_runtime_publishes_ingestion_and_start_events() {
        let path = temp_path("txt");
        fs::write(&path, "# Intro\n\nAlpha beta gamma.").unwrap();

        let mut runtime = FakeRuntime::new();
        let outcome = runtime.ingest_path(&path).unwrap();
        let _ = runtime.start_session(&outcome.document.document_id).unwrap();

        let events = runtime.published_events();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].name, marginalia_core::events::EventName::DocumentIngested);
        assert_eq!(events[1].name, marginalia_core::events::EventName::ReadingStarted);

        let _ = fs::remove_file(path);
    }

    #[test]
    fn sqlite_runtime_can_ingest_and_report_idle_snapshot() {
        let path = temp_path("md");
        fs::write(&path, "# Intro\n\nAlpha beta gamma.").unwrap();

        let mut runtime = SqliteRuntime::open_in_memory().unwrap();
        let outcome = runtime.ingest_path(&path).unwrap();
        let snapshot = runtime.app_snapshot();

        assert!(outcome.document.title.starts_with("Marginalia Runtime Test"));
        assert_eq!(snapshot.state, "idle");
        assert_eq!(snapshot.document_count, 1);

        let _ = fs::remove_file(path);
    }

    #[test]
    fn sqlite_runtime_can_start_session_and_project_session_snapshot() {
        let path = temp_path("md");
        fs::write(&path, "# Intro\n\nAlpha beta gamma.").unwrap();

        let mut runtime = SqliteRuntime::open_in_memory().unwrap();
        let outcome = runtime.ingest_path(&path).unwrap();
        let session = runtime.start_session(&outcome.document.document_id).unwrap();
        let snapshot = runtime.session_snapshot().unwrap().unwrap();

        assert_eq!(session.document_id, outcome.document.document_id);
        assert_eq!(snapshot.state, "reading");
        assert_eq!(snapshot.playback_state, "playing");
        assert_eq!(snapshot.document_id, outcome.document.document_id);

        let _ = fs::remove_file(path);
    }

    #[test]
    fn sqlite_runtime_lists_documents_and_builds_document_view() {
        let path = temp_path("md");
        fs::write(
            &path,
            "# Intro\n\nAlpha beta gamma.\n\n# Second\n\nDelta epsilon zeta.",
        )
        .unwrap();

        let mut runtime = SqliteRuntime::open_in_memory().unwrap();
        let outcome = runtime.ingest_path(&path).unwrap();
        let documents = runtime.list_documents();
        let view = runtime
            .document_view(Some(&outcome.document.document_id))
            .unwrap();

        assert_eq!(documents.len(), 1);
        assert_eq!(documents[0].document_id, outcome.document.document_id);
        assert_eq!(view.document_id, outcome.document.document_id);
        assert_eq!(view.chapter_count, 2);
        assert!(!view.sections.is_empty());

        let _ = fs::remove_file(path);
    }

    #[test]
    fn sqlite_runtime_supports_navigation_commands() {
        let path = temp_path("txt");
        fs::write(
            &path,
            "Alpha beta gamma delta epsilon zeta eta theta iota kappa lambda mu.\n\nNu xi omicron pi rho sigma tau upsilon phi chi psi omega.",
        )
        .unwrap();

        let mut runtime = SqliteRuntime::open_in_memory_with_config(super::RuntimeConfig {
            chunk_target_chars: 20,
        })
        .unwrap();
        let outcome = runtime.ingest_path(&path).unwrap();
        runtime.start_session(&outcome.document.document_id).unwrap();

        let before = runtime.session_snapshot().unwrap().unwrap();
        runtime.next_chunk().unwrap();
        let after_next = runtime.session_snapshot().unwrap().unwrap();
        runtime.previous_chunk().unwrap();
        let after_previous = runtime.session_snapshot().unwrap().unwrap();
        runtime.restart_chapter().unwrap();
        let after_restart = runtime.session_snapshot().unwrap().unwrap();

        assert_ne!(
            before.anchor, after_next.anchor,
            "next_chunk should advance the reading position"
        );
        assert_eq!(before.anchor, after_previous.anchor);
        assert_eq!(after_restart.anchor, "section:0/chunk:0");

        let _ = fs::remove_file(path);
    }

    #[test]
    fn sqlite_runtime_can_create_note_for_active_session() {
        let path = temp_path("md");
        fs::write(&path, "# Intro\n\nAlpha beta gamma.").unwrap();

        let mut runtime = SqliteRuntime::open_in_memory().unwrap();
        let outcome = runtime.ingest_path(&path).unwrap();
        runtime.start_session(&outcome.document.document_id).unwrap();
        let note = runtime.create_note("remember this").unwrap();
        let snapshot = runtime.session_snapshot().unwrap().unwrap();

        assert_eq!(note.document_id, outcome.document.document_id);
        assert_eq!(note.transcript, "remember this");
        assert_eq!(snapshot.notes_count, 1);

        let _ = fs::remove_file(path);
    }
}
