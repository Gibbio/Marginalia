pub mod builder;
pub mod discovery;
mod events;
mod frontend;
pub mod reconfigure;

use marginalia_core::application::{
    DocumentIngestionOutcome, DocumentIngestionService, IngestionError, SessionQueryError,
    SessionQueryService,
};
use marginalia_core::domain::{
    PlaybackState, ReaderState, ReadingPosition, ReadingSession, VoiceNote,
    DEFAULT_CHUNK_TARGET_CHARS,
};
use marginalia_core::events::{DomainEvent, EventName};
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
#[cfg(feature = "url-import")]
use marginalia_import_url::UrlDocumentImporter;
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
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

pub use builder::{BuildOutput, RuntimeBuilder, RuntimeSidecar};
pub use discovery::{Discovery, Gender, LangInfo, SttEngine, TtsBackend, VoiceInfo};
pub use events::{EventCallback, RuntimeEvent, RuntimeEventSink};
pub use frontend::{RuntimeFrontend, RuntimeFrontendResponse};
pub use marginalia_core::ports::SttEngineOutput;
pub use reconfigure::{apply_provider_spec, ApplyReport, ProviderSpec, ReconfigureContext};

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
    fn import_path(
        &self,
        source_path: &Path,
    ) -> Result<marginalia_core::domain::ImportedDocument, DocumentImportError> {
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
static EVENT_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Map `whatlang::Lang` to a BCP-47 2-letter prefix for matching against
/// the runtime's configured voice language. Only the languages we ship a
/// Kokoro voice for are mapped — unknown/unsupported detections return
/// `None` and the `VoiceMismatch` event is suppressed (better to stay
/// silent than to nag the user about a language we can't fix).
fn bcp47_prefix_for(lang: whatlang::Lang) -> Option<&'static str> {
    use whatlang::Lang;
    match lang {
        Lang::Eng => Some("en"),
        Lang::Ita => Some("it"),
        Lang::Fra => Some("fr"),
        Lang::Deu => Some("de"),
        Lang::Spa => Some("es"),
        Lang::Por => Some("pt"),
        Lang::Jpn => Some("ja"),
        Lang::Cmn => Some("zh"),
        Lang::Hin => Some("hi"),
        _ => None,
    }
}

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
    /// Catch-all for runtime-level errors (storage, logic) that don't
    /// fit into the specific variants. Message is surfaced verbatim via
    /// the FFI.
    Runtime(String),
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
            Self::Runtime(msg) => write!(f, "{msg}"),
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
    /// Wrapped in `Arc<Mutex<>>` so the FFI can clone a handle and run the
    /// blocking `transcribe()` call on a dedicated thread without holding
    /// the runtime-wide lock. Dictation takes ~10–30 s (user speech +
    /// silence timeout); blocking the runtime that whole time would
    /// freeze the UI's 100 ms event poll.
    dictation_transcriber: Arc<Mutex<Box<dyn DictationTranscriber + Send>>>,
    /// Side channel for the live partial transcript exposed by Apple
    /// STT. Lives outside the transcriber mutex so polling it doesn't
    /// block while `transcribe()` (which holds that mutex for the
    /// entire dictation) is in flight. Empty when no provider is
    /// publishing partials.
    dictation_partial: Arc<Mutex<String>>,
    rewrite_generator: Box<dyn RewriteGenerator + Send>,
    topic_summarizer: Box<dyn TopicSummarizer + Send>,
    provider_doctor_blobs: HashMap<String, serde_json::Value>,
    /// Cache: (document_id, section, chunk, voice) → SynthesisResult
    tts_cache: HashMap<String, SynthesisResult>,
    /// Push-based event system for app notifications.
    event_sink: RuntimeEventSink,
}

