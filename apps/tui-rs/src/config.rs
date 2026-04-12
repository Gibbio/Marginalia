use serde::Deserialize;
use std::path::PathBuf;

/// Runtime configuration for marginalia-tui.
/// Loaded from `apps/tui-rs/marginalia.toml` by default, or from
/// the path specified in the `MARGINALIA_CONFIG` environment variable.
/// All paths are relative to the working directory (repo root when using `make tui-rs`).
#[derive(Debug, Deserialize, Default)]
pub struct TuiConfig {
    /// Path to the SQLite database. Default: `.marginalia/beta.sqlite3`.
    pub database_path: Option<PathBuf>,
    /// Target characters per chunk when splitting imported documents.
    /// Lower values = shorter TTS utterances (faster response on slow TTS).
    /// Higher values = fewer chunks, less navigation overhead.
    /// Default: 300.
    pub chunk_target_chars: Option<usize>,
    /// Trigger words mapped to actions (`pause`, `next`, etc.).
    #[serde(default)]
    pub voice_commands: VoiceCommandsSection,
    /// Speech-to-text engine settings. The chosen engine handles both command
    /// recognition and dictation; per-context tuning lives in `[stt.commands]`
    /// and `[stt.dictation]`.
    #[serde(default)]
    pub stt: SttSection,
    #[serde(default)]
    pub kokoro: KokoroSection,
    #[serde(default)]
    pub playback: PlaybackSection,
    #[serde(default)]
    #[cfg_attr(not(feature = "mlx-tts"), allow(dead_code))]
    pub mlx: MlxSection,
}

/// STT configuration root.
///
/// Layout:
/// - `engine`, `language`, `debug` — global engine selection and shared options.
/// - `[stt.whisper]` / `[stt.apple]` — engine-specific settings (only the
///   chosen engine is read; the other section is ignored).
/// - `[stt.commands]` — tuning for short-utterance command recognition.
/// - `[stt.dictation]` — tuning for long-utterance note dictation.
#[derive(Debug, Deserialize, Default)]
pub struct SttSection {
    /// Engine choice: `"apple"` or `"whisper"`. Default: `"whisper"`.
    #[serde(default = "default_stt_engine")]
    pub engine: String,
    /// Recognition language. Whisper expects ISO ("it"), Apple expects BCP-47
    /// ("it-IT"); the backend normalizes between the two. Default: `"it"`.
    pub language: Option<String>,
    /// Show raw transcript in the Log pane.
    #[serde(default)]
    pub debug: bool,
    /// Apple-engine settings (placeholder for future apple-only options).
    #[serde(default)]
    #[allow(dead_code)] // currently empty; reserved for future apple-only fields
    pub apple: AppleEngineSection,
    /// Whisper-engine settings (model file path).
    #[serde(default)]
    #[cfg_attr(not(feature = "whisper-stt"), allow(dead_code))]
    pub whisper: WhisperEngineSection,
    /// Tuning profile applied when recognizing voice commands (short utterances).
    #[serde(default)]
    pub commands: SttContextSection,
    /// Tuning profile applied when transcribing dictated notes (long utterances).
    #[serde(default)]
    #[cfg_attr(not(feature = "whisper-stt"), allow(dead_code))]
    pub dictation: SttContextSection,
}

fn default_stt_engine() -> String {
    "whisper".to_string()
}

/// Apple-engine settings. Currently empty; reserved for future apple-only
/// options (e.g. on-device requirement, custom locale, etc.).
#[derive(Debug, Deserialize, Default)]
#[cfg_attr(not(feature = "apple-stt"), allow(dead_code))]
pub struct AppleEngineSection {}

#[derive(Debug, Deserialize, Default)]
#[cfg_attr(not(feature = "whisper-stt"), allow(dead_code))]
pub struct WhisperEngineSection {
    /// Path to the Whisper ggml model file (e.g. `ggml-small.bin`).
    pub model_path: Option<PathBuf>,
}

