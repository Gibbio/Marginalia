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
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc, Mutex};
use std::thread::JoinHandle;

use marginalia_config::AppConfig;
use marginalia_runtime::reconfigure::{self, ReconfigureContext};
use marginalia_runtime::{Discovery, RuntimeBuilder, SqliteRuntime};

mod conversions;
mod logger;

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

/// What `clear_all_notes` actually did, surfaced to the UI so the user
/// gets concrete feedback ("X note rimosse · Y MB liberati") rather
/// than a generic "fatto".
#[derive(Debug, Clone, Default)]
pub struct ClearNotesReport {
    pub notes_deleted: u32,
    pub bytes_freed: u64,
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
    waveform_handle: Option<Arc<Mutex<marginalia_runtime::builder::WaveformData>>>,
    /// Cloned handle to the AEC render slot so the parent thread can
    /// feed the reference signal when GUI-side playback (note WAVs via
    /// AVAudioPlayer) starts. The slot is `Clone` and stays valid even
    /// across STT helper respawns — `install` simply replaces its inner
    /// sender. None on builds without apple-stt + host-playback.
    #[cfg(all(feature = "apple-stt", feature = "host-playback"))]
    aec_render_slot: reconfigure::AecRenderSlot,
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
    /// Filesystem path of the source file at ingestion time. Used by
    /// the GUI's "Apri file in editor…" action — `NSWorkspace.shared.open`
    /// with a `URL(fileURLWithPath:)` from this string.
    pub source_path: String,
    /// SHA-256 of the source file's bytes at ingestion time. `None` for
    /// rows imported before migration `003_document_fingerprint`; the
    /// next reload backfills it.
    pub content_sha256: Option<String>,
    /// `true` iff the file currently on disk has a different SHA than
    /// the row's `content_sha256`. Computed by the runtime off-mutex
    /// during `list_documents`. `false` on IO errors (file missing,
    /// permission denied) or pre-migration rows. Drives the reload
    /// indicator in the sidebar.
    pub needs_reload: bool,
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
    /// Absolute path of the persisted raw dictation WAV. `None` when
    /// no audio is attached (typed note, bookmark, or a dictation
    /// that didn't record). Swift uses this to decide between
    /// playing the user's own voice vs TTS-ing the transcript.
    pub audio_reference: Option<String>,
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
        /// Wall-clock duration of the TTS work, measured Rust-side.
        /// 0 on cache hits.
        elapsed_ms: u64,
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

/// What the catalog declares about a downloadable asset. The engine specs
/// are hardcoded (adding a new engine is a source change). The voice specs
/// are built from `voices.manifest.json` at startup so adding a voice is
/// a manifest edit — no recompile required.
#[derive(Debug, Clone)]
struct AssetSpec {
    id: String,
    display_name: String,
    category: String,
    language: Option<String>,
    gender: Option<String>,
    size_bytes: u64,
    source: AssetSource,
}

#[derive(Debug, Clone)]
enum AssetSource {
    /// Kokoro MLX safetensors — root file (`kokoro-v1_0.safetensors`).
    MlxCore { file: String },
    /// Kokoro MLX voice embedding (`voices/{id}.safetensors`).
    MlxVoice { voice_id: String },
    /// Whisper ggml (`ggml-small.bin`, `ggml-medium.bin`, …).
    Whisper { file: String },
    /// Kokoro ONNX fallback (`onnx/model_q8f16.onnx`).
    KokoroOnnx { file: String },
}

/// Resolve the voices manifest path at runtime.
///
/// Priority:
///   1. `MARGINALIA_VOICES_MANIFEST` env var (explicit override)
///   2. `<exe>/../../Resources/models/tts/mlx/voices.manifest.json`
///      (macOS .app bundle layout — `Contents/MacOS/bin` → `Contents/Resources/…`)
///   3. `models/tts/mlx/voices.manifest.json` (dev mode, CWD-relative)
///
/// Used by `load_voices_from_manifest()`. When none of the paths exist
/// we fall back to a tiny hardcoded voice list so the catalog isn't empty
/// on a broken install (see `fallback_voices()`).
fn resolve_voices_manifest_path() -> PathBuf {
    if let Ok(p) = std::env::var("MARGINALIA_VOICES_MANIFEST") {
        let path = PathBuf::from(p);
        if path.is_file() {
            return path;
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(macos_dir) = exe.parent() {
            if let Some(contents) = macos_dir.parent() {
                let p = contents.join("Resources/models/tts/mlx/voices.manifest.json");
                if p.is_file() {
                    return p;
                }
            }
        }
    }
    PathBuf::from("models/tts/mlx/voices.manifest.json")
}

/// User-writable location where the latest fetch from huggingface.co is
/// persisted (`<config_dir>/voices.cache.json`). Set by `FfiRuntime::new`;
/// before then, and on first launch before any successful fetch, the
/// catalog falls back to the bundled manifest. Same JSON schema as the
/// bundled manifest so the parser is shared.
static CATALOG_CACHE_PATH: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

fn set_catalog_cache_path(path: PathBuf) {
    let _ = CATALOG_CACHE_PATH.set(path);
}

/// Decode a `voices.manifest.json`-shaped file into specs. Used both for
/// the bundled manifest and the user cache (same schema). Returns `None`
/// on any IO/parse failure so the caller can fall through to the next
/// tier without distinguishing missing-file from corrupt-file at the
/// call site.
fn parse_voice_manifest(path: &Path) -> Option<Vec<AssetSpec>> {
    let bytes = std::fs::read(path).ok()?;
    let v: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    let arr = v.get("voices")?.as_array()?;
    let specs: Vec<AssetSpec> = arr
        .iter()
        .filter_map(|item| {
            let id = item.get("id")?.as_str()?.to_string();
            let display = item.get("display")?.as_str()?.to_string();
            let lang = item.get("lang").and_then(|x| x.as_str()).map(String::from);
            let gender = item
                .get("gender")
                .and_then(|x| x.as_str())
                .map(String::from);
            // Human-readable label: "Sara (it-IT, F)" — keeps the row
            // compact while still surfacing language + gender.
            let gender_tag = match gender.as_deref() {
                Some("female") => "F",
                Some("male") => "M",
                _ => "?",
            };
            let lang_tag = lang.as_deref().unwrap_or("und");
            let label = format!("{display} ({lang_tag}, {gender_tag})");
            Some(AssetSpec {
                id: format!("voice:{id}"),
                display_name: label,
                category: "voice".to_string(),
                language: lang,
                gender,
                // All MLX voice embeddings are roughly the same size.
                size_bytes: 500_000,
                source: AssetSource::MlxVoice { voice_id: id },
            })
        })
        .collect();
    if specs.is_empty() {
        None
    } else {
        Some(specs)
    }
}

/// Three-tier voice spec resolution:
///   1. user cache at `<config_dir>/voices.cache.json` (refreshed via
///      `fetch_remote_voice_catalog`, written explicitly by the user
///      from onboarding or Settings)
///   2. bundled `voices.manifest.json` (shipped inside the .app)
///   3. hardcoded `fallback_voices()` (so the UI is never empty even on
///      a broken install)
fn load_voices_from_manifest() -> Vec<AssetSpec> {
    if let Some(cache) = CATALOG_CACHE_PATH.get() {
        if let Some(specs) = parse_voice_manifest(cache) {
            log::info!(
                "[catalog] voices loaded from user cache ({} entries) at {}",
                specs.len(),
                cache.display()
            );
            return specs;
        }
    }
    let bundled = resolve_voices_manifest_path();
    if let Some(specs) = parse_voice_manifest(&bundled) {
        log::info!(
            "[catalog] voices loaded from bundled manifest ({} entries) at {}",
            specs.len(),
            bundled.display()
        );
        return specs;
    }
    log::warn!("[catalog] no voices manifest readable, using hardcoded fallback");
    fallback_voices()
}

/// Map the first character of a Kokoro voice id to a BCP-47 language tag.
/// The convention is fixed by the model author (hexgrad/Kokoro-82M):
/// 1st char = language family, 2nd char = gender. Unknown prefixes return
/// `None` so a manifest with future characters (e.g. an unannounced
/// language) is silently skipped instead of mislabelled.
///
/// Tags must match the bundled `voices.manifest.json` exactly so the
/// FFI-fetched HF cache and the bundled fallback agree on the lang
/// string for the same voice id (the runtime's `voice_to_lang` map
/// gets keyed by both interchangeably).
fn kokoro_lang_from_id(voice_id: &str) -> Option<&'static str> {
    let c = voice_id.chars().next()?;
    Some(match c {
        'a' => "en-US",
        'b' => "en-GB",
        'e' => "es-ES",
        'f' => "fr-FR",
        'h' => "hi-IN",
        'i' => "it-IT",
        'j' => "ja-JP",
        'p' => "pt-BR",
        'z' => "zh-CN",
        _ => return None,
    })
}

fn kokoro_gender_from_id(voice_id: &str) -> Option<&'static str> {
    let c = voice_id.chars().nth(1)?;
    Some(match c {
        'f' => "female",
        'm' => "male",
        _ => return None,
    })
}