/// Returns `true` iff the file at `source_path` exists and its current
/// SHA-256 differs from `stored_sha`. Used by `list_documents` to flag
/// rows whose source file was edited externally since last ingestion.
/// On any IO error (file missing, permission denied) → `false`: the
/// "Apri file in editor…" action will surface the OS error if the user
/// really tries to open the file. The `_size` and `_mtime` parameters
/// are reserved for a future fast-path (skip the SHA recompute when
/// stat-only metadata matches); the v1 always recomputes.
fn compute_needs_reload(
    source_path: &Path,
    stored_sha: Option<&str>,
    _size: Option<u64>,
    _mtime: Option<i64>,
) -> bool {
    let stored = match stored_sha {
        Some(s) if !s.is_empty() => s,
        // Pre-migration row (sha unknown). Treat as "not flagged" — we
        // don't want to nag every legacy row at first launch; the user
        // can still trigger a manual reload from the context menu, and
        // the next ingest backfills the sha.
        _ => return false,
    };
    match marginalia_core::application::file_fingerprint(source_path) {
        Ok(fp) => fp.sha256_hex != stored,
        Err(_) => false,
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
            dictation_transcriber: Arc::new(Mutex::new(Box::new(
                FakeDictationTranscriber::default(),
            ))),
            dictation_partial: Arc::new(Mutex::new(String::new())),
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
            dictation_transcriber: Arc::new(Mutex::new(Box::new(
                FakeDictationTranscriber::default(),
            ))),
            dictation_partial: Arc::new(Mutex::new(String::new())),
            rewrite_generator: Box::new(FakeRewriteGenerator::new()),
            topic_summarizer: Box::new(FakeTopicSummarizer::new()),
            provider_doctor_blobs: HashMap::new(),
            tts_cache: HashMap::new(),
            event_sink: RuntimeEventSink::new(),
        })
    }

    /// Latest partial dictation transcript exposed by the STT provider,
    /// empty when no dictation is in flight (or the provider doesn't
    /// stream partials). Hosts poll this on their event tick to render
    /// the running transcript live, without waiting for `transcribe()`
    /// to return its silence-finalized result. Reads from a dedicated
    /// `Arc<Mutex<String>>` (not the transcriber mutex) so polling
    /// doesn't block while `transcribe()` is holding its own lock.
    pub fn dictation_partial(&self) -> String {
        self.dictation_partial
            .lock()
            .map(|s| s.clone())
            .unwrap_or_default()
    }

    /// Hot-swap the side-channel used by `dictation_partial()`. Called
    /// from the Apple STT factory so its reader-thread shared slot
    /// becomes the source the host polls. Engines that don't stream
    /// partials leave this untouched and the slot stays empty.
    pub fn set_dictation_partial_slot(&mut self, slot: Arc<Mutex<String>>) {
        self.dictation_partial = slot;
    }

    /// Override the default TTS voice for this runtime. Also refreshes
    /// the active session (if any) so its cached `voice` tag matches —
    /// without this, `replay_session_at_position` would still build
    /// `SynthesisRequest { voice: Some(session.voice) }` with the old
    /// voice id, the TTS cache would key on the stale voice and serve
    /// the previously-synthesized WAV (rendered by the old voice),
    /// ignoring the fact that the synthesizer has been swapped. The user
    /// would "change voice" in Settings but still hear the old voice on
    /// any chunk they had already listened to. By sync'ing
    /// `session.voice`, the next resume rebuilds the cache key under the
    /// new voice → cache miss → fresh synthesis with the new voice.
    pub fn set_default_voice(&mut self, voice: &str) {
        let voice_changed = self.config.default_voice != voice;
        self.config.default_voice = voice.to_string();
        if let Some(mut session) = self.session_repository.get_active_session() {
            if session.voice.as_deref() != Some(voice) {
                session.voice = Some(voice.to_string());
                session.touch();
                if let Err(e) = self.session_repository.save_session(session) {
                    log::warn!("failed to refresh active session voice: {e}");
                }
            }
        }
        // Drop whatever audio is currently queued in the playback engine.
        // The cached WAV was synthesized for the OLD voice; without this,
        // a paused engine would replay the previous voice when the user
        // hits play after switching. `resume_session` detects the
        // resulting Stopped state and routes through
        // `replay_session_at_position`, which re-synthesizes with the
        // new voice (cache miss on the new content-addressed key).
        if voice_changed {
            log::info!("[runtime] default voice changed to {voice}, clearing playback engine");
            let _ = self.playback_engine.stop();
        }
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

    /// Replace the TTS speech synthesizer provider with a pre-boxed trait
    /// object. Used by `reconfigure::apply_provider_spec`, which picks the
    /// concrete backend at runtime and returns `Box<dyn SpeechSynthesizer>`.
    pub fn set_speech_synthesizer_boxed(&mut self, synthesizer: Box<dyn SpeechSynthesizer + Send>) {
        self.tts = synthesizer;
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
        // Content-addressed cache key: `sha256(text + voice + lang)`.
        // Was position-keyed (`{doc}:{sec}:{chunk}:{voice}`), but that
        // meant the same key could point to DIFFERENT audio across a
        // doc reload — same chunk index, new text from disk → cached
        // WAV played the OLD content. Hashing the actual payload
        // (text + voice + language) makes the key match the audio's
        // true identity: identical chunks share cache entries (across
        // documents and across reloads), edited chunks miss the cache
        // and re-synthesize, deleted documents leave their cache files
        // as harmless orphans (only a few KB each).
        let voice = request.voice.clone().unwrap_or_default();
        let cache_key = {
            use sha2::{Digest, Sha256};
            let mut h = Sha256::new();
            h.update(request.text.as_bytes());
            h.update(b":");
            h.update(voice.as_bytes());
            h.update(b":");
            h.update(request.language.as_bytes());
            format!("{:x}", h.finalize())
        };
        log::info!(
            "[runtime] synthesize_cached key={} doc={} sec={} chunk={} voice={} lang={} text_len={}",
            &cache_key[..12],
            document_id,
            section_index,
            chunk_index,
            voice,
            request.language,
            request.text.chars().count()
        );

        // 1. Check in-memory cache (hot path for same session).
        if let Some(cached) = self.tts_cache.get(&cache_key) {
            if std::path::Path::new(&cached.audio_reference).exists() {
                let result = cached.clone();
                log::info!(
                    "[runtime] synthesize_cached: in-memory hit audio_ref={}",
                    result.audio_reference
                );
                self.event_sink.emit(RuntimeEvent::SynthesisReady {
                    document_id: document_id.to_string(),
                    section_index,
                    chunk_index,
                    cache_hit: true,
                    elapsed_ms: 0,
                });
                return Ok(result);
            }
        }

        // 2. Check on-disk cache by deterministic filename (cross-session).
        // Filename IS the cache key (already a sha256 hex digest), so
        // no extra hashing step. `.wav` for MLX now; `.flac` kept as a
        // legacy fallback for any older entries still on disk.
        if let Some(ref cache_dir) = self.config.tts_cache_dir {
            let wav_path = cache_dir.join(format!("{cache_key}.wav"));
            let flac_path = cache_dir.join(format!("{cache_key}.flac"));
            let cached_path = if wav_path.exists() {
                wav_path
            } else if flac_path.exists() {
                flac_path
            } else {
                wav_path
            }; // sentinel; .exists() below is false
            if cached_path.exists() {
                let ext = cached_path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("wav");
                log::info!(
                    "[runtime] synthesize_cached: on-disk hit path={}",
                    cached_path.display()
                );
                let result = SynthesisResult {
                    provider_name: self.tts.describe_capabilities().provider_name,
                    voice: voice.clone(),
                    content_type: format!("audio/{ext}"),
                    audio_reference: cached_path.display().to_string(),
                    byte_length: cached_path
                        .metadata()
                        .map(|m| m.len() as usize)
                        .unwrap_or(0),
                    text_excerpt: request.text.chars().take(50).collect(),
                    metadata: HashMap::new(),
                };
                self.tts_cache.insert(cache_key, result.clone());
                self.event_sink.emit(RuntimeEvent::SynthesisReady {
                    document_id: document_id.to_string(),
                    section_index,
                    chunk_index,
                    cache_hit: true,
                    elapsed_ms: 0,
                });
                return Ok(result);
            }
        }

        // 3. Synthesize and cache the result. Emit SynthesisStarted right
        // before the (potentially slow) TTS call so the UI can light up a
        // "sintetizzando…" indicator during the gap — the cache-hit paths
        // above are instantaneous and would cause a spurious spinner flash.
        self.event_sink.emit(RuntimeEvent::SynthesisStarted {
            document_id: document_id.to_string(),
            section_index,
            chunk_index,
        });
        let provider = self.tts.describe_capabilities().provider_name;
        let voice_for_log = request.voice.clone().unwrap_or_default();
        let lang_for_log = request.language.clone();
        let text_len = request.text.chars().count();
        log::info!(
            "[runtime] tts.synthesize provider={provider} voice={voice_for_log} lang={lang_for_log} text_len={text_len}"
        );
        let synth_start = std::time::Instant::now();
        let mut result = match self.tts.synthesize(request) {
            Ok(r) => {
                log::info!(
                    "[runtime] tts.synthesize ok provider={provider} elapsed_ms={} audio_ref={}",
                    synth_start.elapsed().as_millis(),
                    r.audio_reference,
                );
                r
            }
            Err(e) => {
                log::error!(
                    "[runtime] tts.synthesize FAILED provider={provider} voice={voice_for_log} lang={lang_for_log} elapsed_ms={} err={e}",
                    synth_start.elapsed().as_millis(),
                );
                return Err(e);
            }
        };
        let synth_elapsed_ms = synth_start.elapsed().as_millis() as u64;

        // 4. Rename to the content-addressed stable path so the next
        // session (or a different document with an identical chunk)
        // hits the on-disk cache. `cache_key` IS the sha256 hex
        // already, so it doubles as the filename stem.
        if let Some(ref cache_dir) = self.config.tts_cache_dir {
            let ext = std::path::Path::new(&result.audio_reference)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("wav");
            let stable_path = cache_dir.join(format!("{cache_key}.{ext}"));
            if let Err(e) = std::fs::rename(&result.audio_reference, &stable_path) {
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

        self.tts_cache.insert(cache_key.clone(), result.clone());
        self.event_sink.emit(RuntimeEvent::SynthesisReady {
            document_id: document_id.to_string(),
            section_index,
            chunk_index,
            cache_hit: false,
            elapsed_ms: synth_elapsed_ms,
        });
        Ok(result)
    }

    /// Return a reference to the underlying SQLite database.
    pub fn database(&self) -> &SQLiteDatabase {
        &self.database
    }

    /// Import a document from a file path into the runtime's storage.
    /// Emits `IngestStarted` / `IngestFinished` so the UI can overlay a
    /// "sto leggendo …" spinner during chunking — large PDFs take several
    /// seconds and the user needs to know the app isn't frozen. The `source`
    /// field in the events carries the full path so the UI's "Riprova"
    /// action can re-run the exact same import.
    pub fn ingest_path(
        &mut self,
        source_path: &Path,
    ) -> Result<DocumentIngestionOutcome, IngestionError> {
        let source = source_path.display().to_string();
        self.event_sink.emit(RuntimeEvent::IngestStarted {
            source: source.clone(),
        });
        let result = {
            let mut service = DocumentIngestionService::new(
                &mut self.document_repository,
                &self.importer,
                self.event_publisher.clone(),
                self.config.chunk_target_chars,
            );
            service.ingest_path(source_path)
        };
        match &result {
            Ok(outcome) => self.event_sink.emit(RuntimeEvent::IngestFinished {
                source,
                document_id: Some(outcome.document.document_id.clone()),
                error: None,
            }),
            Err(e) => self.event_sink.emit(RuntimeEvent::IngestFinished {
                source,
                document_id: None,
                error: Some(format!("{e:?}")),
            }),
        }
        result
    }

    /// Fetch a URL, extract its readable article via Mozilla Readability, and
    /// persist it as a single-section document. Short URLs are resolved
    /// transparently via HTTP redirect following.
    #[cfg(feature = "url-import")]
    pub fn ingest_url(&mut self, url: &str) -> Result<DocumentIngestionOutcome, IngestionError> {
        self.event_sink.emit(RuntimeEvent::IngestStarted {
            source: url.to_string(),
        });
        // A fresh importer per call: ureq::Agent construction is microseconds
        // and URL ingestion is a low-frequency user-driven action. Avoids
        // holding a long-lived TCP pool inside the runtime struct.
        let importer = UrlDocumentImporter::new();
        let result: Result<DocumentIngestionOutcome, IngestionError> = (|| {
            let imported = importer.import_url(url)?;
            let mut service = DocumentIngestionService::new(
                &mut self.document_repository,
                &self.importer,
                self.event_publisher.clone(),
                self.config.chunk_target_chars,
            );
            service.ingest_imported(imported)
        })();
        match &result {
            Ok(outcome) => self.event_sink.emit(RuntimeEvent::IngestFinished {
                source: url.to_string(),
                document_id: Some(outcome.document.document_id.clone()),
                error: None,
            }),
            Err(e) => self.event_sink.emit(RuntimeEvent::IngestFinished {
                source: url.to_string(),
                document_id: None,
                error: Some(format!("{e:?}")),
            }),
        }
        result
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

        // B3 — language auto-detect. Peek the first chunk against whatlang
        // and emit a `VoiceMismatch` event when the detection is confident
        // AND disagrees with the currently-selected voice's language. UI
        // shows a toast with a "passa a voce X" action; ignoring it lets
        // playback proceed unchanged.
        if let Some(info) = whatlang::detect(&chunk.text) {
            if info.is_reliable() {
                if let Some(detected_prefix) = bcp47_prefix_for(info.lang()) {
                    let current_prefix = self
                        .config
                        .default_language
                        .split(['-', '_'])
                        .next()
                        .unwrap_or("")
                        .to_lowercase();
                    if !current_prefix.is_empty() && current_prefix != detected_prefix {
                        self.event_sink.emit(RuntimeEvent::VoiceMismatch {
                            document_id: document_id.to_string(),
                            detected_language: detected_prefix.to_string(),
                            current_language: self.config.default_language.clone(),
                        });
                    }
                }
            }
        }

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
    ///
    /// `needs_reload` is computed by re-fingerprinting each row's source
    /// file off the storage mutex (the path/hash snapshot is taken under
    /// the lock, then released before the IO loop). On a library of N
    /// docs this is N stat+sha calls — small enough for libraries up to
    /// the low thousands; if it ever becomes a hotspot we'd add a
    /// "mtime+size matches → trust the stored sha" fast path.
    pub fn list_documents(&self) -> Vec<DocumentListItem> {
        let docs = self.document_repository.list_documents();
        docs.into_iter()
            .map(|document| {
                let source_path = document.source_path.to_string_lossy().to_string();
                let needs_reload = compute_needs_reload(
                    &document.source_path,
                    document.content_sha256.as_deref(),
                    document.content_size_bytes,
                    document.content_mtime_ms,
                );
                DocumentListItem {
                    chapter_count: document.chapter_count(),
                    chunk_count: document.total_chunk_count(),
                    document_id: document.document_id,
                    title: document.title,
                    source_path,
                    content_sha256: document.content_sha256,
                    needs_reload,
                }
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
    ///
    /// Two paths:
    /// - **Engine is Paused** (the common case: user clicked play after
    ///   pausing) → just unpause; the queued WAV resumes from where it
    ///   left off.
    /// - **Engine is Stopped** (the queue was cleared by `stop_session`,
    ///   `set_default_voice` after a voice swap, or a fresh
    ///   `restore_session` at app start) → route through
    ///   `replay_session_at_position`. This re-synthesizes the current
    ///   chunk under the *current* voice + language and feeds it to the
    ///   engine. Without this, after a voice change the user would hit
    ///   play and still hear the old voice's cached audio.
    pub fn resume_session(&mut self) -> Result<(), RuntimeError> {
        let session = self
            .session_repository
            .get_active_session()
            .ok_or(RuntimeError::MissingActiveSession)?;
        let prev_pb = self.playback_engine.snapshot();
        log::info!(
            "[runtime] resume_session session.playback_state={:?} engine_state={:?} engine_last={} audio_ref={:?}",
            session.playback_state,
            prev_pb.state,
            prev_pb.last_action,
            prev_pb.audio_reference,
        );
        if matches!(prev_pb.state, PlaybackState::Stopped) {
            log::info!(
                "[runtime] resume_session: engine is Stopped → routing through replay_session_at_position"
            );
            return self.replay_session_at_position(session, "resume_session");
        }
        let mut session = session;
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
        self.event_sink
            .emit(RuntimeEvent::SessionStopped { document_id });
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

    /// Set linear playback volume. 0.0 = muted, 1.0 = full. Values > 1.0
    /// amplify — accepted by rodio but may distort.
    pub fn set_volume(&mut self, volume: f32) {
        self.playback_engine.set_volume(volume);
    }

    /// Current playback volume. 1.0 for engines that don't track it.
    pub fn volume(&self) -> f32 {
        self.playback_engine.volume()
    }

    /// Jump directly to a specific `(section, chunk)` position in the
    /// active document. Used by the reading view's click-to-seek — the
    /// user taps a chunk paragraph and playback resumes from there. The
    /// position must exist in the document; out-of-bounds returns
    /// `EmptyDocument` (same error variant `seek_relative_chunk` uses).
    pub fn seek_to_chunk(
        &mut self,
        section_index: usize,
        chunk_index: usize,
    ) -> Result<(), RuntimeError> {
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
        // Validate — the caller might pass stale indices if the document
        // changed under them. Avoid advancing to a phantom position.
        let valid = document
            .sections
            .iter()
            .any(|s| s.index == section_index && s.chunks.iter().any(|c| c.index == chunk_index));
        if !valid {
            return Err(RuntimeError::EmptyDocument {
                document_id: session.document_id.clone(),
            });
        }
        session.position.section_index = section_index;
        session.position.chunk_index = chunk_index;
        session.position.char_offset = 0;
        self.replay_session_at_position(session, "seek_to_chunk")
    }

    /// Create a voice note attached to the current reading position.
    /// `audio_path` attaches a recorded WAV to the note so callers can
    /// play the user's own voice back (see mac-gui's note playback).
    /// Dictation uses `Some(path)` (fed by the AEC recorder); typed
    /// notes and bookmarks pass `None`.
    pub fn create_note(
        &mut self,
        text: &str,
        audio_path: Option<std::path::PathBuf>,
    ) -> Result<VoiceNote, RuntimeError> {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Err(RuntimeError::MissingActiveSession);
        }

        let mut session = self
            .session_repository
            .get_active_session()
            .ok_or(RuntimeError::MissingActiveSession)?;
        let note = VoiceNote {
            // UUID v4: process-local + monotonic. The previous
            // `format!("note-{}", NOTE_COUNTER.fetch_add(1, ...))`
            // restarted from 1 on every app launch, so the second
            // session's `note-1` overwrote the first session's `note-1`
            // through `save_note`'s `ON CONFLICT(note_id) DO UPDATE`.
            // UUIDs make collisions cryptographically impossible.
            note_id: uuid::Uuid::new_v4().to_string(),
            session_id: session.session_id.clone(),
            document_id: session.document_id.clone(),
            position: session.position.clone(),
            transcript: trimmed.to_string(),
            transcription_provider: if audio_path.is_some() {
                "dictation".to_string()
            } else {
                "manual".to_string()
            },
            language: session
                .command_language
                .clone()
                .unwrap_or_else(|| "und".to_string()),
            raw_audio_path: audio_path,
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

    /// Delete a note by id. Idempotent — returns `Ok(false)` when the id
    /// is not present. Does not touch any active session (deleting a note
    /// doesn't move the reading position).
    pub fn delete_note(&mut self, note_id: &str) -> Result<bool, RuntimeError> {
        self.note_repository
            .delete_note(note_id)
            .map_err(|e| RuntimeError::Runtime(format!("delete_note: {e}")))
    }

    /// Remove a document from the library. Cascades to chunks, sections,
    /// notes and sessions via the storage repository. If the document
    /// being removed is the active session's document, stop the session
    /// first so the UI doesn't end up rendering a ghost chunk.
    ///
    /// **TTS cache**: not swept. The cache is content-addressed
    /// (`sha256(text + voice + lang)`); cache entries for this doc's
    /// chunks may still be useful (e.g. another doc has identical
    /// chunks, the user re-imports later). Orphaned WAVs are ~50 KB
    /// each — disk waste isn't worth the complexity of tracking
    /// reference counts across documents.
    pub fn delete_document(&mut self, document_id: &str) -> Result<bool, RuntimeError> {
        // Stop session if it's for this doc.
        if let Some(s) = self.session_repository.get_active_session() {
            if s.document_id == document_id {
                let _ = self.stop_session();
            }
        }
        self.document_repository
            .delete_document(document_id)
            .map_err(|e| RuntimeError::Runtime(format!("delete_document: {e}")))
    }

    /// Re-ingest a document from its source file on disk. Used when the
    /// user edits the source externally (the GUI flags the row as
    /// "modificato dall'ultima importazione" via
    /// `DocumentListItem::needs_reload`). The path-stable id derivation
    /// in `build_document_from_import` ensures the resulting document
    /// lands on the same `document_id`, so the existing row is upserted
    /// in place — notes / sessions / future AI elaborations stay
    /// attached.
    ///
    /// **TTS cache**: nothing to evict. The cache is content-addressed
    /// (`sha256(text + voice + lang)`), so chunks that survived the
    /// edit unchanged keep their cache entry, edited chunks naturally
    /// miss-and-resynth. Orphaned WAVs from the old text are ~50 KB
    /// each — disk waste isn't worth tracking ref-counts for.
    pub fn reload_document(
        &mut self,
        document_id: &str,
    ) -> Result<DocumentIngestionOutcome, RuntimeError> {
        let document = self
            .document_repository
            .get_document(document_id)
            .ok_or_else(|| RuntimeError::MissingDocument {
                document_id: document_id.to_string(),
            })?;
        let source_path = document.source_path.clone();
        self.ingest_path(&source_path).map_err(|e| {
            RuntimeError::Runtime(format!("reload_document: ingest failed: {e}"))
        })
    }

/// Overwrite the transcript of an existing note. Looks up the note,
    /// mutates the transcript, saves back through the `save_note`
    /// upsert. Returns the updated note; errors when the id is unknown.
    pub fn update_note(
        &mut self,
        note_id: &str,
        new_text: &str,
    ) -> Result<VoiceNote, RuntimeError> {
        let trimmed = new_text.trim();
        if trimmed.is_empty() {
            return Err(RuntimeError::Runtime(
                "update_note: empty transcript".to_string(),
            ));
        }
        let mut note = self
            .note_repository
            .get_note(note_id)
            .ok_or_else(|| RuntimeError::Runtime(format!("note not found: {note_id}")))?;
        let text_changed = note.transcript != trimmed;
        note.transcript = trimmed.to_string();
        // Editing the text invalidates any attached recording: the WAV
        // captured the original dictated words, which the playback UI
        // would otherwise return verbatim while the displayed text
        // says something different. Strip the reference (and delete
        // the WAV from disk, since nothing else points at it) so
        // playback of an edited note falls back to TTS of the new text.
        if text_changed {
            if let Some(path) = note.raw_audio_path.take() {
                if let Err(e) = std::fs::remove_file(&path) {
                    log::debug!(
                        "update_note: could not remove stale audio {}: {e}",
                        path.display()
                    );
                }
                note.transcription_provider = "manual".to_string();
            }
        }
        self.note_repository
            .save_note(note.clone())
            .map_err(|e| RuntimeError::Runtime(format!("update_note save: {e}")))?;
        Ok(note)
    }

    /// List all saved notes for a document, newest first.
    /// Pass `None` to target the document of the active session; returns
    /// empty when no session is active.
    pub fn list_notes(&self, document_id: Option<&str>) -> Vec<VoiceNote> {
        let target = match document_id {
            Some(id) => id.to_string(),
            None => match self.session_repository.get_active_session() {
                Some(s) => s.document_id,
                None => return Vec::new(),
            },
        };
        let mut notes = self.note_repository.list_notes_for_document(&target);
        notes.sort_by_key(|n| std::cmp::Reverse(n.created_at));
        notes
    }

    /// Synthesize a one-off preview sample with the current TTS backend.
    /// Used by the GUI's Settings page to let users hear a voice before
    /// committing to it. Bypasses the per-chunk cache — the result is a
    /// short throwaway file under `tts_cache_dir` (the caller is responsible
    /// for playback and doesn't need to clean it up; the cache eviction
    /// policy will reap it eventually).
    pub fn synthesize_preview(
        &mut self,
        text: &str,
        voice: &str,
        language: &str,
    ) -> Result<SynthesisResult, SynthesisError> {
        let request = SynthesisRequest {
            text: text.to_string(),
            voice: Some(voice.to_string()),
            language: language.to_string(),
        };
        self.tts.synthesize(request)
    }

    /// Generate a diagnostic report of all configured providers and their status.
    pub fn doctor_report(&self) -> serde_json::Value {
        let playback_name = self.playback_engine.describe_capabilities().provider_name;
        let tts_name = self.tts.describe_capabilities().provider_name;
        let stt_name = self
            .command_recognizer
            .describe_capabilities()
            .provider_name;
        // `try_lock` so we don't block the doctor report if a dictation
        // is currently in progress — report "busy" in that (rare) case.
        let dictation_name = match self.dictation_transcriber.try_lock() {
            Ok(guard) => guard.describe_capabilities().provider_name,
            Err(_) => "busy".to_string(),
        };

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
        *self.dictation_transcriber.lock().unwrap() = Box::new(transcriber);
    }

    /// Convenience: set both command recognizer and dictation transcriber from
    /// an `SttEngineOutput`. Use this when the engine factory returns a matched
    /// pair (e.g. `new_apple_stt`, or a future unified Whisper factory).
    pub fn set_stt_engine(&mut self, output: SttEngineOutput) {
        self.command_recognizer = output.command_recognizer;
        *self.dictation_transcriber.lock().unwrap() = output.dictation_transcriber;
    }

    /// Cloneable handle to the dictation transcriber — used by the FFI's
    /// dedicated dictation thread so the blocking `transcribe()` call
    /// doesn't hold the runtime-wide lock.
    pub fn dictation_transcriber_handle(&self) -> Arc<Mutex<Box<dyn DictationTranscriber + Send>>> {
        self.dictation_transcriber.clone()
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
        log::info!(
            "[runtime] replay_session_at_position cmd={command_name} doc={} sec={} chunk={}",
            session.document_id,
            session.position.section_index,
            session.position.chunk_index,
        );
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
        log::info!("[runtime] replay: about to synthesize_cached");
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
        log::info!(
            "[runtime] replay: synth done audio_ref={} bytes={}",
            synthesis.audio_reference,
            synthesis.byte_length,
        );
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
        let note = runtime.create_note("remember this", None).unwrap();
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
        assert!(stopped.unwrap().is_none(), "no active session after stop");

        let _ = fs::remove_file(path);
    }

    #[test]
    fn sqlite_runtime_restore_session() {
        let path = temp_path("md");
        fs::write(&path, "Alpha beta gamma delta epsilon zeta eta theta.").unwrap();

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
        assert_eq!(session.position.chunk_index, pos_before.chunk_index);

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
        let note = runtime.create_note("test note", None).unwrap();
        assert_eq!(note.transcript, "test note");

        // 5. Pause + resume
        runtime.pause_session().unwrap();
        assert_eq!(runtime.session_snapshot().unwrap().unwrap().state, "paused");
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
