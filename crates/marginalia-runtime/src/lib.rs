pub mod builder;
mod events;
mod frontend;

use marginalia_core::application::{
    DocumentIngestionOutcome, DocumentIngestionService, IngestionError, SessionQueryError,
    SessionQueryService,
};
use marginalia_core::domain::{
    PlaybackState, ReaderState, ReadingPosition, ReadingSession, VoiceNote,
    DEFAULT_CHUNK_TARGET_CHARS,
};
use marginalia_core::events::{DomainEvent, EventName};
use std::path::PathBuf;
use marginalia_core::frontend::{
    AppSnapshot, DocumentChunkView, DocumentListItem, DocumentSectionView, DocumentView,
    SessionSnapshot,
};
use marginalia_core::ports::storage::{DocumentRepository, NoteRepository, SessionRepository};
use marginalia_core::ports::{
    CommandRecognizer, DictationTranscriber, PlaybackEngine, RewriteGenerator,
    SpeechInterruptMonitor, SpeechSynthesizer, SynthesisError, SynthesisRequest, SynthesisResult,
    TopicSummarizer,
};
use marginalia_core::ports::{DocumentImportError, DocumentImporter};
#[cfg(feature = "epub-import")]
use marginalia_import_epub::EpubDocumentImporter;
#[cfg(feature = "pdf-import")]
use marginalia_import_pdf::PdfDocumentImporter;
use marginalia_import_text::TextDocumentImporter;
use marginalia_provider_fake::{
    FakeCommandRecognizer, FakeDictationTranscriber, FakePlaybackEngine, FakeRewriteGenerator,
    FakeSpeechSynthesizer, FakeTopicSummarizer, RecordingEventPublisher,
};
use marginalia_storage_sqlite::{
    SQLiteDatabase, SQLiteDocumentRepository, SQLiteNoteRepository, SQLiteRewriteDraftRepository,
    SQLiteSessionRepository,
};
use std::collections::HashMap;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

pub use builder::{BuildOutput, RuntimeBuilder, RuntimeSidecar};
pub use events::{EventCallback, RuntimeEvent, RuntimeEventSink};
pub use frontend::{RuntimeFrontend, RuntimeFrontendResponse};
pub use marginalia_core::ports::SttEngineOutput;

/// Routes import requests to the right backend by file extension.
///
/// - `.pdf` → `PdfDocumentImporter` (requires PDFium binary — optional
///   feature `pdf-import`, defaults to on).
/// - `.epub` → `EpubDocumentImporter` (pure-Rust — optional feature
///   `epub-import`, defaults to on).
/// - anything else → `TextDocumentImporter` (plain text / markdown).
struct DispatchImporter {
    text: TextDocumentImporter,
    #[cfg(feature = "pdf-import")]
    pdf: Option<PdfDocumentImporter>,
    #[cfg(feature = "epub-import")]
    epub: EpubDocumentImporter,
}

impl DocumentImporter for DispatchImporter {
    fn import_path(&self, source_path: &Path) -> Result<marginalia_core::domain::ImportedDocument, DocumentImportError> {
        let ext = source_path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase());

        match ext.as_deref() {
            #[cfg(feature = "pdf-import")]
            Some("pdf") => match &self.pdf {
                Some(pdf) => pdf.import_path(source_path),
                None => Err(DocumentImportError::ReadFailed {
                    source_path: source_path.to_path_buf(),
                    message: "PDF support not available. Run: make bootstrap-pdf".to_string(),
                }),
            },
            #[cfg(feature = "epub-import")]
            Some("epub") => self.epub.import_path(source_path),
            _ => self.text.import_path(source_path),
        }
    }
}

/// Build the importer dispatcher.
/// `pdfium_lib_dir`: explicit path to the directory containing libpdfium.dylib/.so.
/// Pass `None` to fall back to `models/pdf/lib` (relative to CWD, dev-only).
fn build_dispatch_importer(pdfium_lib_dir: Option<&std::path::Path>) -> DispatchImporter {
    #[cfg(feature = "pdf-import")]
    let pdf = {
        let default_dir = std::path::Path::new("models/pdf/lib");
        let lib_dir = pdfium_lib_dir.unwrap_or(default_dir);
        match PdfDocumentImporter::try_new_at(lib_dir) {
            Ok(p) => {
                log::info!("PDF import: PDFium loaded — .pdf files supported");
                Some(p)
            }
            Err(e) => {
                log::warn!("PDF import unavailable: {e}");
                None
            }
        }
    };
    DispatchImporter {
        text: TextDocumentImporter,
        #[cfg(feature = "pdf-import")]
        pdf,
        #[cfg(feature = "epub-import")]
        epub: EpubDocumentImporter::new(),
    }
}

