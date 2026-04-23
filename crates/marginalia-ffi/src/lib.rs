//! Marginalia UniFFI bindings.
//!
//! This crate is the single FFI surface between the Rust runtime and any
//! non-Rust consumer (SwiftUI on macOS/iOS, Kotlin on Android, Python for
//! scripting). All types exposed through the generated bindings are defined
//! in `marginalia.udl`; this file wires them to the real `marginalia-runtime`.
//!
//! # Thread model
//!
//! The runtime owns `!Send` resources via `RuntimeSidecar.aec_pipeline` (the
//! `cpal::Stream`). UniFFI, however, requires all exposed objects to be
//! `Send + Sync`. We resolve this by running a dedicated **sidecar thread**
//! that exclusively owns the sidecar and the `ReconfigureContext`. The FFI
//! object (`FfiRuntime`) communicates with the thread over an mpsc channel
//! and a reply `oneshot` (here: `sync_channel(1)`).
//!
//! - Discovery calls are filesystem-only and run on the caller's thread
//!   without touching the sidecar thread.
//! - `apply_provider_spec` is sent to the sidecar thread, which locks the
//!   shared `SqliteRuntime` mutex, runs the reconfiguration, and replies.
//!
//! Dropping `FfiRuntime` closes the command channel → the sidecar thread
//! exits → the `RuntimeSidecar` is dropped → the mic stream closes and the
//! Apple helper subprocess is killed. This is the clean shutdown path.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{mpsc, Arc, Mutex};
use std::thread::JoinHandle;

use marginalia_config::AppConfig;
use marginalia_runtime::reconfigure::{self, ReconfigureContext};
use marginalia_runtime::{Discovery, RuntimeBuilder, SqliteRuntime};

mod conversions;

uniffi::include_scaffolding!("marginalia");

/// Max number of events kept in the poll buffer. Older events drop on
/// overflow (oldest-first). 256 is enough for ~30 s of TTS activity at
/// the typical emission rate (synthesis + chunk-advance + auto-advance).
const EVENT_BUFFER_CAP: usize = 256;

// ──────────────────────────────────────────────────────────────────────
// Error type
// ──────────────────────────────────────────────────────────────────────

/// FFI-friendly error enum. Each variant carries a human-readable message
/// in Swift via `.description` / `.localizedDescription`.
#[derive(Debug, thiserror::Error)]
pub enum FfiError {
    #[error("config error: {0}")]
    Config(String),
    #[error("io error: {0}")]
    Io(String),
    #[error("runtime build failed: {0}")]
    Build(String),
    #[error("reconfigure failed: {0}")]
    Reconfigure(String),
    #[error("runtime error: {0}")]
    Runtime(String),
    #[error("ingestion failed: {0}")]
    Ingestion(String),
    #[error("no active session")]
    NoSession,
}

// Convenience conversions so `?` works nicely in the FfiRuntime impls.
impl From<marginalia_runtime::RuntimeError> for FfiError {
    fn from(e: marginalia_runtime::RuntimeError) -> Self {
        match e {
            marginalia_runtime::RuntimeError::MissingActiveSession => FfiError::NoSession,
            other => FfiError::Runtime(other.to_string()),
        }
    }
}

// ──────────────────────────────────────────────────────────────────────
// Records (mirrors of runtime types, converted at the FFI boundary)
// ──────────────────────────────────────────────────────────────────────

/// Voice gender, as surfaced in the UI.
#[derive(Debug, Clone, Copy)]
pub enum Gender {
    Female,
    Male,
    Unknown,
}

impl From<marginalia_runtime::Gender> for Gender {
    fn from(g: marginalia_runtime::Gender) -> Self {
        match g {
            marginalia_runtime::Gender::Female => Gender::Female,
            marginalia_runtime::Gender::Male => Gender::Male,
            marginalia_runtime::Gender::Unknown => Gender::Unknown,
        }
    }
}

pub struct VoiceInfo {
    pub id: String,
    pub display: String,
    pub lang: String,
    pub gender: Gender,
    pub backend: String,
}

impl From<marginalia_runtime::VoiceInfo> for VoiceInfo {
    fn from(v: marginalia_runtime::VoiceInfo) -> Self {
        Self {
            id: v.id,
            display: v.display,
            lang: v.lang,
            gender: v.gender.into(),
            backend: v.backend,
        }
    }
}

pub struct TtsBackend {
    pub id: String,
    pub name: String,
    pub available: bool,
    pub reason: Option<String>,
}

impl From<marginalia_runtime::TtsBackend> for TtsBackend {
    fn from(b: marginalia_runtime::TtsBackend) -> Self {
        Self {
            id: b.id,
            name: b.name,
            available: b.available,
            reason: b.reason,
        }
    }
}

pub struct SttEngine {
    pub id: String,
    pub name: String,
    pub available: bool,
    pub reason: Option<String>,
}

impl From<marginalia_runtime::SttEngine> for SttEngine {
    fn from(e: marginalia_runtime::SttEngine) -> Self {
        Self {
            id: e.id,
            name: e.name,
            available: e.available,
            reason: e.reason,
        }
    }
}

pub struct LangInfo {
    pub bcp47: String,
    pub display: String,
    pub has_tts: bool,
    pub has_stt: bool,
}