/// Per-context tuning. Each context (commands / dictation) gets its own values
/// for the same parameter set. Backend interpretation depends on the engine.
#[derive(Debug, Deserialize, Default)]
#[allow(dead_code)] // some fields are read only when a given engine is enabled
pub struct SttContextSection {
    /// Seconds of silence after speech before emitting/finalizing.
    /// Default: 0.8 (commands) / 1.5 (dictation).
    pub silence_timeout: Option<f64>,
    /// Maximum recording duration in seconds (Whisper only).
    /// Default: 4 (commands) / 60 (dictation).
    pub max_record_seconds: Option<f64>,
    /// Minimum RMS amplitude (0-32767) considered as speech (Whisper only).
    /// Default: 500.
    pub speech_threshold: Option<i16>,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(not(feature = "mlx-tts"), allow(dead_code))]
pub struct MlxSection {
    /// HuggingFace model repo or local path. Default: `prince-canuma/Kokoro-82M`.
    #[serde(default = "default_mlx_model")]
    pub model: String,
    /// Voice preset name. Default: `af_bella`.
    #[serde(default = "default_mlx_voice")]
    pub voice: String,
}

impl Default for MlxSection {
    fn default() -> Self {
        Self {
            model: default_mlx_model(),
            voice: default_mlx_voice(),
        }
    }
}

fn default_mlx_model() -> String {
    "prince-canuma/Kokoro-82M".to_string()
}
fn default_mlx_voice() -> String {
    "af_bella".to_string()
}

#[derive(Debug, Deserialize, Default)]
pub struct KokoroSection {
    /// Directory containing `kokoro.onnx`, `config.json`, `voices/`, and `lib/`.
    pub assets_root: Option<PathBuf>,
    /// Directory where synthesised WAV files are cached.
    /// Default: `<database_dir>/.marginalia-tts-cache/`.
    pub tts_cache_dir: Option<PathBuf>,
    /// External phonemizer program (e.g. `espeak-ng`).
    /// When set, plain text is piped through this command for G2P conversion.
    pub phonemizer_program: Option<String>,
    /// Arguments for the phonemizer program.
    /// Default for espeak-ng: `["-v", "it", "--ipa", "-q"]`.
    #[serde(default)]
    pub phonemizer_args: Vec<String>,
}

/// Maps actions to trigger words. The STT backend listens for all words;
/// when one is recognized, the corresponding action is executed.
/// Users can add synonyms in any language.
#[derive(Debug, Deserialize)]
pub struct VoiceCommandsSection {
    /// Words that pause playback. Default: ["pausa", "pause"]
    #[serde(default = "default_pause")]
    pub pause: Vec<String>,
    /// Words that advance to next chunk. Default: ["avanti", "next"]
    #[serde(default = "default_next")]
    pub next: Vec<String>,
    /// Words that go back one chunk. Default: ["indietro", "back"]
    #[serde(default = "default_back")]
    pub back: Vec<String>,
    /// Words that stop the session. Default: ["stop"]
    #[serde(default = "default_stop")]
    pub stop: Vec<String>,
    /// Words that repeat current chunk. Default: ["ripeti", "repeat"]
    #[serde(default = "default_repeat")]
    pub repeat: Vec<String>,
    /// Words that resume playback. Default: ["riprendi", "resume"]
    #[serde(default = "default_resume")]
    pub resume: Vec<String>,
    /// Words that jump to next chapter. Default: ["capitolo", "prossimo capitolo"]
    #[serde(default = "default_next_chapter")]
    pub next_chapter: Vec<String>,
    /// Words that jump to previous chapter. Default: ["capitolo indietro", "capitolo precedente"]
    #[serde(default = "default_prev_chapter")]
    pub prev_chapter: Vec<String>,
    /// Words that create a bookmark note. Default: ["segna", "segnalibro"]
    #[serde(default = "default_bookmark")]
    pub bookmark: Vec<String>,
    /// Words that start note dictation. Default: ["nota", "appunto"]
    #[serde(default = "default_note")]
    pub note: Vec<String>,
    /// Words that report current position. Default: ["dove sono", "posizione"]
    #[serde(default = "default_where")]
    pub r#where: Vec<String>,
}