static SESSION_COUNTER: AtomicU64 = AtomicU64::new(1);
static NOTE_COUNTER: AtomicU64 = AtomicU64::new(1);
static EVENT_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Configuration for the Marginalia runtime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeConfig {
    /// Target size in characters for text chunks.
    pub chunk_target_chars: usize,
    /// Default language code for TTS and STT (e.g. "it").
    pub default_language: String,
    /// Default TTS voice identifier (e.g. "if_sara").
    pub default_voice: String,
    /// Directory for TTS WAV cache. When set, synthesize_cached uses
    /// deterministic filenames (SHA-256 of the cache key) so WAVs
    /// persist across process restarts.
    pub tts_cache_dir: Option<PathBuf>,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            chunk_target_chars: DEFAULT_CHUNK_TARGET_CHARS,
            default_language: "it".to_string(),
            default_voice: "narrator".to_string(),
            tts_cache_dir: None,
        }
    }
}

/// Errors that can occur during runtime operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeError {
    /// No active reading session exists.
    MissingActiveSession,
    /// The requested document was not found in storage.
    MissingDocument { document_id: String },
    /// The document exists but has no readable chunks.
    EmptyDocument { document_id: String },
    /// TTS synthesis failed.
    Synthesis(SynthesisError),
    /// A session query operation failed.
    Query(SessionQueryError),
}

impl Display for RuntimeError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingActiveSession => {
                write!(f, "No active session is available in the runtime.")
            }
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

/// The main runtime that orchestrates core services, storage, and providers.
pub struct SqliteRuntime {
    config: RuntimeConfig,
    database: SQLiteDatabase,
    document_repository: SQLiteDocumentRepository,
    session_repository: SQLiteSessionRepository,
    note_repository: SQLiteNoteRepository,
    draft_repository: SQLiteRewriteDraftRepository,
    importer: DispatchImporter,
    event_publisher: RecordingEventPublisher,
    playback_engine: Box<dyn PlaybackEngine + Send>,
    tts: Box<dyn SpeechSynthesizer + Send>,
    command_recognizer: Box<dyn CommandRecognizer + Send>,
    dictation_transcriber: Box<dyn DictationTranscriber + Send>,
    rewrite_generator: Box<dyn RewriteGenerator + Send>,
    topic_summarizer: Box<dyn TopicSummarizer + Send>,
    provider_doctor_blobs: HashMap<String, serde_json::Value>,
    /// Cache: (document_id, section, chunk, voice) → SynthesisResult
    tts_cache: HashMap<String, SynthesisResult>,
    /// Push-based event system for app notifications.
    event_sink: RuntimeEventSink,
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
        .or_else(|| {
            active_session
                .as_ref()
                .map(|session| session.document_id.clone())
        })
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
    /// Open a runtime backed by an in-memory SQLite database (for testing).
    pub fn open_in_memory() -> rusqlite::Result<Self> {
        Self::open_in_memory_with_config(RuntimeConfig::default())
    }