impl From<marginalia_runtime::LangInfo> for LangInfo {
    fn from(l: marginalia_runtime::LangInfo) -> Self {
        Self {
            bcp47: l.bcp47,
            display: l.display,
            has_tts: l.has_tts,
            has_stt: l.has_stt,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProviderSpec {
    pub tts_backend: String,
    pub voice: String,
    pub stt_engine: String,
    pub language: String,
}

impl From<reconfigure::ProviderSpec> for ProviderSpec {
    fn from(s: reconfigure::ProviderSpec) -> Self {
        Self {
            tts_backend: s.tts_backend,
            voice: s.voice,
            stt_engine: s.stt_engine,
            language: s.language,
        }
    }
}

impl From<ProviderSpec> for reconfigure::ProviderSpec {
    fn from(s: ProviderSpec) -> Self {
        Self {
            tts_backend: s.tts_backend,
            voice: s.voice,
            stt_engine: s.stt_engine,
            language: s.language,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ApplyReport {
    pub tts_swapped: bool,
    pub stt_swapped: bool,
    pub language_changed: bool,
    pub elapsed_ms: u64,
}

impl From<reconfigure::ApplyReport> for ApplyReport {
    fn from(r: reconfigure::ApplyReport) -> Self {
        Self {
            tts_swapped: r.tts_swapped,
            stt_swapped: r.stt_swapped,
            language_changed: r.language_changed,
            elapsed_ms: r.elapsed_ms,
        }
    }
}

// ──────────────────────────────────────────────────────────────────────
// Sidecar-thread command protocol
// ──────────────────────────────────────────────────────────────────────

/// Payload sent once from the sidecar thread to the parent at init time.
/// Carries the Send handles the parent needs after the sidecar has built
/// the runtime. !Send resources (AecPipeline, cpal::Stream) stay inside
/// the sidecar thread.
struct SidecarInit {
    runtime: Arc<Mutex<SqliteRuntime>>,
    #[cfg(feature = "apple-stt")]
    waveform_handle:
        Option<Arc<Mutex<marginalia_runtime::builder::WaveformData>>>,
}

enum SidecarCmd {
    ApplySpec {
        spec: reconfigure::ProviderSpec,
        reply: mpsc::SyncSender<Result<reconfigure::ApplyReport, String>>,
    },
    SaveConfig {
        voice_commands: marginalia_config::VoiceCommandsSection,
        chunk_target_chars: u32,
        stt_debug: bool,
        reply: mpsc::SyncSender<Result<(), String>>,
    },
}

// ──────────────────────────────────────────────────────────────────────
// FfiRuntime
// ──────────────────────────────────────────────────────────────────────

// ──────────────────────────────────────────────────────────────────────
// New in J — reader/library/event types
// ──────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy)]
pub enum PlaybackState {
    Idle,
    Playing,
    Paused,
    Finished,
    Unknown,
}

pub struct DocumentListItem {
    pub id: String,
    pub title: String,
    pub chapter_count: u32,
    pub chunk_count: u32,
}

pub struct IngestResult {
    pub document_id: String,
    pub title: String,
}

pub struct ChunkView {
    pub index: u32,
    pub anchor: String,
    pub text: String,
    pub is_active: bool,
    pub is_read: bool,
}

pub struct SectionView {
    pub index: u32,
    pub title: String,
    pub source_anchor: Option<String>,
    pub chunks: Vec<ChunkView>,
}

pub struct DocumentView {
    pub document_id: String,
    pub title: String,
    pub source_path: String,
    pub chapter_count: u32,
    pub chunk_count: u32,
    pub active_section_index: Option<u32>,
    pub active_chunk_index: Option<u32>,
    pub sections: Vec<SectionView>,
}

pub struct SessionSnapshot {
    pub session_id: String,
    pub document_id: String,
    pub anchor: String,
    pub state: String,
    pub playback_state: PlaybackState,
    pub section_index: u32,
    pub section_count: u32,
    pub chunk_index: u32,
    pub chunk_text: String,
    pub section_title: String,
    pub notes_count: u32,
    pub voice: Option<String>,
    pub tts_provider: Option<String>,
    pub command_stt_provider: Option<String>,
    pub command_listening_active: bool,
}

pub struct AppSnapshot {
    pub state: String,
    pub document_count: u32,
    pub active_session_id: Option<String>,
    pub latest_document_id: Option<String>,
    pub playback_state: Option<String>,
    pub runtime_status: Option<String>,
}

pub struct NoteView {
    pub note_id: String,
    pub document_id: String,
    pub session_id: String,
    pub anchor: String,
    pub section_index: u32,
    pub chunk_index: u32,
    pub text: String,
    pub language: String,
    pub transcription_provider: String,
    pub created_at_iso: String,
}

#[derive(Debug, Clone)]
pub enum FfiRuntimeEvent {
    ChunkAdvanced {
        document_id: String,
        section_index: u32,
        chunk_index: u32,
    },
    SynthesisStarted {
        document_id: String,
        section_index: u32,
        chunk_index: u32,
    },
    SynthesisReady {
        document_id: String,
        section_index: u32,
        chunk_index: u32,
        cache_hit: bool,
    },
    PlaybackFinished {
        document_id: String,
        section_index: u32,
        chunk_index: u32,
    },
    CommandRecognized {
        raw_text: String,
        command: Option<String>,
    },
    SessionRestored {
        session_id: String,
        document_id: String,
        section_index: u32,
        chunk_index: u32,
    },
    SessionStopped {
        document_id: String,
    },
    IngestStarted {
        source: String,
    },
    IngestFinished {
        source: String,
        document_id: Option<String>,
        error_message: Option<String>,
    },
    DictationStarted,
    VoiceNoteTranscribed {
        text: String,
        duration_secs: f64,
        note_id: Option<String>,
        error_message: Option<String>,
    },
    VoiceMismatch {
        document_id: String,
        detected_language: String,
        current_language: String,
    },
    Error {
        message: String,
    },
}

/// Shared event buffer drained by `poll_events`. Populated by a dedicated
/// drainer thread that consumes from `runtime.subscribe_events()`.
#[derive(Clone, Default)]
struct EventBuffer {
    inner: Arc<Mutex<VecDeque<FfiRuntimeEvent>>>,
}

impl EventBuffer {
    fn push(&self, ev: FfiRuntimeEvent) {
        let mut q = self.inner.lock().unwrap();
        if q.len() >= EVENT_BUFFER_CAP {
            q.pop_front();
        }
        q.push_back(ev);
    }

    fn drain(&self) -> Vec<FfiRuntimeEvent> {
        let mut q = self.inner.lock().unwrap();
        q.drain(..).collect()
    }
}

// ──────────────────────────────────────────────────────────────────────
// Installable asset catalog (onboarding + Settings downloader)
// ──────────────────────────────────────────────────────────────────────

/// What the catalog declares about a downloadable asset. Hardcoded here
/// because we ship a fixed set of providers — adding a new row is a
/// source change, not a user-facing config knob.
#[derive(Debug, Clone, Copy)]
struct AssetSpec {
    id: &'static str,
    display_name: &'static str,
    category: &'static str,
    language: Option<&'static str>,
    gender: Option<&'static str>,
    size_bytes: u64,
    source: AssetSource,
}

#[derive(Debug, Clone, Copy)]
enum AssetSource {
    /// Kokoro MLX safetensors — root file (`kokoro-v1_0.safetensors`).
    MlxCore { file: &'static str },
    /// Kokoro MLX voice embedding (`voices/{id}.safetensors`).
    MlxVoice { voice_id: &'static str },
    /// Whisper ggml (`ggml-small.bin`, `ggml-medium.bin`, …).
    Whisper { file: &'static str },
    /// Kokoro ONNX fallback (`onnx/model_q8f16.onnx`).
    KokoroOnnx { file: &'static str },
}

const CATALOG: &[AssetSpec] = &[
    AssetSpec {
        id: "mlx-core",
        display_name: "Kokoro TTS (MLX)",
        category: "tts_core",
        language: None,
        gender: None,
        size_bytes: 310_000_000,
        source: AssetSource::MlxCore {
            file: "kokoro-v1_0.safetensors",
        },
    },
    AssetSpec {
        id: "voice:if_sara",
        display_name: "Sara (italiano, F)",
        category: "voice",
        language: Some("it-IT"),
        gender: Some("female"),
        size_bytes: 500_000,
        source: AssetSource::MlxVoice {
            voice_id: "if_sara",
        },
    },
    AssetSpec {
        id: "voice:im_nicola",
        display_name: "Nicola (italiano, M)",
        category: "voice",
        language: Some("it-IT"),
        gender: Some("male"),
        size_bytes: 500_000,
        source: AssetSource::MlxVoice {
            voice_id: "im_nicola",
        },
    },
    AssetSpec {
        id: "voice:af_bella",
        display_name: "Bella (English, F)",
        category: "voice",
        language: Some("en-US"),
        gender: Some("female"),
        size_bytes: 500_000,
        source: AssetSource::MlxVoice {
            voice_id: "af_bella",
        },
    },
    AssetSpec {
        id: "whisper-small",
        display_name: "Whisper small (multilingue)",
        category: "stt",
        language: None,
        gender: None,
        size_bytes: 465_000_000,
        source: AssetSource::Whisper {
            file: "ggml-small.bin",
        },
    },
    AssetSpec {
        id: "kokoro-onnx",
        display_name: "Kokoro ONNX (fallback)",
        category: "tts_core",
        language: None,
        gender: None,
        size_bytes: 34_000_000,
        source: AssetSource::KokoroOnnx {
            file: "onnx/model_q8f16.onnx",
        },
    },
];

/// FFI-exposed catalog row.
pub struct InstallableAsset {
    pub id: String,
    pub display_name: String,
    pub category: String,
    pub language: Option<String>,
    pub gender: Option<String>,
    pub size_bytes: u64,
    pub installed: bool,
    pub source_host: String,
    pub notes: Option<String>,
}

impl InstallableAsset {
    fn from_spec(spec: &AssetSpec) -> Self {
        Self {
            id: spec.id.to_string(),
            display_name: spec.display_name.to_string(),
            category: spec.category.to_string(),
            language: spec.language.map(String::from),
            gender: spec.gender.map(String::from),
            size_bytes: spec.size_bytes,
            installed: is_asset_cached(&spec.source),
            source_host: "huggingface.co".to_string(),
            notes: None,
        }
    }
}

/// FFI-exposed progress frame.
#[derive(Debug, Clone)]
pub struct InstallProgress {
    pub asset_id: String,
    pub state: String,
    pub bytes_done: u64,
    pub bytes_total: u64,
    pub error_message: Option<String>,
}

/// `hf-hub` `Progress` impl that pushes byte-level frames into the
/// shared install ring buffer. Used by `install_asset` so the UI can
/// render a real percent bar instead of an indeterminate spinner. To
/// avoid flooding the 64-slot buffer with 16 KB chunks on a 300 MB
/// download, we coalesce: only emit a frame when either 2% or 512 KB
/// of additional progress has accumulated since the last emission.
struct InstallProgressReporter {
    asset_id: String,
    buf: InstallBuffer,
    total: u64,
    done: u64,
    last_emitted_done: u64,
}

impl InstallProgressReporter {
    fn new(asset_id: String, buf: InstallBuffer) -> Self {
        Self {
            asset_id,
            buf,
            total: 0,
            done: 0,
            last_emitted_done: 0,
        }
    }
    fn emit(&self) {
        self.buf.push(InstallProgress {
            asset_id: self.asset_id.clone(),
            state: "downloading".into(),
            bytes_done: self.done,
            bytes_total: self.total,
            error_message: None,
        });
    }
}

impl hf_hub::api::Progress for InstallProgressReporter {
    fn init(&mut self, size: usize, _filename: &str) {
        self.total = size as u64;
        self.done = 0;
        self.last_emitted_done = 0;
        self.emit();
    }
    fn update(&mut self, size: usize) {
        self.done = self.done.saturating_add(size as u64);
        // Coalesce: 2% step OR 512 KB, whichever comes first.
        let delta = self.done.saturating_sub(self.last_emitted_done);
        let pct_step = self.total / 50; // 2%
        if delta >= pct_step.max(512 * 1024) {
            self.last_emitted_done = self.done;
            self.emit();
        }
    }
    fn finish(&mut self) {
        self.done = self.total;
        self.emit();
    }
}

/// Same ring-buffer pattern as `EventBuffer`. Separate from events because
/// the cadence and semantics are different (~500 ms poll for progress vs
/// 100 ms for playback events).
#[derive(Clone, Default)]
struct InstallBuffer {
    inner: Arc<Mutex<VecDeque<InstallProgress>>>,
}

impl InstallBuffer {
    fn push(&self, p: InstallProgress) {
        let mut q = self.inner.lock().unwrap();
        // Progress buffer is small — keep the last 64 frames. Enough for
        // several assets downloaded concurrently.
        if q.len() >= 64 {
            q.pop_front();
        }
        q.push_back(p);
    }

    fn drain(&self) -> Vec<InstallProgress> {
        let mut q = self.inner.lock().unwrap();
        q.drain(..).collect()
    }
}

/// Probe HF cache for the given asset. Uses offline-mode `ensure_*` — it
/// resolves when the file is cached, errors when it isn't. This never
/// hits the network.
fn is_asset_cached(source: &AssetSource) -> bool {
    // `set_var` is unsafe in Rust 2024+; wrap for forward compat. The app
    // is single-process so the env mutation is race-free for this guard —
    // no other thread flips `HF_HUB_OFFLINE` at runtime.
    unsafe { std::env::set_var("HF_HUB_OFFLINE", "1") };
    let mgr = match marginalia_models::ModelManager::new() {
        Ok(m) => m,
        Err(_) => return false,
    };
    match source {
        AssetSource::MlxCore { file } => mgr.ensure_mlx_core(file).is_ok(),
        AssetSource::MlxVoice { voice_id } => mgr.ensure_mlx_voice(voice_id).is_ok(),
        AssetSource::Whisper { file } => mgr.ensure_whisper(file).is_ok(),
        AssetSource::KokoroOnnx { file } => mgr.ensure_kokoro_onnx(file).is_ok(),
    }
}

/// Download the asset with byte-level progress reporting via the supplied
/// `InstallProgressReporter`. Must be called with `HF_HUB_OFFLINE` unset
/// — the caller (`install_asset`) flips the env var around this call.
fn download_asset_with_progress(
    source: &AssetSource,
    progress: InstallProgressReporter,
) -> Result<PathBuf, String> {
    let mgr = marginalia_models::ModelManager::new().map_err(|e| e.to_string())?;
    let (repo, file): (&str, String) = match source {
        AssetSource::MlxCore { file } => ("prince-canuma/Kokoro-82M", file.to_string()),
        AssetSource::MlxVoice { voice_id } => (
            "prince-canuma/Kokoro-82M",
            format!("voices/{voice_id}.safetensors"),
        ),
        AssetSource::Whisper { file } => ("ggerganov/whisper.cpp", file.to_string()),
        AssetSource::KokoroOnnx { file } => ("onnx-community/Kokoro-82M", file.to_string()),
    };
    mgr.download_with_progress(repo, &file, progress)
        .map_err(|e| e.to_string())
}

// ──────────────────────────────────────────────────────────────────────
// FfiRuntime
// ──────────────────────────────────────────────────────────────────────

/// The FFI-exposed runtime. Cheap to hold; does not own mutable state directly
/// (state lives on the sidecar thread).
pub struct FfiRuntime {
    runtime: Arc<Mutex<SqliteRuntime>>,
    discovery: Discovery,
    cmd_tx: mpsc::Sender<SidecarCmd>,
    event_buffer: EventBuffer,
    install_buffer: InstallBuffer,
    /// Cached at init time so the UI can display "path" without taking the
    /// runtime lock. Re-reads on every call are acceptable too but this is
    /// simpler.
    whisper_path_cached: Option<PathBuf>,
    #[cfg(feature = "apple-stt")]
    waveform_handle:
        Option<Arc<Mutex<marginalia_runtime::builder::WaveformData>>>,
    _sidecar: Mutex<Option<JoinHandle<()>>>,
}

/// Snapshot of the AEC waveform buffers at a point in time.
#[derive(Debug, Clone, Default)]
pub struct WaveformSnapshot {
    pub tts_levels: Vec<f32>,
    pub mic_levels: Vec<f32>,
}

impl FfiRuntime {
    /// Open the runtime using a `marginalia.toml` at `config_path`. Starts
    /// the sidecar thread and initializes all providers.
    ///
    /// UniFFI wraps the returned `Self` in an `Arc` for the binding's lifetime
    /// management, so we return the bare type here.
    pub fn new(config_path: String) -> Result<Self, FfiError> {
        let config_path = PathBuf::from(&config_path);
        if !config_path.is_file() {
            return Err(FfiError::Config(format!(
                "marginalia.toml not found at {}",
                config_path.display()
            )));
        }

        // Load the TOML synchronously so setup errors surface on the caller's
        // thread rather than on the sidecar.
        let tui = AppConfig::load_from(&config_path).map_err(FfiError::Config)?;

        let db_path = tui
            .database_path
            .clone()
            .unwrap_or_else(|| PathBuf::from(".marginalia/beta.sqlite3"));
        let mlx_model_dir = PathBuf::from(&tui.mlx.model);
        let whisper_model_path = tui.stt.whisper.model_path.clone();
        let whisper_path_cached = whisper_model_path.clone();
        let discovery = Discovery::new(&mlx_model_dir, whisper_model_path);

        // Build the runtime on a dedicated thread — `RuntimeBuilder::build()`
        // produces both the Send `SqliteRuntime` and the `!Send` sidecar.
        // We keep the sidecar on that thread for the rest of its life.
        let (init_tx, init_rx) =
            mpsc::sync_channel::<Result<SidecarInit, String>>(1);
        let (cmd_tx, cmd_rx) = mpsc::channel::<SidecarCmd>();

        let mlx_cfg = tui.mlx.clone();
        let stt_cfg = tui.stt.clone();
        let voice_cmds = tui.voice_commands.clone();
        let kokoro_cfg = tui.kokoro.clone();
        let playback_cfg = tui.playback.clone();
        let database_path_opt = tui.database_path.clone();
        let chunk_target_chars_opt = tui.chunk_target_chars;
        let config_path_clone = config_path.clone();
        let event_buffer = EventBuffer::default();
        let event_buffer_for_thread = event_buffer.clone();
        let mut runtime_cfg = marginalia_runtime::RuntimeConfig::default();
        if let Some(v) = tui.chunk_target_chars {
            runtime_cfg.chunk_target_chars = v;
        }
        if let Some(dir) = tui.tts_cache_dir.clone() {
            runtime_cfg.tts_cache_dir = Some(dir);
        }
        let db_path_clone = db_path.clone();

        let handle = std::thread::Builder::new()
            .name("marginalia-sidecar".into())
            .spawn(move || {
                // Clone the sections that we'll need AFTER moving the originals
                // into RuntimeBuilder — the sidecar keeps its own copies in ctx.
                let kokoro_cfg_for_ctx = kokoro_cfg.clone();
                let playback_cfg_for_ctx = playback_cfg.clone();

                let result = RuntimeBuilder::new(&db_path_clone)
                    .config(runtime_cfg)
                    .voice_commands(voice_cmds.clone())
                    .stt(stt_cfg.clone())
                    .kokoro(kokoro_cfg)
                    .mlx(mlx_cfg.clone())
                    .playback(playback_cfg)
                    .build();

                let (mut sidecar, mut runtime_owned) = match result {
                    Ok(output) => {
                        let runtime_arc = Arc::new(Mutex::new(output.runtime));
                        #[cfg(feature = "apple-stt")]
                        let waveform_handle = output.sidecar.waveform_data.clone();
                        let init = SidecarInit {
                            runtime: runtime_arc.clone(),
                            #[cfg(feature = "apple-stt")]
                            waveform_handle,
                        };
                        if init_tx.send(Ok(init)).is_err() {
                            return; // parent gave up
                        }
                        (output.sidecar, runtime_arc)
                    }
                    Err(e) => {
                        let _ = init_tx.send(Err(e));
                        return;
                    }
                };

                let tts_cache_dir = runtime_owned
                    .lock()
                    .unwrap()
                    .config()
                    .tts_cache_dir
                    .clone()
                    .unwrap_or_else(|| PathBuf::from(".marginalia/tts-cache"));

                // Install the event drainer: subscribe once, spawn a thread
                // that pushes every RuntimeEvent into the shared buffer so
                // `poll_events` on the FFI can read them without locking the
                // runtime. The drainer exits when the runtime's event sender
                // is dropped (runtime shutdown).
                let event_rx = runtime_owned.lock().unwrap().subscribe_events();
                let drainer_buf = event_buffer_for_thread.clone();
                std::thread::Builder::new()
                    .name("marginalia-event-drainer".into())
                    .spawn(move || {
                        while let Ok(event) = event_rx.recv() {
                            drainer_buf.push(event.into());
                        }
                    })
                    .ok();

                #[cfg(all(feature = "apple-stt", feature = "host-playback"))]
                let aec_render_slot = reconfigure::AecRenderSlot::new();

                let mut ctx = ReconfigureContext {
                    mlx: mlx_cfg,
                    stt: stt_cfg,
                    voice_commands: voice_cmds,
                    tts_cache_dir,
                    kokoro: kokoro_cfg_for_ctx,
                    playback: playback_cfg_for_ctx,
                    database_path: database_path_opt,
                    chunk_target_chars: chunk_target_chars_opt,
                    config_path: Some(config_path_clone),
                    #[cfg(all(feature = "apple-stt", feature = "host-playback"))]
                    aec_render_slot,
                };

                // Command loop: drains the command channel until the parent
                // drops the sender, at which point recv() returns Err and we
                // fall through to drop `sidecar` (stops mic + kills helper).
                while let Ok(cmd) = cmd_rx.recv() {
                    match cmd {
                        SidecarCmd::ApplySpec { spec, reply } => {
                            let res = {
                                let mut rt = runtime_owned.lock().unwrap();
                                reconfigure::apply_provider_spec(
                                    &mut *rt, &mut sidecar, &mut ctx, &spec,
                                )
                            };
                            let _ = reply.send(res);
                        }
                        SidecarCmd::SaveConfig {
                            voice_commands,
                            chunk_target_chars,
                            stt_debug,
                            reply,
                        } => {
                            ctx.voice_commands = voice_commands;
                            ctx.chunk_target_chars = Some(chunk_target_chars as usize);
                            ctx.stt.debug = stt_debug;
                            let res = ctx.save();
                            let _ = reply.send(res);
                        }
                    }
                }

                let _ = &mut runtime_owned;
                // Sidecar drops here.
            })
            .map_err(|e| FfiError::Build(format!("spawn sidecar thread: {e}")))?;

        let init = init_rx
            .recv()
            .map_err(|_| FfiError::Build("sidecar thread exited before init".into()))?
            .map_err(FfiError::Build)?;

        Ok(Self {
            runtime: init.runtime,
            discovery,
            cmd_tx,
            event_buffer,
            install_buffer: InstallBuffer::default(),
            whisper_path_cached,
            #[cfg(feature = "apple-stt")]
            waveform_handle: init.waveform_handle,
            _sidecar: Mutex::new(Some(handle)),
        })
    }

    /// Current provider selection. Reads the runtime config; does not touch
    /// the sidecar thread.
    pub fn current_spec(&self) -> ProviderSpec {
        let rt = self.runtime.lock().unwrap();
        let cfg = rt.config();
        reconfigure::ProviderSpec {
            tts_backend: current_tts_backend().to_string(),
            voice: cfg.default_voice.clone(),
            stt_engine: "apple".to_string(), // TODO: persist last applied engine
            language: cfg.default_language.clone(),
        }
        .into()
    }

    /// Apply a new provider selection. Blocks until the sidecar thread
    /// finishes the rebuild (typical: 0.3–1 s when the Apple STT helper
    /// respawns, <50 ms for a voice-only swap).
    pub fn apply_provider_spec(&self, spec: ProviderSpec) -> Result<ApplyReport, FfiError> {
        let (reply_tx, reply_rx) = mpsc::sync_channel(1);
        self.cmd_tx
            .send(SidecarCmd::ApplySpec {
                spec: spec.into(),
                reply: reply_tx,
            })
            .map_err(|_| FfiError::Reconfigure("sidecar thread has exited".into()))?;
        match reply_rx.recv() {
            Ok(Ok(report)) => Ok(report.into()),
            Ok(Err(e)) => Err(FfiError::Reconfigure(e)),
            Err(_) => Err(FfiError::Reconfigure("sidecar thread dropped reply".into())),
        }
    }

    pub fn list_voices(&self, backend: String) -> Vec<VoiceInfo> {
        self.discovery
            .list_voices(&backend)
            .into_iter()
            .map(Into::into)
            .collect()
    }

    pub fn list_tts_backends(&self) -> Vec<TtsBackend> {
        self.discovery
            .list_tts_backends()
            .into_iter()
            .map(Into::into)
            .collect()
    }

    pub fn list_stt_engines(&self) -> Vec<SttEngine> {
        self.discovery
            .list_stt_engines()
            .into_iter()
            .map(Into::into)
            .collect()
    }

    pub fn list_languages(&self) -> Vec<LangInfo> {
        self.discovery
            .list_languages()
            .into_iter()
            .map(Into::into)
            .collect()
    }

    /// Persist non-provider settings to the on-disk `marginalia.toml`.
    /// Provider settings (TTS/STT/voice/language) are persisted by
    /// `apply_provider_spec` updating the context; this method catches
    /// everything else: voice-command triggers, chunk size, STT debug.
    pub fn save_config(
        &self,
        voice_commands: Vec<VoiceCommandEntry>,
        chunk_target_chars: u32,
        stt_debug: bool,
    ) -> Result<(), FfiError> {
        let cfg_vc = voice_commands_entries_to_section(voice_commands);
        let (reply_tx, reply_rx) = mpsc::sync_channel(1);
        self.cmd_tx
            .send(SidecarCmd::SaveConfig {
                voice_commands: cfg_vc,
                chunk_target_chars,
                stt_debug,
                reply: reply_tx,
            })
            .map_err(|_| FfiError::Io("sidecar thread has exited".into()))?;
        match reply_rx.recv() {
            Ok(Ok(())) => Ok(()),
            Ok(Err(e)) => Err(FfiError::Io(e)),
            Err(_) => Err(FfiError::Io("sidecar thread dropped reply".into())),
        }
    }

    // ─────────────────────────────────────────────────────────
    // Library / documents
    // ─────────────────────────────────────────────────────────

    pub fn list_documents(&self) -> Vec<DocumentListItem> {
        self.runtime
            .lock()
            .unwrap()
            .list_documents()
            .into_iter()
            .map(Into::into)
            .collect()
    }

    pub fn document_view(&self, document_id: Option<String>) -> Option<DocumentView> {
        self.runtime
            .lock()
            .unwrap()
            .document_view(document_id.as_deref())
            .map(Into::into)
    }

    pub fn ingest_file(&self, path: String) -> Result<IngestResult, FfiError> {
        let p = std::path::Path::new(&path).to_path_buf();
        let mut rt = self.runtime.lock().unwrap();
        match rt.ingest_path(&p) {
            Ok(outcome) => Ok(ingest_outcome_to_result(outcome)),
            Err(e) => Err(FfiError::Ingestion(format!("{e:?}"))),
        }
    }

    pub fn delete_document(&self, document_id: String) -> Result<(), FfiError> {
        self.runtime.lock().unwrap().delete_document(&document_id)?;
        Ok(())
    }

    pub fn ingest_url(&self, url: String) -> Result<IngestResult, FfiError> {
        #[cfg(feature = "url-import")]
        {
            let mut rt = self.runtime.lock().unwrap();
            match rt.ingest_url(&url) {
                Ok(outcome) => Ok(ingest_outcome_to_result(outcome)),
                Err(e) => Err(FfiError::Ingestion(format!("{e:?}"))),
            }
        }
        #[cfg(not(feature = "url-import"))]
        {
            let _ = url;
            Err(FfiError::Ingestion(
                "url-import feature not enabled".to_string(),
            ))
        }
    }

    // ─────────────────────────────────────────────────────────
    // Session lifecycle
    // ─────────────────────────────────────────────────────────

    pub fn start_session(&self, document_id: String) -> Result<(), FfiError> {
        self.runtime
            .lock()
            .unwrap()
            .start_session(&document_id)?;
        Ok(())
    }

    pub fn restore_session(&self) -> Result<bool, FfiError> {
        Ok(self.runtime.lock().unwrap().restore_session().is_some())
    }

    pub fn pause_session(&self) -> Result<(), FfiError> {
        self.runtime.lock().unwrap().pause_session()?;
        Ok(())
    }

    pub fn resume_session(&self) -> Result<(), FfiError> {
        self.runtime.lock().unwrap().resume_session()?;
        Ok(())
    }

    pub fn stop_session(&self) -> Result<(), FfiError> {
        self.runtime.lock().unwrap().stop_session()?;
        Ok(())
    }

    // ─────────────────────────────────────────────────────────
    // Playback navigation
    // ─────────────────────────────────────────────────────────

    pub fn next_chunk(&self) -> Result<(), FfiError> {
        self.runtime.lock().unwrap().next_chunk()?;
        Ok(())
    }
    pub fn previous_chunk(&self) -> Result<(), FfiError> {
        self.runtime.lock().unwrap().previous_chunk()?;
        Ok(())
    }
    pub fn next_chapter(&self) -> Result<(), FfiError> {
        self.runtime.lock().unwrap().next_chapter()?;
        Ok(())
    }
    pub fn previous_chapter(&self) -> Result<(), FfiError> {
        self.runtime.lock().unwrap().previous_chapter()?;
        Ok(())
    }
    pub fn restart_chapter(&self) -> Result<(), FfiError> {
        self.runtime.lock().unwrap().restart_chapter()?;
        Ok(())
    }
    pub fn repeat_chunk(&self) -> Result<(), FfiError> {
        self.runtime.lock().unwrap().repeat_chunk()?;
        Ok(())
    }
    /// Click-to-seek: jump to `(section, chunk)` in the active document.
    pub fn seek_to_chunk(
        &self,
        section_index: u32,
        chunk_index: u32,
    ) -> Result<(), FfiError> {
        self.runtime
            .lock()
            .unwrap()
            .seek_to_chunk(section_index as usize, chunk_index as usize)?;
        Ok(())
    }
    pub fn auto_advance(&self) -> bool {
        self.runtime.lock().unwrap().try_auto_advance()
    }

    pub fn set_volume(&self, level: f32) {
        self.runtime.lock().unwrap().set_volume(level);
    }

    pub fn volume(&self) -> f32 {
        self.runtime.lock().unwrap().volume()
    }

    /// Kick off a voice-note dictation. Returns immediately; the blocking
    /// `transcribe()` call runs on a dedicated thread so the UI's 100 ms
    /// event poll stays responsive. Events:
    ///
    ///   1. `DictationStarted` — helper in DICTATION mode, recording
    ///   2. `VoiceNoteTranscribed { text, note_id }` on success
    ///      OR `VoiceNoteTranscribed { error_message }` on failure
    ///
    /// No-op-ish when called with no active session: the dictation still
    /// runs (user might want to dictate a free-floating note) but
    /// `create_note` will fail with `NoSession`; the failure surfaces
    /// as `VoiceNoteTranscribed { error_message: "no active session" }`.
    pub fn start_dictation(&self) -> Result<(), FfiError> {
        let runtime = self.runtime.clone();
        let event_buf = self.event_buffer.clone();

        // 1. Brief runtime lock: grab dictation handle. Pausing playback
        // is left to the UI — reading stays active so the user can
        // reference the current chunk while dictating.
        let dictation_handle = {
            let rt = runtime.lock().unwrap();
            rt.dictation_transcriber_handle()
        };

        std::thread::Builder::new()
            .name("marginalia-dictation".into())
            .spawn(move || {
                // 2. Announce we're listening so the UI shows the live-note
                // card in "recording" state.
                event_buf.push(FfiRuntimeEvent::DictationStarted);

                // 3. Blocking transcribe — only the transcriber mutex is
                // held here, not the runtime mutex, so other FFI calls
                // keep flowing while the user speaks. session_id/note_id
                // are currently unused by the Apple transcriber (see
                // `crates/marginalia-stt-apple/src/lib.rs`).
                let transcript = {
                    let mut t = dictation_handle.lock().unwrap();
                    t.transcribe(None, None)
                };
                let text = transcript.text.trim().to_string();
                let duration = transcript
                    .segments
                    .iter()
                    .map(|s| s.end_ms.saturating_sub(s.start_ms) as f64 / 1000.0)
                    .sum::<f64>();

                if text.is_empty() {
                    event_buf.push(FfiRuntimeEvent::VoiceNoteTranscribed {
                        text: String::new(),
                        duration_secs: duration,
                        note_id: None,
                        error_message: Some(
                            "dettatura vuota o non riconosciuta".to_string(),
                        ),
                    });
                    return;
                }

                // 4. Re-lock runtime to persist the note at the current
                // reading position. `create_note` emits its own note
                // repository activity; we wrap with our event so the UI
                // can light the live-note card in one place.
                let create_result = {
                    let mut rt = runtime.lock().unwrap();
                    rt.create_note(&text)
                };
                match create_result {
                    Ok(note) => event_buf.push(FfiRuntimeEvent::VoiceNoteTranscribed {
                        text,
                        duration_secs: duration,
                        note_id: Some(note.note_id),
                        error_message: None,
                    }),
                    Err(e) => event_buf.push(FfiRuntimeEvent::VoiceNoteTranscribed {
                        text,
                        duration_secs: duration,
                        note_id: None,
                        error_message: Some(format!("{e:?}")),
                    }),
                }
            })
            .map_err(|e| FfiError::Io(format!("spawn dictation thread: {e}")))?;
        Ok(())
    }

    // ─────────────────────────────────────────────────────────
    // State
    // ─────────────────────────────────────────────────────────

    pub fn app_snapshot(&self) -> AppSnapshot {
        self.runtime.lock().unwrap().app_snapshot().into()
    }

    pub fn session_snapshot(&self) -> Option<SessionSnapshot> {
        match self.runtime.lock().unwrap().session_snapshot() {
            Ok(snap) => snap.map(Into::into),
            Err(_) => None,
        }
    }

    pub fn doctor_report_json(&self) -> String {
        let blob = self.runtime.lock().unwrap().doctor_report();
        blob.to_string()
    }

    // ─────────────────────────────────────────────────────────
    // Notes
    // ─────────────────────────────────────────────────────────

    pub fn create_note(&self, text: String) -> Result<NoteView, FfiError> {
        let note = self.runtime.lock().unwrap().create_note(&text)?;
        Ok(note.into())
    }

    pub fn list_notes(&self, document_id: Option<String>) -> Vec<NoteView> {
        self.runtime
            .lock()
            .unwrap()
            .list_notes(document_id.as_deref())
            .into_iter()
            .map(Into::into)
            .collect()
    }

    /// Remove a note. Idempotent — unknown ids return Ok without an
    /// error (same semantics as the runtime method).
    pub fn delete_note(&self, note_id: String) -> Result<(), FfiError> {
        self.runtime.lock().unwrap().delete_note(&note_id)?;
        Ok(())
    }

    /// Overwrite the transcript of an existing note.
    pub fn update_note(
        &self,
        note_id: String,
        new_text: String,
    ) -> Result<NoteView, FfiError> {
        let note = self
            .runtime
            .lock()
            .unwrap()
            .update_note(&note_id, &new_text)?;
        Ok(note.into())
    }

    // ─────────────────────────────────────────────────────────
    // Event polling
    // ─────────────────────────────────────────────────────────

    pub fn poll_events(&self) -> Vec<FfiRuntimeEvent> {
        self.event_buffer.drain()
    }

    /// Synthesize a one-off preview WAV through the live TTS backend.
    pub fn synthesize_preview(
        &self,
        text: String,
        voice: String,
        language: String,
    ) -> Result<String, FfiError> {
        let mut rt = self.runtime.lock().unwrap();
        match rt.synthesize_preview(&text, &voice, &language) {
            Ok(res) => Ok(res.audio_reference),
            Err(e) => Err(FfiError::Runtime(format!("{e:?}"))),
        }
    }

    /// Read the configured Whisper model path. Empty string = unset.
    pub fn whisper_model_path(&self) -> String {
        self.whisper_path_cached
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_default()
    }

    /// Snapshot the AEC waveform buffers. Empty arrays on non-apple-stt
    /// builds or before the first mic frame. Lock is brief — the AEC
    /// thread only ever does `try_lock` so we don't starve it.
    #[cfg(feature = "apple-stt")]
    pub fn poll_waveform(&self) -> WaveformSnapshot {
        let Some(handle) = self.waveform_handle.as_ref() else {
            return WaveformSnapshot::default();
        };
        let guard = match handle.try_lock() {
            Ok(g) => g,
            Err(_) => return WaveformSnapshot::default(),
        };
        WaveformSnapshot {
            tts_levels: guard.tts_levels.clone(),
            mic_levels: guard.mic_levels.clone(),
        }
    }
    #[cfg(not(feature = "apple-stt"))]
    pub fn poll_waveform(&self) -> WaveformSnapshot {
        WaveformSnapshot::default()
    }

    // ─────────────────────────────────────────────────────────
    // Installable assets
    // ─────────────────────────────────────────────────────────

    /// Catalog of downloadable assets with current installed state. Cheap:
    /// the installed flag is a cache probe (no network). Called on every
    /// Settings/Onboarding open.
    pub fn list_installable_assets(&self) -> Vec<InstallableAsset> {
        CATALOG.iter().map(InstallableAsset::from_spec).collect()
    }

    /// Kick off an asynchronous download for the named asset. Returns
    /// immediately; progress is reported through `install_progress()`.
    /// The background thread temporarily disables `HF_HUB_OFFLINE`
    /// (which the runtime otherwise forces to `1`), downloads via
    /// `marginalia-models`, then restores the flag.
    pub fn install_asset(&self, asset_id: String) -> Result<(), FfiError> {
        let spec = CATALOG
            .iter()
            .find(|s| s.id == asset_id)
            .ok_or_else(|| FfiError::Io(format!("unknown asset '{asset_id}'")))?;
        let buf = self.install_buffer.clone();
        let source = spec.source;
        let id = asset_id.clone();
        std::thread::Builder::new()
            .name(format!("install-{id}"))
            .spawn(move || {
                buf.push(InstallProgress {
                    asset_id: id.clone(),
                    state: "queued".into(),
                    bytes_done: 0,
                    bytes_total: 0,
                    error_message: None,
                });
                // Open the network gate for this thread. `set_var` is
                // process-wide, but no other thread attempts a fetch
                // concurrently with the install flow (runtime is in
                // onboarding mode or paused), so the race window is safe.
                unsafe { std::env::remove_var("HF_HUB_OFFLINE") };
                // `InstallProgressReporter` pushes its own "downloading"
                // frames with real byte counts once hf-hub calls init().
                let reporter = InstallProgressReporter::new(id.clone(), buf.clone());
                let result = download_asset_with_progress(&source, reporter);
                unsafe { std::env::set_var("HF_HUB_OFFLINE", "1") };
                match result {
                    Ok(_path) => buf.push(InstallProgress {
                        asset_id: id,
                        state: "installed".into(),
                        bytes_done: 0,
                        bytes_total: 0,
                        error_message: None,
                    }),
                    Err(e) => {
                        // Classify common error strings into friendly
                        // messages. The hf-hub error bubbles up as a
                        // string containing the underlying io::Error's
                        // Display, so matching on substrings is the
                        // most stable contract available without
                        // downcasting through several wrapper types.
                        let friendly: String = if e.contains("No space left") {
                            "Spazio su disco insufficiente. Libera spazio e riprova.".into()
                        } else if e.contains("Too many retries")
                            || e.contains("dns error")
                            || e.contains("failed to lookup")
                        {
                            "Problema di rete: impossibile raggiungere huggingface.co. Verifica la connessione e riprova.".into()
                        } else if e.contains("Permission denied") {
                            "Permesso negato sulla cartella modelli. Controlla le autorizzazioni di sistema.".into()
                        } else {
                            e
                        };
                        buf.push(InstallProgress {
                            asset_id: id,
                            state: "error".into(),
                            bytes_done: 0,
                            bytes_total: 0,
                            error_message: Some(friendly),
                        })
                    }
                }
            })
            .map_err(|e| FfiError::Io(format!("spawn install thread: {e}")))?;
        Ok(())
    }

    /// Drain the install-progress ring buffer. Swift polls this every
    /// ~500 ms from a separate timer than `poll_events` (progress has
    /// looser timing needs than playback state).
    pub fn install_progress(&self) -> Vec<InstallProgress> {
        self.install_buffer.drain()
    }

    /// Remove a previously-installed asset from the HF cache. Synchronous
    /// and fast (one `unlink` of the blob + the snapshot symlink), so we
    /// don't bother with a background thread here. Unknown asset_id is an
    /// error; "asset not currently installed" is a no-op (Ok).
    pub fn uninstall_asset(&self, asset_id: String) -> Result<(), FfiError> {
        let spec = CATALOG
            .iter()
            .find(|s| s.id == asset_id)
            .ok_or_else(|| FfiError::Io(format!("unknown asset '{asset_id}'")))?;
        let (repo, file): (&str, String) = match &spec.source {
            AssetSource::MlxCore { file } => ("prince-canuma/Kokoro-82M", file.to_string()),
            AssetSource::MlxVoice { voice_id } => (
                "prince-canuma/Kokoro-82M",
                format!("voices/{voice_id}.safetensors"),
            ),
            AssetSource::Whisper { file } => ("ggerganov/whisper.cpp", file.to_string()),
            AssetSource::KokoroOnnx { file } => ("onnx-community/Kokoro-82M", file.to_string()),
        };
        marginalia_models::ModelManager::uninstall_from_repo(repo, &file)
            .map_err(|e| FfiError::Io(e.to_string()))?;
        Ok(())
    }
}

/// Lift a `DocumentIngestionOutcome` to an `IngestResult`. Both the "new"
/// and "already-present" cases have a title + id — we don't differentiate in
/// the FFI boundary; callers can check `document.already_present` via a later
/// call to `list_documents` if needed.
fn ingest_outcome_to_result(
    outcome: marginalia_core::application::DocumentIngestionOutcome,
) -> IngestResult {
    IngestResult {
        document_id: outcome.document.document_id,
        title: outcome.document.title,
    }
}

/// Pair of (action, triggers) — flat surface for UniFFI since
/// `VoiceCommandsSection` has 11 named fields.
#[derive(Debug, Clone)]
pub struct VoiceCommandEntry {
    pub action: String,
    pub triggers: Vec<String>,
}

fn voice_commands_entries_to_section(
    entries: Vec<VoiceCommandEntry>,
) -> marginalia_config::VoiceCommandsSection {
    let mut out = marginalia_config::VoiceCommandsSection::default();
    for e in entries {
        match e.action.as_str() {
            "pause" => out.pause = e.triggers,
            "next" => out.next = e.triggers,
            "back" => out.back = e.triggers,
            "stop" => out.stop = e.triggers,
            "repeat" => out.repeat = e.triggers,
            "resume" => out.resume = e.triggers,
            "next_chapter" => out.next_chapter = e.triggers,
            "prev_chapter" => out.prev_chapter = e.triggers,
            "bookmark" => out.bookmark = e.triggers,
            "note" => out.note = e.triggers,
            "where" => out.r#where = e.triggers,
            other => log::warn!("save_config: ignoring unknown action '{other}'"),
        }
    }
    out
}

fn current_tts_backend() -> &'static str {
    #[cfg(feature = "mlx-tts")]
    {
        "mlx"
    }
    #[cfg(not(feature = "mlx-tts"))]
    {
        "kokoro"
    }
}