/// Capitalise the suffix of a Kokoro voice id (`if_sara` → `Sara`).
/// Falls back to the raw id if the underscore split fails.
fn display_from_voice_id(voice_id: &str) -> String {
    let suffix = voice_id.split('_').nth(1).unwrap_or(voice_id);
    let mut chars = suffix.chars();
    match chars.next() {
        Some(c) => c.to_uppercase().chain(chars).collect(),
        None => suffix.to_string(),
    }
}

/// Fetch the live Kokoro 82M voice catalog from huggingface.co and write
/// it to `<config_dir>/voices.cache.json`. Returns the number of voices
/// persisted on success. Caller is responsible for invalidating in-process
/// catalog state (`invalidate_catalog`) so the next `list_installable_assets`
/// call picks up the new file.
///
/// **This is one of the very few places Marginalia performs a runtime
/// network call.** It only runs when the user explicitly triggers it
/// (onboarding `installModels` step, or a button in Settings →
/// Installazioni). On failure the previous cache is left untouched, so
/// the UI keeps showing whatever was last known.
fn fetch_remote_voice_catalog_inner() -> Result<u32, String> {
    let cache_path = CATALOG_CACHE_PATH.get().ok_or_else(|| {
        "catalog cache path not initialised — call FfiRuntime::new first".to_string()
    })?;
    let url = "https://huggingface.co/api/models/prince-canuma/Kokoro-82M/tree/main/voices";
    log::info!("[catalog] fetching voice list from {url}");
    // Short per-attempt timeouts on purpose: the catalog is a tiny JSON
    // listing (~10 KB), and the user wants the UI to fall back to the
    // bundled manifest fast rather than spinning for half a minute on a
    // flaky link. We then retry transient IO errors up to twice — HF
    // Front sometimes truncates the TLS stream mid-response ("io:
    // unexpected end of file") and the next attempt succeeds. HTTP
    // status errors (4xx/5xx) are NOT retried — they aren't transient.
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_connect(Some(std::time::Duration::from_secs(3)))
        .timeout_recv_response(Some(std::time::Duration::from_secs(3)))
        .timeout_global(Some(std::time::Duration::from_secs(4)))
        .user_agent("Marginalia/0.2 (https://github.com/Gibbio/Marginalia)")
        .build()
        .into();
    const MAX_ATTEMPTS: u32 = 3;
    const RETRY_BACKOFF_MS: u64 = 400;
    let mut last_io_err: Option<String> = None;
    let body: String = 'attempts: {
        for attempt in 1..=MAX_ATTEMPTS {
            match agent.get(url).call() {
                Ok(mut resp) => {
                    let status = resp.status();
                    if !status.is_success() {
                        // HTTP-level error: not transient, bail immediately.
                        return Err(format!(
                            "HF API returned HTTP {} {}",
                            status.as_u16(),
                            status.canonical_reason().unwrap_or("")
                        ));
                    }
                    match resp.body_mut().read_to_string() {
                        Ok(body) => break 'attempts body,
                        Err(e) => {
                            log::warn!(
                                "[catalog] fetch attempt {attempt}/{MAX_ATTEMPTS} body read failed: {e}"
                            );
                            last_io_err = Some(format!("read response body: {e}"));
                        }
                    }
                }
                Err(e) => {
                    log::warn!(
                        "[catalog] fetch attempt {attempt}/{MAX_ATTEMPTS} request failed: {e}"
                    );
                    last_io_err = Some(format!("HF API request failed: {e}"));
                }
            }
            if attempt < MAX_ATTEMPTS {
                std::thread::sleep(std::time::Duration::from_millis(RETRY_BACKOFF_MS));
            }
        }
        return Err(format!(
            "HF API unreachable after {MAX_ATTEMPTS} attempts: {}",
            last_io_err.unwrap_or_else(|| "no error captured".to_string())
        ));
    };
    let entries: serde_json::Value =
        serde_json::from_str(&body).map_err(|e| format!("parse HF API JSON: {e}"))?;
    let arr = entries
        .as_array()
        .ok_or_else(|| "HF API returned non-array body".to_string())?;
    let mut voices = Vec::with_capacity(arr.len());
    for entry in arr {
        let path = match entry.get("path").and_then(|v| v.as_str()) {
            Some(p) => p,
            None => continue,
        };
        // Skip non-voice files (README, config, …) and any non-safetensors
        // tensor packs that may show up alongside voices/.
        if !path.ends_with(".safetensors") {
            continue;
        }
        let stem = match Path::new(path).file_stem().and_then(|s| s.to_str()) {
            Some(s) => s,
            None => continue,
        };
        let lang = match kokoro_lang_from_id(stem) {
            Some(l) => l,
            None => {
                log::warn!("[catalog] skipping voice with unknown language prefix: {stem}");
                continue;
            }
        };
        let gender = kokoro_gender_from_id(stem).unwrap_or("unknown");
        let display = display_from_voice_id(stem);
        voices.push(serde_json::json!({
            "id": stem,
            "display": display,
            "lang": lang,
            "gender": gender,
        }));
    }
    if voices.is_empty() {
        return Err("HF API listing returned no recognizable voices".to_string());
    }
    voices.sort_by(|a, b| {
        a["lang"]
            .as_str()
            .unwrap_or("")
            .cmp(b["lang"].as_str().unwrap_or(""))
            .then_with(|| {
                a["id"]
                    .as_str()
                    .unwrap_or("")
                    .cmp(b["id"].as_str().unwrap_or(""))
            })
    });
    let count = voices.len() as u32;
    let fetched_at_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let cache_doc = serde_json::json!({
        "schema_version": 1,
        "fetched_at_ms": fetched_at_ms,
        "source": url,
        "voices": voices,
    });
    if let Some(parent) = cache_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("create cache dir {}: {e}", parent.display()))?;
    }
    std::fs::write(
        cache_path,
        serde_json::to_string_pretty(&cache_doc).map_err(|e| format!("serialize cache: {e}"))?,
    )
    .map_err(|e| format!("write cache {}: {e}", cache_path.display()))?;
    log::info!("[catalog] wrote {count} voices to {}", cache_path.display());
    invalidate_catalog();
    Ok(count)
}