impl Default for VoiceCommandsSection {
    fn default() -> Self {
        Self {
            pause: default_pause(),
            next: default_next(),
            back: default_back(),
            stop: default_stop(),
            repeat: default_repeat(),
            resume: default_resume(),
            next_chapter: default_next_chapter(),
            prev_chapter: default_prev_chapter(),
            bookmark: default_bookmark(),
            note: default_note(),
            r#where: default_where(),
        }
    }
}

impl VoiceCommandsSection {
    /// Flat list of all trigger words (for the STT backend).
    pub fn all_words(&self) -> Vec<String> {
        let mut words = Vec::new();
        for list in [
            &self.pause,
            &self.next,
            &self.back,
            &self.stop,
            &self.repeat,
            &self.resume,
            &self.next_chapter,
            &self.prev_chapter,
            &self.bookmark,
            &self.note,
            &self.r#where,
        ] {
            words.extend(list.clone());
        }
        words
    }

    /// Map a recognized word back to an action name.
    /// Checks longer phrases first to avoid partial matches
    /// (e.g. "capitolo" matching before "capitolo indietro").
    pub fn resolve_action(&self, word: &str) -> Option<&'static str> {
        let w = word.to_lowercase();
        let checks: &[(&Vec<String>, &str)] = &[
            // Longer phrases first
            (&self.next_chapter, "next_chapter"),
            (&self.prev_chapter, "prev_chapter"),
            (&self.bookmark, "bookmark"),
            (&self.note, "note"),
            (&self.r#where, "where"),
            // Then single words
            (&self.pause, "pause"),
            (&self.next, "next"),
            (&self.back, "back"),
            (&self.stop, "stop"),
            (&self.repeat, "repeat"),
            (&self.resume, "resume"),
        ];
        for (triggers, action) in checks {
            if triggers.iter().any(|t| w.contains(&t.to_lowercase())) {
                return Some(action);
            }
        }
        None
    }
}

fn default_pause() -> Vec<String> {
    vec!["pausa".into(), "pause".into()]
}
fn default_next() -> Vec<String> {
    vec!["avanti".into(), "next".into()]
}
fn default_back() -> Vec<String> {
    vec!["indietro".into(), "back".into()]
}
fn default_stop() -> Vec<String> {
    vec!["stop".into()]
}
fn default_repeat() -> Vec<String> {
    vec!["ripeti".into(), "repeat".into()]
}
fn default_resume() -> Vec<String> {
    vec!["riprendi".into(), "resume".into()]
}
fn default_next_chapter() -> Vec<String> {
    vec!["prossimo capitolo".into(), "capitolo avanti".into()]
}
fn default_prev_chapter() -> Vec<String> {
    vec!["capitolo indietro".into(), "capitolo precedente".into()]
}
fn default_bookmark() -> Vec<String> {
    vec!["segna".into(), "segnalibro".into()]
}
fn default_note() -> Vec<String> {
    vec!["nota".into(), "appunto".into()]
}
fn default_where() -> Vec<String> {
    vec!["dove sono".into(), "posizione".into()]
}

#[derive(Debug, Deserialize, Default)]
pub struct PlaybackSection {
    /// Set to `true` to use the no-op fake playback engine (headless/CI environments).
    #[serde(default)]
    pub fake: bool,
}

impl TuiConfig {
    /// Load config from `MARGINALIA_CONFIG` env var path, or from
    /// `apps/tui-rs/marginalia.toml` relative to the current directory.
    /// Returns `Default` if the file is missing, with a warning if it fails to parse.
    pub fn load() -> Self {
        let path = std::env::var("MARGINALIA_CONFIG")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("apps/tui-rs/marginalia.toml"));

        if !path.exists() {
            return Self::default();
        }

        match std::fs::read_to_string(&path) {
            Err(e) => {
                eprintln!("warning: cannot read config {}: {e}", path.display());
                Self::default()
            }
            Ok(content) => match toml::from_str(&content) {
                Err(e) => {
                    eprintln!("warning: cannot parse config {}: {e}", path.display());
                    Self::default()
                }
                Ok(cfg) => cfg,
            },
        }
    }
}