    /// Open an in-memory runtime with a custom configuration.
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
            importer: build_dispatch_importer(None),
            event_publisher: RecordingEventPublisher::new(),
            playback_engine: Box::new(FakePlaybackEngine::new()),
            tts: Box::new(FakeSpeechSynthesizer::new()),
            command_recognizer: Box::new(FakeCommandRecognizer::default()),
            dictation_transcriber: Box::new(FakeDictationTranscriber::default()),
            rewrite_generator: Box::new(FakeRewriteGenerator::new()),
            topic_summarizer: Box::new(FakeTopicSummarizer::new()),
            provider_doctor_blobs: HashMap::new(),
            tts_cache: HashMap::new(),
            event_sink: RuntimeEventSink::new(),
        })
    }

    /// Open a runtime backed by an on-disk SQLite database at the given path.
    pub fn open(path: impl AsRef<Path>) -> rusqlite::Result<Self> {
        Self::open_with_config(path, RuntimeConfig::default())
    }

    /// Open an on-disk runtime with a custom configuration.
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
            importer: build_dispatch_importer(None),
            event_publisher: RecordingEventPublisher::new(),
            playback_engine: Box::new(FakePlaybackEngine::new()),
            tts: Box::new(FakeSpeechSynthesizer::new()),
            command_recognizer: Box::new(FakeCommandRecognizer::default()),
            dictation_transcriber: Box::new(FakeDictationTranscriber::default()),
            rewrite_generator: Box::new(FakeRewriteGenerator::new()),
            topic_summarizer: Box::new(FakeTopicSummarizer::new()),
            provider_doctor_blobs: HashMap::new(),
            tts_cache: HashMap::new(),
            event_sink: RuntimeEventSink::new(),
        })
    }

    /// Override the default TTS voice for this runtime.
    pub fn set_default_voice(&mut self, voice: &str) {
        self.config.default_voice = voice.to_string();
    }

    /// Subscribe to runtime events via an mpsc channel.
    pub fn subscribe_events(&mut self) -> std::sync::mpsc::Receiver<RuntimeEvent> {
        self.event_sink.subscribe_channel()
    }

    /// Register a callback to be invoked on each runtime event.
    pub fn on_event(&mut self, callback: EventCallback) {
        self.event_sink.subscribe_callback(callback);
    }

    /// Return a reference to the current runtime configuration.
    pub fn config(&self) -> &RuntimeConfig {
        &self.config
    }

    /// Replace the playback engine provider.
    pub fn set_playback_engine(&mut self, playback_engine: impl PlaybackEngine + Send + 'static) {
        self.playback_engine = Box::new(playback_engine);
    }

    /// Replace the TTS speech synthesizer provider.
    pub fn set_speech_synthesizer(&mut self, synthesizer: impl SpeechSynthesizer + Send + 'static) {
        self.tts = Box::new(synthesizer);
    }

    /// Store a provider diagnostic blob for the doctor report.
    pub fn set_provider_doctor_blob(&mut self, key: impl Into<String>, blob: serde_json::Value) {
        self.provider_doctor_blobs.insert(key.into(), blob);
    }

    /// Synthesize with cache. Uses a deterministic filename based on the
    /// SHA-256 of the cache key so WAVs survive process restarts. Falls back
    /// to in-memory HashMap if `tts_cache_dir` is not set.
    fn synthesize_cached(
        &mut self,
        document_id: &str,
        section_index: usize,
        chunk_index: usize,
        request: SynthesisRequest,
    ) -> Result<SynthesisResult, SynthesisError> {
        let voice = request.voice.clone().unwrap_or_default();
        let cache_key = format!("{document_id}:{section_index}:{chunk_index}:{voice}");

        // 1. Check in-memory cache (hot path for same session).
        if let Some(cached) = self.tts_cache.get(&cache_key) {
            if std::path::Path::new(&cached.audio_reference).exists() {
                return Ok(cached.clone());
            }
        }

        // 2. Check on-disk cache by deterministic filename (cross-session).
        if let Some(ref cache_dir) = self.config.tts_cache_dir {
            use sha2::{Digest, Sha256};
            let hash = format!("{:x}", Sha256::digest(cache_key.as_bytes()));
            let cached_path = cache_dir.join(format!("{hash}.flac"));
            if cached_path.exists() {
                let result = SynthesisResult {
                    provider_name: self.tts.describe_capabilities().provider_name,
                    voice: voice.clone(),
                    content_type: "audio/flac".to_string(),
                    audio_reference: cached_path.display().to_string(),
                    byte_length: cached_path
                        .metadata()
                        .map(|m| m.len() as usize)
                        .unwrap_or(0),
                    text_excerpt: request.text.chars().take(50).collect(),
                    metadata: HashMap::new(),
                };
                self.tts_cache.insert(cache_key, result.clone());
                return Ok(result);
            }
        }

        // 3. Synthesize and cache the result.
        let mut result = self.tts.synthesize(request)?;

        // 4. Rename to deterministic path so it persists across restarts.
        if let Some(ref cache_dir) = self.config.tts_cache_dir {
            use sha2::{Digest, Sha256};
            let hash = format!("{:x}", Sha256::digest(cache_key.as_bytes()));
            let stable_path = cache_dir.join(format!("{hash}.flac"));
            if let Err(e) = std::fs::rename(&result.audio_reference, &stable_path) {
                // rename may fail cross-device; try copy + delete
                if std::fs::copy(&result.audio_reference, &stable_path).is_ok() {
                    let _ = std::fs::remove_file(&result.audio_reference);
                } else {
                    log::warn!("tts cache rename failed: {e}");
                }
            }
            if stable_path.exists() {
                result.audio_reference = stable_path.display().to_string();
            }
        }

        self.tts_cache.insert(cache_key, result.clone());
        Ok(result)
    }

    /// Return a reference to the underlying SQLite database.
    pub fn database(&self) -> &SQLiteDatabase {
        &self.database
    }

    /// Import a document from a file path into the runtime's storage.
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

    /// Start a new reading session for the given document, synthesizing and playing the first chunk.
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
        let synthesis = self.synthesize_cached(
            document_id,
            position.section_index,
            position.chunk_index,
            SynthesisRequest {
                text: chunk.text.clone(),
                voice: Some(self.config.default_voice.clone()),
                language: self.config.default_language.clone(),
            },
        )?;
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
        session.voice = Some(self.config.default_voice.clone());
        session.tts_provider = Some(tts_provider);
        session.command_stt_provider = Some(
            self.command_recognizer
                .describe_capabilities()
                .provider_name,
        );
        session.playback_provider = playback.provider_name.clone();
        session.command_listening_active = true;
        session.command_language = Some(self.config.default_language.clone());
        session.audio_reference = playback.audio_reference.clone();
        session.playback_process_id = playback.process_id;
        session.runtime_status = Some("active".to_string());
        session.touch();
        if let Err(e) = self.session_repository.save_session(session.clone()) {
            log::warn!("failed to save session: {e}");
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

    /// Restore the last active session from the database, if any. This is
    /// called once at startup so the user picks up where they left off.
    /// The session is restored in **Paused** state with command listening
    /// active — the user can then `/resume` or say "riprendi" to start
    /// playback. Returns `None` if no active session was found or the
    /// document no longer exists.
    pub fn restore_session(&mut self) -> Option<ReadingSession> {
        let mut session = self.session_repository.get_active_session()?;

        // Guard: does the document still exist?
        if self
            .document_repository
            .get_document(&session.document_id)
            .is_none()
        {
            session.is_active = false;
            session.touch();
            let _ = self.session_repository.save_session(session);
            return None;
        }

        // Set to Paused so the TUI shows the document without auto-playing.
        session.state = ReaderState::Paused;
        session.playback_state = PlaybackState::Stopped;
        session.command_listening_active = true;
        session.last_command = Some("restore_session".to_string());
        session.runtime_status = Some("active".to_string());
        session.voice = session
            .voice
            .or_else(|| Some(self.config.default_voice.clone()));
        session.touch();
        if let Err(e) = self.session_repository.save_session(session.clone()) {
            log::warn!("failed to save restored session: {e}");
        }
        self.event_sink.emit(RuntimeEvent::SessionRestored {
            session_id: session.session_id.clone(),
            document_id: session.document_id.clone(),
            section_index: session.position.section_index,
            chunk_index: session.position.chunk_index,
        });
        Some(session)
    }

    /// Check if the current chunk finished playing naturally. If so, advance
    /// to the next chunk and start playback. If at the end of the document,
    /// stop the session. Returns `true` if it advanced.
    pub fn try_auto_advance(&mut self) -> bool {
        let snap = self.playback_engine.snapshot();
        if snap.state != PlaybackState::Stopped || snap.last_action != "completed" {
            return false;
        }
        if let Some(session) = self.session_repository.get_active_session() {
            self.event_sink.emit(RuntimeEvent::PlaybackFinished {
                document_id: session.document_id.clone(),
                section_index: session.position.section_index,
                chunk_index: session.position.chunk_index,
            });
        }
        if self.next_chunk().is_ok() {
            if let Some(session) = self.session_repository.get_active_session() {
                self.event_sink.emit(RuntimeEvent::ChunkAdvanced {
                    document_id: session.document_id.clone(),
                    section_index: session.position.section_index,
                    chunk_index: session.position.chunk_index,
                });
            }
            true
        } else {
            let _ = self.stop_session();
            false
        }
    }

    /// Build a full application snapshot for the frontend.
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

    /// Build a snapshot of the active session, if one exists.
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

    /// Return all domain events published during this runtime's lifetime.
    pub fn published_events(&self) -> Vec<DomainEvent> {
        self.event_publisher.published_events()
    }

    /// List all imported documents as summary items.
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

    /// Build a detailed document view for the frontend, optionally for a specific document.
    pub fn document_view(&self, document_id: Option<&str>) -> Option<DocumentView> {
        build_document_view(
            &self.document_repository,
            &self.session_repository,
            document_id,
        )
    }

    /// Pause the active reading session.
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
            log::warn!("failed to save session: {e}");
        }
        Ok(())
    }

    /// Resume a paused reading session.
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
            log::warn!("failed to save session: {e}");
        }
        Ok(())
    }

    /// Stop the active reading session and mark it inactive.
    pub fn stop_session(&mut self) -> Result<(), RuntimeError> {
        let mut session = self
            .session_repository
            .get_active_session()
            .ok_or(RuntimeError::MissingActiveSession)?;
        let document_id = session.document_id.clone();
        let playback = self.playback_engine.stop();
        session.state = ReaderState::Idle;
        session.playback_state = playback.state;
        session.last_command = Some("stop_session".to_string());
        session.runtime_status = Some("stopped".to_string());
        session.command_listening_active = false;
        session.is_active = false;
        session.touch();
        if let Err(e) = self.session_repository.save_session(session) {
            log::warn!("failed to save session: {e}");
        }
        self.event_sink.emit(RuntimeEvent::SessionStopped { document_id });
        Ok(())
    }

    /// Advance to the next chunk in the document.
    pub fn next_chunk(&mut self) -> Result<(), RuntimeError> {
        self.seek_relative_chunk(1)
    }

    /// Go back to the previous chunk in the document.
    pub fn previous_chunk(&mut self) -> Result<(), RuntimeError> {
        self.seek_relative_chunk(-1)
    }

    /// Advance to the first chunk of the next chapter (section).
    pub fn next_chapter(&mut self) -> Result<(), RuntimeError> {
        self.seek_chapter(1, false)
    }

    /// Go back to the first chunk of the previous chapter (section).
    pub fn previous_chapter(&mut self) -> Result<(), RuntimeError> {
        self.seek_chapter(-1, false)
    }

    /// Restart the current chapter from its first chunk.
    pub fn restart_chapter(&mut self) -> Result<(), RuntimeError> {
        self.seek_chapter(0, true)
    }

    /// Re-synthesize and replay the current chunk.
    pub fn repeat_chunk(&mut self) -> Result<(), RuntimeError> {
        self.replay_current_position("repeat_chunk")
    }

    /// Create a voice note attached to the current reading position.
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
            log::warn!("failed to save note: {e}");
        }
        session.last_command = Some("create_note".to_string());
        session.touch();
        if let Err(e) = self.session_repository.save_session(session.clone()) {
            log::warn!("failed to save session: {e}");
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

    /// Generate a diagnostic report of all configured providers and their status.
    pub fn doctor_report(&self) -> serde_json::Value {
        let playback_name = self.playback_engine.describe_capabilities().provider_name;
        let tts_name = self.tts.describe_capabilities().provider_name;
        let stt_name = self
            .command_recognizer
            .describe_capabilities()
            .provider_name;
        let dictation_name = self
            .dictation_transcriber
            .describe_capabilities()
            .provider_name;

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

    /// Replace the command recognizer provider.
    pub fn set_command_recognizer(&mut self, recognizer: impl CommandRecognizer + Send + 'static) {
        self.command_recognizer = Box::new(recognizer);
    }

    /// Open a persistent speech interrupt monitor for voice command detection.
    pub fn open_command_monitor(&mut self) -> Box<dyn SpeechInterruptMonitor> {
        self.command_recognizer.open_interrupt_monitor()
    }

    /// Replace the dictation transcriber provider.
    pub fn set_dictation_transcriber(
        &mut self,
        transcriber: impl DictationTranscriber + Send + 'static,
    ) {
        self.dictation_transcriber = Box::new(transcriber);
    }

    /// Convenience: set both command recognizer and dictation transcriber from
    /// an `SttEngineOutput`. Use this when the engine factory returns a matched
    /// pair (e.g. `new_apple_stt`, or a future unified Whisper factory).
    pub fn set_stt_engine(&mut self, output: SttEngineOutput) {
        self.command_recognizer = output.command_recognizer;
        self.dictation_transcriber = output.dictation_transcriber;
    }

    /// Return a reference to the active dictation transcriber.
    pub fn dictation_transcriber(&self) -> &dyn DictationTranscriber {
        self.dictation_transcriber.as_ref()
    }

    /// Return a reference to the active rewrite generator.
    pub fn rewrite_generator(&self) -> &dyn RewriteGenerator {
        self.rewrite_generator.as_ref()
    }

    /// Return a reference to the active topic summarizer.
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

        let doc_id = session.document_id.clone();
        let synthesis = self.synthesize_cached(
            &doc_id,
            session.position.section_index,
            session.position.chunk_index,
            SynthesisRequest {
                text: chunk.text.clone(),
                voice: session
                    .voice
                    .clone()
                    .or(Some(self.config.default_voice.clone())),
                language: session
                    .command_language
                    .clone()
                    .unwrap_or_else(|| self.config.default_language.clone()),
            },
        )?;
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
            log::warn!("failed to save session: {e}");
        }
        Ok(())
    }

    /// Pre-synthesize the next chunk into the TTS cache.
    /// Called from a background thread after the current chunk starts playing.
    pub fn prefetch_next(&mut self) {
        let session = match self.session_repository.get_active_session() {
            Some(s) => s,
            None => return,
        };
        let document = match self.document_repository.get_document(&session.document_id) {
            Some(d) => d,
            None => return,
        };

        // Build flat position list and find current
        let positions: Vec<(usize, usize)> = document
            .sections
            .iter()
            .flat_map(|s| s.chunks.iter().map(move |c| (s.index, c.index)))
            .collect();

        let current = positions.iter().position(|(s, c)| {
            *s == session.position.section_index && *c == session.position.chunk_index
        });
        let next_idx = match current {
            Some(i) if i + 1 < positions.len() => i + 1,
            _ => return,
        };
        let (next_section, next_chunk) = positions[next_idx];

        let voice = session.voice.or(Some(self.config.default_voice.clone()));
        let language = session
            .command_language
            .unwrap_or_else(|| self.config.default_language.clone());

        // Check if already cached
        let cache_key = format!(
            "{}:{}:{}:{}",
            document.document_id,
            next_section,
            next_chunk,
            voice.as_deref().unwrap_or("")
        );
        if let Some(cached) = self.tts_cache.get(&cache_key) {
            if std::path::Path::new(&cached.audio_reference).exists() {
                return;
            }
        }

        if let Some(chunk) = document.get_chunk(next_section, next_chunk) {
            let _ = self.synthesize_cached(
                &document.document_id,
                next_section,
                next_chunk,
                SynthesisRequest {
                    text: chunk.text.clone(),
                    voice,
                    language,
                },
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::SqliteRuntime;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_path(extension: &str) -> PathBuf {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "marginalia-runtime-test-{}.{}",
            timestamp, extension
        ))
    }

    #[test]
    fn sqlite_runtime_can_ingest_and_report_idle_snapshot() {
        let path = temp_path("md");
        fs::write(&path, "# Intro\n\nAlpha beta gamma.").unwrap();

        let mut runtime = SqliteRuntime::open_in_memory().unwrap();
        let outcome = runtime.ingest_path(&path).unwrap();
        let snapshot = runtime.app_snapshot();

        assert!(outcome
            .document
            .title
            .starts_with("Marginalia Runtime Test"));
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
        let session = runtime
            .start_session(&outcome.document.document_id)
            .unwrap();
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
            ..super::RuntimeConfig::default()
        })
        .unwrap();
        let outcome = runtime.ingest_path(&path).unwrap();
        runtime
            .start_session(&outcome.document.document_id)
            .unwrap();

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
        runtime
            .start_session(&outcome.document.document_id)
            .unwrap();
        let note = runtime.create_note("remember this").unwrap();
        let snapshot = runtime.session_snapshot().unwrap().unwrap();

        assert_eq!(note.document_id, outcome.document.document_id);
        assert_eq!(note.transcript, "remember this");
        assert_eq!(snapshot.notes_count, 1);

        let _ = fs::remove_file(path);
    }

    #[test]
    fn sqlite_runtime_pause_resume_stop() {
        let path = temp_path("md");
        fs::write(&path, "# Intro\n\nAlpha beta gamma.").unwrap();

        let mut runtime = SqliteRuntime::open_in_memory().unwrap();
        let outcome = runtime.ingest_path(&path).unwrap();
        runtime
            .start_session(&outcome.document.document_id)
            .unwrap();

        runtime.pause_session().unwrap();
        let paused = runtime.session_snapshot().unwrap().unwrap();
        assert_eq!(paused.state, "paused");

        runtime.resume_session().unwrap();
        let resumed = runtime.session_snapshot().unwrap().unwrap();
        assert_eq!(resumed.state, "reading");

        runtime.stop_session().unwrap();
        let stopped = runtime.session_snapshot();
        assert!(
            stopped.unwrap().is_none(),
            "no active session after stop"
        );

        let _ = fs::remove_file(path);
    }

    #[test]
    fn sqlite_runtime_restore_session() {
        let path = temp_path("md");
        fs::write(
            &path,
            "Alpha beta gamma delta epsilon zeta eta theta.",
        )
        .unwrap();

        let mut runtime = SqliteRuntime::open_in_memory_with_config(super::RuntimeConfig {
            chunk_target_chars: 20,
            ..super::RuntimeConfig::default()
        })
        .unwrap();
        let outcome = runtime.ingest_path(&path).unwrap();
        runtime
            .start_session(&outcome.document.document_id)
            .unwrap();
        runtime.next_chunk().unwrap();
        let pos_before = runtime.session_snapshot().unwrap().unwrap();

        // Simulate app restart: restore_session picks up where we left off.
        let restored = runtime.restore_session();
        assert!(restored.is_some(), "should restore the active session");
        let session = restored.unwrap();
        assert_eq!(session.document_id, outcome.document.document_id);
        assert_eq!(
            session.position.chunk_index,
            pos_before.chunk_index as usize
        );

        let _ = fs::remove_file(path);
    }

    #[test]
    fn sqlite_runtime_full_end_to_end_flow() {
        let path = temp_path("txt");
        fs::write(
            &path,
            "Alpha beta gamma delta epsilon zeta eta theta iota kappa.\n\n\
             Lambda mu nu xi omicron pi rho sigma tau upsilon.",
        )
        .unwrap();

        let mut runtime = SqliteRuntime::open_in_memory_with_config(super::RuntimeConfig {
            chunk_target_chars: 20,
            ..super::RuntimeConfig::default()
        })
        .unwrap();

        // 1. Ingest
        let outcome = runtime.ingest_path(&path).unwrap();
        let doc_id = &outcome.document.document_id;
        assert!(runtime.list_documents().len() == 1);

        // 2. Start session
        let session = runtime.start_session(doc_id).unwrap();
        assert_eq!(session.position.chunk_index, 0);

        // 3. Navigate: next, back
        runtime.next_chunk().unwrap();
        let after_next = runtime.session_snapshot().unwrap().unwrap();
        assert!(after_next.chunk_index >= 1, "next_chunk advanced");
        runtime.previous_chunk().unwrap();

        // 4. Create a note
        let note = runtime.create_note("test note").unwrap();
        assert_eq!(note.transcript, "test note");

        // 5. Pause + resume
        runtime.pause_session().unwrap();
        assert_eq!(
            runtime.session_snapshot().unwrap().unwrap().state,
            "paused"
        );
        runtime.resume_session().unwrap();
        assert_eq!(
            runtime.session_snapshot().unwrap().unwrap().state,
            "reading"
        );

        // 6. Stop
        runtime.stop_session().unwrap();
        assert!(runtime.session_snapshot().unwrap().is_none());

        // 7. Restore
        // After stop, is_active = false, so restore returns None.
        assert!(runtime.restore_session().is_none());

        let _ = fs::remove_file(path);
    }
}