fn invalidate_catalog() {
    let mut w = CATALOG_CACHE.write().unwrap();
    *w = None;
}

/// Minimal voice set used when the manifest can't be read. Keeps the UI
/// functional (user sees at least one Italian + one English voice) while
/// surfacing the underlying error in the log.
fn fallback_voices() -> Vec<AssetSpec> {
    vec![
        AssetSpec {
            id: "voice:if_sara".to_string(),
            display_name: "Sara (it-IT, F)".to_string(),
            category: "voice".to_string(),
            language: Some("it-IT".to_string()),
            gender: Some("female".to_string()),
            size_bytes: 500_000,
            source: AssetSource::MlxVoice {
                voice_id: "if_sara".to_string(),
            },
        },
        AssetSpec {
            id: "voice:af_bella".to_string(),
            display_name: "Bella (en-US, F)".to_string(),
            category: "voice".to_string(),
            language: Some("en-US".to_string()),
            gender: Some("female".to_string()),
            size_bytes: 500_000,
            source: AssetSource::MlxVoice {
                voice_id: "af_bella".to_string(),
            },
        },
    ]
}

/// Hardcoded engine specs — adding one is a source change because each
/// maps to a distinct backend crate. Order matters: `mlx-core` stays
/// first so `list_installable_assets` surfaces the macOS primary before
/// the cross-platform ONNX fallback.
///
/// The Kokoro ONNX weights live at `onnx-community/Kokoro-82M-v1.0-ONNX`.
/// The older `onnx-community/Kokoro-82M` repo went gated in 2026-Q1
/// (401 on public downloads) — make sure future edits target the
/// `-v1.0-ONNX` slug.
fn engine_specs() -> Vec<AssetSpec> {
    vec![
        AssetSpec {
            id: "mlx-core".to_string(),
            display_name: "Kokoro TTS (MLX)".to_string(),
            category: "tts_core".to_string(),
            language: None,
            gender: None,
            size_bytes: 310_000_000,
            source: AssetSource::MlxCore {
                file: "kokoro-v1_0.safetensors".to_string(),
            },
        },
        AssetSpec {
            id: "whisper-small".to_string(),
            display_name: "Whisper small (multilingue)".to_string(),
            category: "stt".to_string(),
            language: None,
            gender: None,
            size_bytes: 465_000_000,
            source: AssetSource::Whisper {
                file: "ggml-small.bin".to_string(),
            },
        },
        AssetSpec {
            id: "kokoro-onnx".to_string(),
            display_name: "Kokoro ONNX (fallback cross-platform)".to_string(),
            category: "tts_core".to_string(),
            language: None,
            gender: None,
            size_bytes: 86_000_000,
            source: AssetSource::KokoroOnnx {
                file: "onnx/model_q8f16.onnx".to_string(),
            },
        },
    ]
}

/// In-process snapshot of the asset catalog (engines + voices). Lazy-built
/// from `current_catalog()`; invalidated to `None` whenever a fetch writes
/// a new `voices.cache.json` so the next call rereads the file.
///
/// Was a `LazyLock<Vec<AssetSpec>>`; replaced with an explicit RwLock so
/// `fetch_remote_voice_catalog` can clear the cached snapshot without a
/// process restart.
static CATALOG_CACHE: std::sync::RwLock<Option<Vec<AssetSpec>>> = std::sync::RwLock::new(None);

fn current_catalog() -> Vec<AssetSpec> {
    {
        let g = CATALOG_CACHE.read().unwrap();
        if let Some(v) = g.as_ref() {
            return v.clone();
        }
    }
    let mut out = engine_specs();
    out.extend(load_voices_from_manifest());
    let mut w = CATALOG_CACHE.write().unwrap();
    *w = Some(out.clone());
    out
}

/// Build the `voice_id → BCP-47` map from the current catalog. Each
/// voice spec carries its language as the `language` field (loaded
/// from `voices.manifest.json` or the user-cached HF fetch). Used to
/// seed `runtime.set_voice_to_lang_map` at boot and after every
/// `fetch_remote_voice_catalog` so a voice swap also snaps the
/// runtime's `default_language` to whatever the new voice implies.
/// Single source of truth for the voice→lang relation; no hardcoded
/// `id-prefix → lang` table anywhere.
fn build_voice_to_lang_map() -> std::collections::HashMap<String, String> {
    current_catalog()
        .into_iter()
        .filter_map(|spec| {
            // The catalog stores voices with id `voice:<voice_id>`;
            // `set_default_voice` is called with the bare `<voice_id>`
            // (matches what the runtime / FFI passes around). Strip
            // the prefix so the lookup keys align.
            let id = spec.id.strip_prefix("voice:").map(str::to_string)?;
            let lang = spec.language?;
            Some((id, lang))
        })
        .collect()
}

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
            id: spec.id.clone(),
            display_name: spec.display_name.clone(),
            category: spec.category.clone(),
            language: spec.language.clone(),
            gender: spec.gender.clone(),
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

/// Probe the HuggingFace cache for the given asset — **filesystem only,
/// no hf-hub round-trip**. The old implementation used `ensure_*` with
/// `HF_HUB_OFFLINE=1`, which racily flipped a process-wide env var
/// alongside `install_asset` (which flips it off to allow network). When
/// a Settings refresh (57 probes in a row) overlapped with an install,
/// some probes saw `HF_HUB_OFFLINE` unset and went online — one stuck
/// network retry would block `list_installable_assets` for minutes.
///
/// HF cache layout used here (stable since hf-hub 0.5):
///   $HF_HOME/hub/models--{owner}--{name}/refs/main   → the current rev
///   $HF_HOME/hub/models--{owner}--{name}/snapshots/{rev}/{relative_path}
fn is_asset_cached(source: &AssetSource) -> bool {
    let (owner, name, rel): (&str, &str, String) = match source {
        AssetSource::MlxCore { file } => ("prince-canuma", "Kokoro-82M", file.clone()),
        AssetSource::MlxVoice { voice_id } => (
            "prince-canuma",
            "Kokoro-82M",
            format!("voices/{voice_id}.safetensors"),
        ),
        AssetSource::Whisper { file } => ("ggerganov", "whisper.cpp", file.clone()),
        AssetSource::KokoroOnnx { file } => {
            ("onnx-community", "Kokoro-82M-v1.0-ONNX", file.clone())
        }
    };
    let repo_dir = hf_cache_root().join(format!("models--{owner}--{name}"));
    let Ok(rev) = std::fs::read_to_string(repo_dir.join("refs/main")) else {
        return false;
    };
    let rev = rev.trim();
    if rev.is_empty() {
        return false;
    }
    let target = repo_dir.join("snapshots").join(rev).join(&rel);
    // `symlink_metadata` accepts dangling symlinks (returns the link's
    // own metadata) — that would give false positives if the blob was
    // removed out-of-band. `metadata` follows the symlink and returns
    // an error for dangling, which is what we want.
    std::fs::metadata(&target).is_ok()
}

/// Resolve the HuggingFace Hub cache root the same way hf-hub does:
/// `HF_HOME/hub` wins, then `HUGGINGFACE_HUB_CACHE`, then
/// `$HOME/.cache/huggingface/hub` (macOS/Linux). Matches hf-hub 0.5.
fn hf_cache_root() -> PathBuf {
    if let Ok(p) = std::env::var("HF_HOME") {
        return PathBuf::from(p).join("hub");
    }
    if let Ok(p) = std::env::var("HUGGINGFACE_HUB_CACHE") {
        return PathBuf::from(p);
    }
    let home = std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."));
    home.join(".cache/huggingface/hub")
}

