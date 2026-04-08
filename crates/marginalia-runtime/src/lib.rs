use marginalia_core::application::{
    DocumentIngestionOutcome, DocumentIngestionService, SessionQueryError, SessionQueryService,
};
use marginalia_core::domain::{
    ReadingPosition, ReadingSession, ReaderState, DEFAULT_CHUNK_TARGET_CHARS,
};
use marginalia_core::events::{DomainEvent, EventName};
use marginalia_core::frontend::{AppSnapshot, SessionSnapshot};
use marginalia_core::ports::{
    PlaybackEngine, SpeechSynthesizer, SynthesisRequest,
};
use marginalia_core::ports::storage::{DocumentRepository, SessionRepository};
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

static SESSION_COUNTER: AtomicU64 = AtomicU64::new(1);
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
    MissingDocument { document_id: String },
    EmptyDocument { document_id: String },
    Query(SessionQueryError),
}

impl Display for RuntimeError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingDocument { document_id } => {
                write!(f, "Document {} was not found in the runtime.", document_id)
            }
            Self::EmptyDocument { document_id } => {
                write!(f, "Document {} has no readable chunks.", document_id)
            }
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

pub struct FakeRuntime {
    config: RuntimeConfig,
    document_repository: InMemoryDocumentRepository,
    session_repository: InMemorySessionRepository,
    note_repository: InMemoryNoteRepository,
    draft_repository: InMemoryRewriteDraftRepository,
    importer: TextDocumentImporter,
    event_publisher: RecordingEventPublisher,
    playback_engine: FakePlaybackEngine,
    tts: FakeSpeechSynthesizer,
    command_recognizer: FakeCommandRecognizer,
    dictation_transcriber: FakeDictationTranscriber,
    rewrite_generator: FakeRewriteGenerator,
    topic_summarizer: FakeTopicSummarizer,
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
    playback_engine: FakePlaybackEngine,
    tts: FakeSpeechSynthesizer,
    command_recognizer: FakeCommandRecognizer,
    dictation_transcriber: FakeDictationTranscriber,
    rewrite_generator: FakeRewriteGenerator,
    topic_summarizer: FakeTopicSummarizer,
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
            playback_engine: FakePlaybackEngine::new(),
            tts: FakeSpeechSynthesizer::new(),
            command_recognizer: FakeCommandRecognizer::default(),
            dictation_transcriber: FakeDictationTranscriber::default(),
            rewrite_generator: FakeRewriteGenerator::new(),
            topic_summarizer: FakeTopicSummarizer::new(),
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

    pub fn ingest_path(
        &mut self,
        source_path: &Path,
    ) -> Result<DocumentIngestionOutcome, marginalia_core::ports::DocumentImportError> {
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

        let synthesis = self.tts.synthesize(SynthesisRequest {
            text: chunk.text.clone(),
            voice: Some("narrator".to_string()),
            language: "it".to_string(),
        });
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
        session.tts_provider = Some("fake-tts".to_string());
        session.command_stt_provider = Some("fake-command-stt".to_string());
        session.playback_provider = playback.provider_name.clone();
        session.command_listening_active = true;
        session.command_language = Some("it".to_string());
        session.audio_reference = playback.audio_reference.clone();
        session.playback_process_id = playback.process_id;
        session.runtime_status = Some("active".to_string());
        session.touch();
        self.session_repository.save_session(session.clone());

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
            &mut self.playback_engine,
        );
        service.app_snapshot()
    }

    pub fn session_snapshot(&mut self) -> Result<Option<SessionSnapshot>, RuntimeError> {
        let mut service = SessionQueryService::new(
            &mut self.session_repository,
            &mut self.document_repository,
            &mut self.note_repository,
            &mut self.draft_repository,
            &mut self.playback_engine,
        );
        service.session_snapshot().map_err(RuntimeError::from)
    }

    pub fn published_events(&self) -> Vec<DomainEvent> {
        self.event_publisher.published_events()
    }

    pub fn command_recognizer(&self) -> &FakeCommandRecognizer {
        &self.command_recognizer
    }

    pub fn dictation_transcriber(&self) -> &FakeDictationTranscriber {
        &self.dictation_transcriber
    }

    pub fn rewrite_generator(&self) -> &FakeRewriteGenerator {
        &self.rewrite_generator
    }

    pub fn topic_summarizer(&self) -> &FakeTopicSummarizer {
        &self.topic_summarizer
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
            playback_engine: FakePlaybackEngine::new(),
            tts: FakeSpeechSynthesizer::new(),
            command_recognizer: FakeCommandRecognizer::default(),
            dictation_transcriber: FakeDictationTranscriber::default(),
            rewrite_generator: FakeRewriteGenerator::new(),
            topic_summarizer: FakeTopicSummarizer::new(),
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
            playback_engine: FakePlaybackEngine::new(),
            tts: FakeSpeechSynthesizer::new(),
            command_recognizer: FakeCommandRecognizer::default(),
            dictation_transcriber: FakeDictationTranscriber::default(),
            rewrite_generator: FakeRewriteGenerator::new(),
            topic_summarizer: FakeTopicSummarizer::new(),
        })
    }

    pub fn config(&self) -> RuntimeConfig {
        self.config
    }

    pub fn database(&self) -> &SQLiteDatabase {
        &self.database
    }

    pub fn ingest_path(
        &mut self,
        source_path: &Path,
    ) -> Result<DocumentIngestionOutcome, marginalia_core::ports::DocumentImportError> {
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

        let synthesis = self.tts.synthesize(SynthesisRequest {
            text: chunk.text.clone(),
            voice: Some("narrator".to_string()),
            language: "it".to_string(),
        });
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
        session.tts_provider = Some("fake-tts".to_string());
        session.command_stt_provider = Some("fake-command-stt".to_string());
        session.playback_provider = playback.provider_name.clone();
        session.command_listening_active = true;
        session.command_language = Some("it".to_string());
        session.audio_reference = playback.audio_reference.clone();
        session.playback_process_id = playback.process_id;
        session.runtime_status = Some("active".to_string());
        session.touch();
        self.session_repository.save_session(session.clone());

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
            &mut self.playback_engine,
        );
        service.app_snapshot()
    }

    pub fn session_snapshot(&mut self) -> Result<Option<SessionSnapshot>, RuntimeError> {
        let mut service = SessionQueryService::new(
            &mut self.session_repository,
            &mut self.document_repository,
            &mut self.note_repository,
            &mut self.draft_repository,
            &mut self.playback_engine,
        );
        service.session_snapshot().map_err(RuntimeError::from)
    }

    pub fn published_events(&self) -> Vec<DomainEvent> {
        self.event_publisher.published_events()
    }

    pub fn command_recognizer(&self) -> &FakeCommandRecognizer {
        &self.command_recognizer
    }

    pub fn dictation_transcriber(&self) -> &FakeDictationTranscriber {
        &self.dictation_transcriber
    }

    pub fn rewrite_generator(&self) -> &FakeRewriteGenerator {
        &self.rewrite_generator
    }

    pub fn topic_summarizer(&self) -> &FakeTopicSummarizer {
        &self.topic_summarizer
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
}