/// Copy a downloaded asset from its HF-cache path into the user's
/// configured `mlx_model_dir` so `Discovery::list_voices` (which scans
/// that directory) sees it. Non-MLX assets (Whisper, Kokoro ONNX) are
/// skipped — they don't belong in the MLX dir and the runtime knows
/// how to find them in the HF cache on its own.
///
/// Target layout mirrors what `make bootstrap-mlx` produces:
///   {dir}/kokoro-v1_0.safetensors
///   {dir}/voices/{voice_id}.safetensors
///
/// Failures are propagated; the caller logs and continues (the TTS
/// runtime still finds the file in the HF cache via hf-hub, so the
/// mirror miss only affects the UI's voice picker visibility).
fn mirror_to_mlx_dir(
    cached: &std::path::Path,
    source: &AssetSource,
    dir: &std::path::Path,
) -> std::io::Result<()> {
    let target: PathBuf = match source {
        AssetSource::MlxCore { file } => dir.join(file),
        AssetSource::MlxVoice { voice_id } => {
            dir.join("voices").join(format!("{voice_id}.safetensors"))
        }
        _ => return Ok(()), // non-MLX, nothing to do
    };
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)?;
    }
    // Copy rather than symlink: HF cache can be pruned, and the .app
    // may not have permission to write into the cache dir anyway.
    // Overwrites any existing file (covers the "reinstall after
    // manual deletion" case cleanly).
    std::fs::copy(cached, &target)?;
    log::info!("[install] mirrored to {}", target.display());
    Ok(())
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
        AssetSource::KokoroOnnx { file } => {
            ("onnx-community/Kokoro-82M-v1.0-ONNX", file.to_string())
        }
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
    /// Config file path — stored so `export_backup` / `import_backup`
    /// can include/restore it without the caller re-supplying.
    config_path: PathBuf,
    /// SQLite database path — same reason as `config_path`.
    db_path: PathBuf,
    #[cfg(feature = "apple-stt")]
    waveform_handle: Option<Arc<Mutex<marginalia_runtime::builder::WaveformData>>>,
    /// Forward the AEC render reference from the parent thread when the
    /// GUI plays a note WAV via AVAudioPlayer (which bypasses the rodio
    /// host engine and therefore the existing render callback). Cloned
    /// from the sidecar at init; `install` happens on the sidecar but
    /// the inner `Arc<Mutex<…>>` is shared, so calls here see the live
    /// AEC sender as long as a helper is running.
    #[cfg(all(feature = "apple-stt", feature = "host-playback"))]
    aec_render_slot: reconfigure::AecRenderSlot,
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

        // Install the file logger as the very first thing so `log::*` calls
        // anywhere downstream (runtime, playback-host, AEC) reach disk. Done
        // before the config-not-found check so we record that error too. The
        // log file lands next to marginalia.toml (overridable via
        // MARGINALIA_FFI_LOG_FILE); failures fall back to stderr (which a
        // sandboxed .app routes into Console.app) and don't block startup.
        let log_dir: PathBuf = config_path
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));
        match logger::init_in_dir(&log_dir) {
            Ok(p) => log::info!(
                "[ffi] FfiRuntime::new pid={} config={} log={}",
                std::process::id(),
                config_path.display(),
                p.display(),
            ),
            Err(e) => eprintln!("[marginalia-ffi] log init failed: {e}"),
        }

        // Tell the catalog where to read/write the user-cached voice list.
        // The cache is only written by `fetch_remote_voice_catalog`, never
        // implicitly: a fresh install with no internet still falls back
        // to the bundled `voices.manifest.json` and works offline.
        set_catalog_cache_path(log_dir.join("voices.cache.json"));

        if !config_path.is_file() {
            return Err(FfiError::Config(format!(
                "marginalia.toml not found at {}",
                config_path.display()
            )));
        }

        // Load the TOML synchronously so setup errors surface on the caller's
        // thread rather than on the sidecar.
        let tui = AppConfig::load_from(&config_path).map_err(FfiError::Config)?;

        // Relative `database_path` / `tts_cache_dir` in the TOML are
        // resolved against the config file's parent directory. Without
        // this, a sandboxed `.app` launched from `/` tries to open
        // `.marginalia/beta.sqlite3` literally — which fails because
        // CWD is not under a writable location. The TUI on desktop
        // works fine because it runs from the repo root, but the
        // .app bundle needs the explicit resolution.
        let config_dir: PathBuf = config_path
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));
        let resolve_rel = |p: PathBuf| -> PathBuf {
            if p.is_absolute() {
                p
            } else {
                config_dir.join(p)
            }
        };

        let db_path = resolve_rel(
            tui.database_path
                .clone()
                .unwrap_or_else(|| PathBuf::from(".marginalia/beta.sqlite3")),
        );
        let mlx_model_dir = PathBuf::from(&tui.mlx.model);
        let whisper_model_path = tui.stt.whisper.model_path.clone();
        let whisper_path_cached = whisper_model_path.clone();
        let discovery = Discovery::new(&mlx_model_dir, whisper_model_path);

        // Build the runtime on a dedicated thread — `RuntimeBuilder::build()`
        // produces both the Send `SqliteRuntime` and the `!Send` sidecar.
        // We keep the sidecar on that thread for the rest of its life.
        let (init_tx, init_rx) = mpsc::sync_channel::<Result<SidecarInit, String>>(1);
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
            runtime_cfg.tts_cache_dir = Some(resolve_rel(dir));
        }
        // Normalize the runtime's default_language to BCP-47 so the
        // host UI's `host.languages` (BCP-47 entries from Discovery
        // like "it-IT", "en-US") matches what `currentSpec.language`
        // reports. Without this, RuntimeConfig::default() set "it"
        // and the LangPicker compared "it" to "it-IT" → no selection
        // + the "no voices for this language" filter went empty even
        // when Italian voices were installed.
        runtime_cfg.default_language = reconfigure::normalize_apple_language(&tui.stt.language);
        let db_path_clone = db_path.clone();
        // Captured for use inside the sidecar thread when the config
        // didn't specify a cache dir — same resolution rule as db_path.
        let default_cache_dir = config_dir.join(".marginalia/tts-cache");

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

                // Captured from `output.aec_render_slot` in the Ok arm so the
                // post-init `ReconfigureContext` can reuse the same slot
                // (avoids the pre-existing two-slot bug where STT respawns
                // updated a slot the playback callbacks didn't observe).
                #[cfg(all(feature = "apple-stt", feature = "host-playback"))]
                let build_aec_slot: reconfigure::AecRenderSlot;
                let (mut sidecar, mut runtime_owned) = match result {
                    Ok(output) => {
                        let runtime_arc = Arc::new(Mutex::new(output.runtime));
                        #[cfg(feature = "apple-stt")]
                        let waveform_handle = output.sidecar.waveform_data.clone();
                        #[cfg(all(feature = "apple-stt", feature = "host-playback"))]
                        let slot = output.aec_render_slot.clone();
                        // Push the catalog-derived voice→lang map so
                        // `runtime.set_default_voice` can snap the
                        // language to whatever the chosen voice
                        // implies. Without this, switching from
                        // bm_george to im_nicola would leave the
                        // runtime's `default_language` at en-GB and
                        // espeak-ng would phonemize Italian text with
                        // English rules → "Nicola con accento british".
                        if let Ok(mut rt) = runtime_arc.lock() {
                            rt.set_voice_to_lang_map(build_voice_to_lang_map());
                        }
                        let init = SidecarInit {
                            runtime: runtime_arc.clone(),
                            #[cfg(feature = "apple-stt")]
                            waveform_handle,
                            #[cfg(all(feature = "apple-stt", feature = "host-playback"))]
                            aec_render_slot: slot.clone(),
                        };
                        if init_tx.send(Ok(init)).is_err() {
                            return; // parent gave up
                        }
                        #[cfg(all(feature = "apple-stt", feature = "host-playback"))]
                        {
                            build_aec_slot = slot;
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
                    .unwrap_or_else(|| default_cache_dir.clone());

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

                // Reuse the build()-created slot rather than minting a new one.
                // The playback engine's `play_samples_callback` was wired with
                // clones of *that* slot, so STT respawns via apply_provider_spec
                // must update the same shared inner sender — otherwise the
                // post-respawn playback would publish render references to a
                // dropped channel and AEC would go silent.
                #[cfg(all(feature = "apple-stt", feature = "host-playback"))]
                let aec_render_slot = build_aec_slot;

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
                                    &mut rt, &mut sidecar, &mut ctx, &spec,
                                )
                            };
                            // Persist the new spec to marginalia.toml so
                            // the voice (and any other updated field)
                            // survives app restarts. Without this the
                            // in-memory swap works for the current
                            // session but the next launch re-reads the
                            // old TOML and reverts the user's pick —
                            // manifesting as "I selected im_nicola but
                            // a female voice plays after restart".
                            // Only save on success: a failed apply has
                            // left ctx partially mutated (e.g. voice
                            // build failed so synthesizer is still the
                            // old one), so writing would persist state
                            // that diverges from what's actually
                            // loaded. Save errors are logged but don't
                            // fail the reply — the in-memory apply is
                            // still valid for the current session.
                            if res.is_ok() {
                                if let Err(e) = ctx.save() {
                                    log::warn!(
                                        "[apply] ctx.save() failed — spec won't survive restart: {e}"
                                    );
                                }
                            }
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

        // Voice command monitor — drains the STT helper's CMD output
        // and converts each match into a `CommandRecognized` event the
        // Swift host already handles via `dispatchVoiceAction`. Without
        // this thread the Apple helper's CMD lines pile up in `cmd_rx`
        // forever, unread, and "pausa"/"avanti"/etc. never fire even
        // though the recognizer prints them to stdout. The TUI has the
        // same loop in `backend.rs`; this is the FFI's port of it.
        // Lives for the entire FFIRuntime lifetime — no graceful
        // shutdown plumbed yet, but the OS reaps the thread on exit.
        let monitor_event_buf = event_buffer.clone();
        let monitor = {
            let mut rt = init.runtime.lock().unwrap();
            rt.open_command_monitor()
        };
        std::thread::Builder::new()
            .name("marginalia-cmd-monitor".into())
            .spawn(move || {
                let mut monitor = monitor;
                loop {
                    let capture = monitor.capture_next_interrupt(Some(2.0));
                    if let Some(raw) = &capture.raw_text {
                        if raw.starts_with("error:") {
                            log::warn!("[cmd-monitor] {raw}");
                            std::thread::sleep(std::time::Duration::from_secs(5));
                            continue;
                        }
                    }
                    let raw = capture.raw_text.filter(|t| !t.is_empty());
                    let cmd = capture.recognized_command;
                    if raw.is_some() || cmd.is_some() {
                        monitor_event_buf.push(FfiRuntimeEvent::CommandRecognized {
                            raw_text: raw.unwrap_or_default(),
                            command: cmd,
                        });
                    } else {
                        // No-op cycle: small sleep so a non-blocking
                        // recognizer (Fake, or any that returns
                        // immediately) doesn't burn a core. The Apple
                        // path naturally paces itself via
                        // `recv_timeout(2s)`.
                        std::thread::sleep(std::time::Duration::from_millis(200));
                    }
                }
            })
            .ok();

        Ok(Self {
            runtime: init.runtime,
            discovery,
            cmd_tx,
            event_buffer,
            install_buffer: InstallBuffer::default(),
            whisper_path_cached,
            config_path,
            db_path,
            #[cfg(feature = "apple-stt")]
            waveform_handle: init.waveform_handle,
            #[cfg(all(feature = "apple-stt", feature = "host-playback"))]
            aec_render_slot: init.aec_render_slot,
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
    /// Snapshot of the current voice-command bindings. Re-reads
    /// `marginalia.toml` so we always return what's on disk (incl.
    /// changes made by `save_config` during the session). Empty list is
    /// returned on read failure — callers render the default Italian
    /// command set when they see zero rows.
    pub fn list_voice_commands(&self) -> Vec<VoiceCommandEntry> {
        let Ok(cfg) = AppConfig::load_from(&self.config_path) else {
            return Vec::new();
        };
        voice_commands_section_to_entries(&cfg.voice_commands)
    }

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

    /// Re-ingest a document's source file from disk. Used when the user
    /// edits the source externally (the GUI flags the row's
    /// `needs_reload` via `list_documents`). Path-stable id scheme means
    /// notes / sessions / TTS cache stay attached to the same id.
    /// Returns the same shape as `ingest_file` so the GUI handler can
    /// reuse its existing post-import flow.
    pub fn reload_document(&self, document_id: String) -> Result<IngestResult, FfiError> {
        log::info!(
            "[ffi] reload_document doc={document_id} thread={:?}",
            std::thread::current().id()
        );
        let mut rt = self.runtime.lock().unwrap();
        match rt.reload_document(&document_id) {
            Ok(outcome) => Ok(ingest_outcome_to_result(outcome)),
            Err(e) => Err(FfiError::Ingestion(format!("{e:?}"))),
        }
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
        log::info!(
            "[ffi] start_session doc={document_id} thread={:?}",
            std::thread::current().id()
        );
        self.runtime.lock().unwrap().start_session(&document_id)?;
        Ok(())
    }

    pub fn restore_session(&self) -> Result<bool, FfiError> {
        Ok(self.runtime.lock().unwrap().restore_session().is_some())
    }

    pub fn pause_session(&self) -> Result<(), FfiError> {
        log::info!(
            "[ffi] pause_session thread={:?}",
            std::thread::current().id()
        );
        self.runtime.lock().unwrap().pause_session()?;
        Ok(())
    }

    pub fn resume_session(&self) -> Result<(), FfiError> {
        log::info!(
            "[ffi] resume_session thread={:?}",
            std::thread::current().id()
        );
        self.runtime.lock().unwrap().resume_session()?;
        Ok(())
    }

    pub fn stop_session(&self) -> Result<(), FfiError> {
        log::info!(
            "[ffi] stop_session thread={:?}",
            std::thread::current().id()
        );
        self.runtime.lock().unwrap().stop_session()?;
        Ok(())
    }

    // ─────────────────────────────────────────────────────────
    // Playback navigation
    // ─────────────────────────────────────────────────────────

    pub fn next_chunk(&self) -> Result<(), FfiError> {
        log::info!("[ffi] next_chunk thread={:?}", std::thread::current().id());
        self.runtime.lock().unwrap().next_chunk()?;
        Ok(())
    }
    pub fn previous_chunk(&self) -> Result<(), FfiError> {
        log::info!(
            "[ffi] previous_chunk thread={:?}",
            std::thread::current().id()
        );
        self.runtime.lock().unwrap().previous_chunk()?;
        Ok(())
    }
    pub fn next_chapter(&self) -> Result<(), FfiError> {
        log::info!(
            "[ffi] next_chapter thread={:?}",
            std::thread::current().id()
        );
        self.runtime.lock().unwrap().next_chapter()?;
        Ok(())
    }
    pub fn previous_chapter(&self) -> Result<(), FfiError> {
        log::info!(
            "[ffi] previous_chapter thread={:?}",
            std::thread::current().id()
        );
        self.runtime.lock().unwrap().previous_chapter()?;
        Ok(())
    }
    pub fn restart_chapter(&self) -> Result<(), FfiError> {
        log::info!(
            "[ffi] restart_chapter thread={:?}",
            std::thread::current().id()
        );
        self.runtime.lock().unwrap().restart_chapter()?;
        Ok(())
    }
    pub fn repeat_chunk(&self) -> Result<(), FfiError> {
        log::info!(
            "[ffi] repeat_chunk thread={:?}",
            std::thread::current().id()
        );
        self.runtime.lock().unwrap().repeat_chunk()?;
        Ok(())
    }
    /// Click-to-seek: jump to `(section, chunk)` in the active document.
    pub fn seek_to_chunk(&self, section_index: u32, chunk_index: u32) -> Result<(), FfiError> {
        self.runtime
            .lock()
            .unwrap()
            .seek_to_chunk(section_index as usize, chunk_index as usize)?;
        Ok(())
    }

    /// Variant used by single-click-on-note: seek to the position but
    /// keep playback paused. The runtime's `seek_to_chunk` always auto-
    /// plays (it routes through `replay_session_at_position` which
    /// drives `playback_engine.start` → rodio's sink begins playing as
    /// soon as the source is appended). For "review silently" semantics
    /// we follow up with `pause_session` under the same lock so there
    /// is no interleaved event window. The cached audio is still
    /// loaded, so a subsequent `resume` is instant.
    pub fn seek_to_chunk_paused(
        &self,
        section_index: u32,
        chunk_index: u32,
    ) -> Result<(), FfiError> {
        let mut rt = self.runtime.lock().unwrap();
        rt.seek_to_chunk(section_index as usize, chunk_index as usize)?;
        rt.pause_session()?;
        Ok(())
    }
    pub fn auto_advance(&self) -> bool {
        self.runtime.lock().unwrap().try_auto_advance()
    }

    /// Spawn a background thread that pre-synthesizes the next chunk
    /// into the TTS cache. Returns immediately — the thread sleeps 100 ms
    /// so the current command's UI refresh lands first, then acquires
    /// the runtime lock and calls `prefetch_next()`. On a cache hit this
    /// is a fast no-op; on a miss it runs full synthesis (1–2 s) while
    /// the user is listening to the current chunk. Subsequent "next"
    /// then hits the cache and plays instantly.
    ///
    /// CLAUDE.md calls out that prefetch MUST live on its own thread —
    /// running it on the command thread froze the UI for ~2 s. Callers
    /// in Swift just fire this after every navigation command; no need
    /// to await anything.
    pub fn prefetch_next(&self) {
        let rt = self.runtime.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(100));
            if let Ok(mut r) = rt.lock() {
                r.prefetch_next();
            }
        });
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
                        error_message: Some("dettatura vuota o non riconosciuta".to_string()),
                    });
                    return;
                }

                // 4. Re-lock runtime to persist the note at the current
                // reading position. `create_note` emits its own note
                // repository activity; we wrap with our event so the UI
                // can light the live-note card in one place.
                // Attach the AEC-recorded WAV path (Apple STT only).
                // Other transcribers leave `raw_audio_path` as `None`,
                // which the note playback path interprets as
                // "fall back to TTS synthesis of the transcript".
                let audio_path = transcript.raw_audio_path.clone();
                let create_result = {
                    let mut rt = runtime.lock().unwrap();
                    rt.create_note(&text, audio_path)
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
        let note = self.runtime.lock().unwrap().create_note(&text, None)?;
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
    pub fn update_note(&self, note_id: String, new_text: String) -> Result<NoteView, FfiError> {
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

    /// Latest partial dictation transcript exposed by the active STT
    /// provider. Empty when nothing is being dictated. See
    /// `SqliteRuntime::dictation_partial` for the side-channel
    /// rationale. Caller polls this on its event tick.
    pub fn dictation_partial(&self) -> String {
        self.runtime.lock().unwrap().dictation_partial()
    }

    /// Feed the AEC render reference from a WAV/FLAC file the GUI is
    /// about to play through a pipeline that BYPASSES the rodio host
    /// engine (currently: Swift `AVAudioPlayer` for note playback).
    ///
    /// Without this, the speaker echo of a note would reach the mic
    /// uncancelled and SFSpeechRecognizer would happily trigger any
    /// command words inside the note body — e.g. a note containing
    /// "nota" would auto-fire the dictation flow on the next playback.
    ///
    /// The audio is decoded, downmixed to mono `f32`, and shipped as
    /// the full reference buffer (matching how chunk playback feeds
    /// AEC at `start()`). On builds without apple-stt+host-playback
    /// this is a silent no-op. Returns `false` if the file couldn't
    /// be opened/decoded so the caller can decide whether to suppress
    /// commands as a fallback.
    pub fn aec_set_render_reference(&self, _path: String) -> bool {
        #[cfg(all(feature = "apple-stt", feature = "host-playback"))]
        {
            let Ok(reader) = hound::WavReader::open(&_path) else {
                log::warn!("[aec] cannot open WAV at {_path} for render reference");
                return false;
            };
            let spec = reader.spec();
            let channels = spec.channels.max(1) as usize;
            let samples_f32: Vec<f32> = match spec.sample_format {
                hound::SampleFormat::Int => {
                    let bits = spec.bits_per_sample.max(1) as i32;
                    let scale = (1i32 << (bits - 1)) as f32;
                    let mut iter = reader.into_samples::<i32>();
                    let mut out: Vec<f32> = Vec::new();
                    'outer: loop {
                        // Take channels-worth of samples; keep first channel only.
                        let Some(first) = iter.next() else {
                            break 'outer;
                        };
                        let Ok(v) = first else { break 'outer };
                        out.push(v as f32 / scale);
                        for _ in 1..channels {
                            if iter.next().is_none() {
                                break 'outer;
                            }
                        }
                    }
                    out
                }
                hound::SampleFormat::Float => {
                    let mut iter = reader.into_samples::<f32>();
                    let mut out: Vec<f32> = Vec::new();
                    'outer: loop {
                        let Some(first) = iter.next() else {
                            break 'outer;
                        };
                        let Ok(v) = first else { break 'outer };
                        out.push(v);
                        for _ in 1..channels {
                            if iter.next().is_none() {
                                break 'outer;
                            }
                        }
                    }
                    out
                }
            };
            // The AEC pipeline expects 24kHz mono (matches
            // marginalia_stt_apple::AEC_SAMPLE_RATE). Both note sources
            // hit that natively: MLX TTS writes 24k WAV, the Apple
            // dictation recorder also writes 24k via hound. If a future
            // source drifts off-rate, downsample here — for now keep
            // the path tight and warn so the mismatch surfaces.
            const AEC_RATE: u32 = 24_000;
            if spec.sample_rate != AEC_RATE {
                log::warn!(
                    "[aec] render reference sample rate {} != {}, AEC alignment may suffer",
                    spec.sample_rate,
                    AEC_RATE
                );
            }
            self.aec_render_slot.send_set_reference(samples_f32);
            return true;
        }
        #[cfg(not(all(feature = "apple-stt", feature = "host-playback")))]
        {
            false
        }
    }

    /// Clear the AEC render reference. Call this when the GUI-side
    /// player stops (user tapped stop, audio drained naturally, note
    /// got deleted mid-playback). No-op when no reference is loaded.
    pub fn aec_clear_render_reference(&self) {
        #[cfg(all(feature = "apple-stt", feature = "host-playback"))]
        {
            self.aec_render_slot.send_clear();
        }
    }

    /// Absolute path of the TTS audio cache directory (where the
    /// synthesizer writes its FLAC files). Used by Settings to display
    /// the live size and offer a "reveal in Finder" affordance.
    pub fn tts_cache_dir(&self) -> String {
        let rt = self.runtime.lock().unwrap();
        rt.config()
            .tts_cache_dir
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_default()
    }

    /// Read the configured Whisper model path. Empty string = unset.
    pub fn whisper_model_path(&self) -> String {
        self.whisper_path_cached
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_default()
    }

    /// Absolute path of the SQLite library (resolved from
    /// `marginalia.toml`'s `database_path` against the config's parent
    /// directory). Used by Settings to reveal the file in Finder.
    pub fn database_path(&self) -> String {
        self.db_path.display().to_string()
    }

    /// Absolute path of the directory that holds recorded voice-note
    /// audio (`<marginalia-data>/notes-audio/`). Mirrors the derivation
    /// used by `builder.rs` and `reconfigure.rs` so the UI agrees with
    /// where the STT helper actually writes WAVs.
    pub fn notes_audio_dir(&self) -> String {
        let rt = self.runtime.lock().unwrap();
        let Some(cache_dir) = rt.config().tts_cache_dir.as_ref() else {
            return String::new();
        };
        let parent = cache_dir.parent().unwrap_or(cache_dir.as_path());
        parent.join("notes-audio").display().to_string()
    }

    /// Absolute path of the active `marginalia.toml`. The Settings page
    /// shows this in the sub-nav footer so the user knows exactly which
    /// file the running app reads from.
    pub fn config_path(&self) -> String {
        self.config_path.display().to_string()
    }

    /// Empty the on-disk TTS cache directory and the runtime's in-memory
    /// cache map. Returns the number of bytes freed. Walks
    /// `tts_cache_dir` and removes regular files; subdirectories (none
    /// today, but be defensive) are left untouched. The next chunk
    /// request synthesizes from scratch.
    pub fn clear_tts_cache(&self) -> u64 {
        let cache_dir = {
            let rt = self.runtime.lock().unwrap();
            rt.config().tts_cache_dir.clone()
        };
        let Some(dir) = cache_dir else {
            log::info!("[ffi] clear_tts_cache: no cache dir configured");
            return 0;
        };
        let mut bytes_freed: u64 = 0;
        match std::fs::read_dir(&dir) {
            Ok(entries) => {
                for entry in entries.flatten() {
                    let path = entry.path();
                    let Ok(meta) = entry.metadata() else { continue };
                    if !meta.is_file() {
                        continue;
                    }
                    let size = meta.len();
                    match std::fs::remove_file(&path) {
                        Ok(_) => bytes_freed = bytes_freed.saturating_add(size),
                        Err(e) => log::warn!(
                            "[ffi] clear_tts_cache: failed to remove {}: {e}",
                            path.display()
                        ),
                    }
                }
            }
            Err(e) => {
                log::warn!(
                    "[ffi] clear_tts_cache: read_dir({}) failed: {e}",
                    dir.display()
                );
                return 0;
            }
        }
        // Drop the in-memory map too — otherwise `synthesize_cached`'s
        // step-1 hot path would still hand back deleted file paths.
        self.runtime.lock().unwrap().clear_tts_cache_memory();
        log::info!(
            "[ffi] clear_tts_cache: freed {} bytes from {}",
            bytes_freed,
            dir.display()
        );
        bytes_freed
    }

    /// Wipe **every** voice note (audio + DB row) from the system. The
    /// runtime first deletes referenced WAVs from disk, then truncates
    /// the `notes` table; any orphan files left in `notes_audio_dir`
    /// (recordings whose row was lost in an earlier crash) are swept
    /// here too so the directory ends up empty. Returns
    /// `(notes_deleted, bytes_freed)` so the UI can show a precise
    /// toast. Destructive — the caller is responsible for confirming
    /// with the user before invoking.
    pub fn clear_all_notes(&self) -> Result<ClearNotesReport, FfiError> {
        let (count, mut bytes_freed) = self
            .runtime
            .lock()
            .unwrap()
            .clear_all_notes()
            .map_err(|e| FfiError::Runtime(e.to_string()))?;
        // Sweep any orphan files left in notes_audio_dir.
        let notes_dir = {
            let rt = self.runtime.lock().unwrap();
            rt.config().tts_cache_dir.as_ref().map(|cache| {
                cache
                    .parent()
                    .unwrap_or(cache.as_path())
                    .join("notes-audio")
            })
        };
        if let Some(dir) = notes_dir {
            if let Ok(entries) = std::fs::read_dir(&dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    let Ok(meta) = entry.metadata() else { continue };
                    if !meta.is_file() {
                        continue;
                    }
                    let size = meta.len();
                    match std::fs::remove_file(&path) {
                        Ok(_) => bytes_freed = bytes_freed.saturating_add(size),
                        Err(e) => log::warn!(
                            "[ffi] clear_all_notes orphan sweep: failed to remove {}: {e}",
                            path.display()
                        ),
                    }
                }
            }
        }
        log::info!("[ffi] clear_all_notes: deleted {count} notes, freed {bytes_freed} bytes total");
        Ok(ClearNotesReport {
            notes_deleted: count as u32,
            bytes_freed,
        })
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
        current_catalog()
            .iter()
            .map(InstallableAsset::from_spec)
            .collect()
    }

    /// Refresh the voice catalog from huggingface.co. **One of the very
    /// few network calls in Marginalia** — only fired when the user
    /// explicitly asks for it (onboarding `installModels` step or the
    /// "Aggiorna lista voci" button in Settings). Writes the result to
    /// `<config_dir>/voices.cache.json`; the next `list_installable_assets`
    /// call will pick it up. Returns the number of voices fetched.
    pub fn fetch_remote_voice_catalog(&self) -> Result<u32, FfiError> {
        let count = fetch_remote_voice_catalog_inner().map_err(FfiError::Io)?;
        // The catalog snapshot was invalidated inside `_inner`. Rebuild
        // the runtime's `voice_to_lang` map from the fresh catalog so a
        // newly-fetched voice (e.g. the user just clicked "aggiorna
        // lista" and Kokoro published a new language) immediately gets
        // the right language when the user picks it.
        if let Ok(mut rt) = self.runtime.lock() {
            rt.set_voice_to_lang_map(build_voice_to_lang_map());
        }
        Ok(count)
    }

    /// Kick off an asynchronous download for the named asset. Returns
    /// immediately; progress is reported through `install_progress()`.
    /// The background thread temporarily disables `HF_HUB_OFFLINE`
    /// (which the runtime otherwise forces to `1`), downloads via
    /// `marginalia-models`, then restores the flag.
    pub fn install_asset(&self, asset_id: String) -> Result<(), FfiError> {
        let catalog = current_catalog();
        let spec = catalog
            .iter()
            .find(|s| s.id == asset_id)
            .ok_or_else(|| FfiError::Io(format!("unknown asset '{asset_id}'")))?;
        let buf = self.install_buffer.clone();
        let source = spec.source.clone();
        let id = asset_id.clone();

        // If the config's `[mlx] model` is an absolute filesystem path,
        // mirror downloaded weights into it after the hf-hub download so
        // `Discovery::list_voices` (which scans `{mlx_model_dir}/voices/`)
        // can see them. The TUI's relative path gets treated as a HF
        // repo id here and the copy is skipped — the TUI uses
        // `make bootstrap-mlx` to populate that directory instead.
        let mlx_mirror_dir: Option<PathBuf> = AppConfig::load_from(&self.config_path)
            .ok()
            .and_then(|cfg| {
                let p = PathBuf::from(&cfg.mlx.model);
                p.is_absolute().then_some(p)
            });

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
                    Ok(cached_path) => {
                        // Sync into `mlx_model_dir` if the config asks
                        // for a local dir. Failures here are non-fatal
                        // — the TTS runtime can still find the file in
                        // the HF cache via hf-hub. Discovery just stays
                        // blind until the copy succeeds on a retry.
                        if let Some(ref dir) = mlx_mirror_dir {
                            if let Err(e) = mirror_to_mlx_dir(&cached_path, &source, dir) {
                                log::warn!(
                                    "[install] mirror to {} failed: {e}",
                                    dir.display()
                                );
                            }
                        }
                        buf.push(InstallProgress {
                            asset_id: id,
                            state: "installed".into(),
                            bytes_done: 0,
                            bytes_total: 0,
                            error_message: None,
                        })
                    }
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
        let catalog = current_catalog();
        let spec = catalog
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
            AssetSource::KokoroOnnx { file } => {
                ("onnx-community/Kokoro-82M-v1.0-ONNX", file.to_string())
            }
        };
        marginalia_models::ModelManager::uninstall_from_repo(repo, &file)
            .map_err(|e| FfiError::Io(e.to_string()))?;

        // Also drop the mirror in `mlx_model_dir` if we wrote one during
        // install — otherwise Discovery would keep listing the voice
        // because `list_voices` scans that directory.
        if let Ok(cfg) = AppConfig::load_from(&self.config_path) {
            let p = PathBuf::from(&cfg.mlx.model);
            if p.is_absolute() {
                let mirror = match &spec.source {
                    AssetSource::MlxCore { file } => Some(p.join(file)),
                    AssetSource::MlxVoice { voice_id } => {
                        Some(p.join("voices").join(format!("{voice_id}.safetensors")))
                    }
                    _ => None,
                };
                if let Some(mirror) = mirror {
                    let _ = std::fs::remove_file(&mirror);
                }
            }
        }
        Ok(())
    }

    // ─────────────────────────────────────────────────────────
    // Backup / restore (B9)
    //
    // Export zips the config TOML + sqlite DB + voices manifest into a
    // single archive the user can stash on iCloud or elsewhere. Import
    // reverses it; because the sqlite connection is open while the app
    // runs, importing requires the user to restart — we write the files
    // but don't hot-reload the runtime. The Swift side shows an
    // NSAlert telling them to quit + reopen.
    //
    // Explicitly NOT included in the archive: TTS cache (re-generatable,
    // large), downloaded model weights (user-controlled via Installa-
    // zioni), or the STT helper binary (re-compiled from source).
    // ─────────────────────────────────────────────────────────

    /// Write a backup zip at `out_path`. Overwrites if the file exists.
    pub fn export_backup(&self, out_path: String) -> Result<(), FfiError> {
        let out = PathBuf::from(&out_path);
        // Collect the paths we care about. Missing ones are skipped
        // with a log warning — not every install has every file (e.g.
        // first-run may not have a voices manifest yet).
        let mlx_manifest = PathBuf::from("models/tts/mlx/voices.manifest.json");
        let files: Vec<(PathBuf, &str)> = [
            (self.config_path.clone(), "marginalia.toml"),
            (self.db_path.clone(), "beta.sqlite3"),
            (mlx_manifest, "voices.manifest.json"),
        ]
        .into_iter()
        .filter(|(p, _)| p.is_file())
        .collect();

        let file =
            std::fs::File::create(&out).map_err(|e| FfiError::Io(format!("create backup: {e}")))?;
        let mut zip = zip::ZipWriter::new(file);
        let opts: zip::write::SimpleFileOptions = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .unix_permissions(0o644);

        for (src, archive_name) in files {
            let bytes = std::fs::read(&src)
                .map_err(|e| FfiError::Io(format!("read {}: {e}", src.display())))?;
            zip.start_file(archive_name, opts)
                .map_err(|e| FfiError::Io(format!("zip start {archive_name}: {e}")))?;
            std::io::Write::write_all(&mut zip, &bytes)
                .map_err(|e| FfiError::Io(format!("zip write {archive_name}: {e}")))?;
        }

        zip.finish()
            .map_err(|e| FfiError::Io(format!("zip finalize: {e}")))?;
        log::info!("[ffi] backup written to {}", out.display());
        Ok(())
    }

    /// Restore a backup zip at `src_path`. Writes config + sqlite back
    /// to their expected locations, overwriting existing files. Does
    /// NOT hot-reload the runtime — the caller must restart the app.
    pub fn import_backup(&self, src_path: String) -> Result<(), FfiError> {
        let src = PathBuf::from(&src_path);
        let file =
            std::fs::File::open(&src).map_err(|e| FfiError::Io(format!("open backup: {e}")))?;
        let mut archive =
            zip::ZipArchive::new(file).map_err(|e| FfiError::Io(format!("read zip: {e}")))?;

        // Map archive entries back to destination paths. Unknown names
        // are skipped (forward-compat when future backups include extra
        // files).
        let mlx_manifest = PathBuf::from("models/tts/mlx/voices.manifest.json");
        for i in 0..archive.len() {
            let mut entry = archive
                .by_index(i)
                .map_err(|e| FfiError::Io(format!("zip entry {i}: {e}")))?;
            let name = entry.name().to_string();
            let dest: Option<PathBuf> = match name.as_str() {
                "marginalia.toml" => Some(self.config_path.clone()),
                "beta.sqlite3" => Some(self.db_path.clone()),
                "voices.manifest.json" => Some(mlx_manifest.clone()),
                _ => {
                    log::warn!("[ffi] backup contains unknown entry '{name}', skipping");
                    None
                }
            };
            let Some(dest) = dest else { continue };
            if let Some(parent) = dest.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let mut out = std::fs::File::create(&dest)
                .map_err(|e| FfiError::Io(format!("create {}: {e}", dest.display())))?;
            std::io::copy(&mut entry, &mut out)
                .map_err(|e| FfiError::Io(format!("write {}: {e}", dest.display())))?;
        }
        log::info!("[ffi] backup restored from {}", src.display());
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

/// Inverse of `voice_commands_entries_to_section` — project the 11
/// fixed fields of the config section into the flat FFI shape
/// (action, triggers). Stable order so the UI row ordering matches
/// the config file.
fn voice_commands_section_to_entries(
    section: &marginalia_config::VoiceCommandsSection,
) -> Vec<VoiceCommandEntry> {
    vec![
        VoiceCommandEntry {
            action: "pause".into(),
            triggers: section.pause.clone(),
        },
        VoiceCommandEntry {
            action: "resume".into(),
            triggers: section.resume.clone(),
        },
        VoiceCommandEntry {
            action: "next".into(),
            triggers: section.next.clone(),
        },
        VoiceCommandEntry {
            action: "back".into(),
            triggers: section.back.clone(),
        },
        VoiceCommandEntry {
            action: "repeat".into(),
            triggers: section.repeat.clone(),
        },
        VoiceCommandEntry {
            action: "stop".into(),
            triggers: section.stop.clone(),
        },
        VoiceCommandEntry {
            action: "next_chapter".into(),
            triggers: section.next_chapter.clone(),
        },
        VoiceCommandEntry {
            action: "prev_chapter".into(),
            triggers: section.prev_chapter.clone(),
        },
        VoiceCommandEntry {
            action: "bookmark".into(),
            triggers: section.bookmark.clone(),
        },
        VoiceCommandEntry {
            action: "note".into(),
            triggers: section.note.clone(),
        },
        VoiceCommandEntry {
            action: "where".into(),
            triggers: section.r#where.clone(),
        },
    ]
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
