//! Shared configuration types for Marginalia applications.
//!
//! Each app (TUI, future GUI, mobile, web) defines its own top-level config
//! struct and its own loading logic (file path, env var, bundled defaults).
//! This crate provides the **reusable sections** so that the TOML schema is
//! consistent across all hosts:
//!
//! - [`VoiceCommandsSection`] — `[voice_commands]` trigger words
//! - [`SttSection`] — `[stt]` engine selection + per-context tuning
//! - [`KokoroSection`] — `[kokoro]` ONNX TTS config
//! - [`MlxSection`] — `[mlx]` MLX Metal TTS config
//! - [`PlaybackSection`] — `[playback]` options
//!
//! All types derive `Deserialize` + `Default` so they work seamlessly with
//! TOML (or any other serde format).

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

// =============================================================================
// STT
// =============================================================================

/// Root of the `[stt]` configuration tree.
///
/// Layout:
/// - `engine`, `language`, `debug` — global engine selection and shared options
/// - `[stt.whisper]` / `[stt.apple]` — engine-specific settings
/// - `[stt.commands]` — tuning for short-utterance command recognition
/// - `[stt.dictation]` — tuning for long-utterance note dictation
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct SttSection {
    /// Engine choice: `"apple"` or `"whisper"`. Default: `"whisper"`.
    #[serde(default = "default_stt_engine")]
    pub engine: String,
    /// Recognition language. Whisper expects ISO (`"it"`), Apple expects
    /// BCP-47 (`"it-IT"`); the backend normalizes between the two.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    /// Show raw STT transcript in the app's debug/log pane.
    #[serde(default)]
    pub debug: bool,
    /// Apple-engine settings (placeholder for future apple-only options).
    #[serde(default)]
    pub apple: AppleEngineSection,
    /// Whisper-engine settings (model file path).
    #[serde(default)]
    pub whisper: WhisperEngineSection,
    /// Tuning profile for short-utterance command recognition.
    #[serde(default)]
    pub commands: SttContextSection,
    /// Tuning profile for long-utterance note dictation.
    #[serde(default)]
    pub dictation: SttContextSection,
}

fn default_stt_engine() -> String {
    "whisper".to_string()
}

/// Apple-engine settings. Currently empty; reserved for future options
/// (e.g. on-device requirement, custom locale).
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct AppleEngineSection {}

/// Whisper-engine settings.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct WhisperEngineSection {
    /// Path to the Whisper ggml model file (e.g. `ggml-small.bin`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_path: Option<PathBuf>,
}

/// Per-context tuning applied on top of the chosen engine. Each context
/// (commands / dictation) gets its own values for the same parameter set.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct SttContextSection {
    /// Seconds of silence after speech before emitting/finalizing.
    /// Default: 0.8 (commands) / 1.5 (dictation).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub silence_timeout: Option<f64>,
    /// Maximum recording duration in seconds (Whisper only).
    /// Default: 4 (commands) / 60 (dictation).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_record_seconds: Option<f64>,
    /// Minimum RMS amplitude (0-32767) considered as speech (Whisper only).
    /// Default: 500.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speech_threshold: Option<i16>,
}

// =============================================================================
// Voice commands
// =============================================================================

/// Maps actions to trigger words (`[voice_commands]`). The STT backend
/// listens for all words; when one is recognized, the corresponding action
/// is executed. Users can add synonyms in any language.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VoiceCommandsSection {
    #[serde(default = "default_pause")]
    pub pause: Vec<String>,
    #[serde(default = "default_next")]
    pub next: Vec<String>,
    #[serde(default = "default_back")]
    pub back: Vec<String>,
    #[serde(default = "default_stop")]
    pub stop: Vec<String>,
    #[serde(default = "default_repeat")]
    pub repeat: Vec<String>,
    #[serde(default = "default_resume")]
    pub resume: Vec<String>,
    #[serde(default = "default_next_chapter")]
    pub next_chapter: Vec<String>,
    #[serde(default = "default_prev_chapter")]
    pub prev_chapter: Vec<String>,
    #[serde(default = "default_bookmark")]
    pub bookmark: Vec<String>,
    #[serde(default = "default_note")]
    pub note: Vec<String>,
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

    /// Map a recognized text back to an action name. Checks longer phrases
    /// first to avoid partial matches (e.g. "capitolo" matching before
    /// "capitolo indietro").
    pub fn resolve_action(&self, word: &str) -> Option<&'static str> {
        let w = word.to_lowercase();
        let checks: &[(&Vec<String>, &str)] = &[
            (&self.next_chapter, "next_chapter"),
            (&self.prev_chapter, "prev_chapter"),
            (&self.bookmark, "bookmark"),
            (&self.note, "note"),
            (&self.r#where, "where"),
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

// =============================================================================
// TTS
// =============================================================================

/// Kokoro ONNX TTS configuration (`[kokoro]`).
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct KokoroSection {
    /// Directory containing `kokoro.onnx`, `config.json`, `voices/`, and `lib/`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assets_root: Option<PathBuf>,
    /// Directory for synthesised WAV cache.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tts_cache_dir: Option<PathBuf>,
    /// External phonemizer program (e.g. `espeak-ng`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phonemizer_program: Option<String>,
    /// Arguments for the phonemizer program.
    #[serde(default)]
    pub phonemizer_args: Vec<String>,
}

/// Kokoro MLX Metal TTS configuration (`[mlx]`).
#[derive(Debug, Clone, Deserialize, Serialize)]
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

// =============================================================================
// Playback
// =============================================================================

/// Playback configuration (`[playback]`).
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct PlaybackSection {
    /// Use the no-op fake playback engine (headless/CI environments).
    #[serde(default)]
    pub fake: bool,
}

// =============================================================================
// Top-level app config
// =============================================================================

/// Top-level Marginalia configuration, shared across all hosts (TUI, future
/// macOS GUI, mobile). Wraps the section types above with a handful of
/// app-agnostic top-level fields.
///
/// Loaded from TOML. Hosts pick their own default path (TUI uses
/// `apps/tui-rs/marginalia.toml`; the macOS GUI will use
/// `~/Library/Application Support/Marginalia/marginalia.toml`).
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct AppConfig {
    /// Path to the SQLite database. Default: `.marginalia/beta.sqlite3`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub database_path: Option<PathBuf>,
    /// Directory for cached TTS WAV/FLAC files.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tts_cache_dir: Option<PathBuf>,
    /// Target characters per chunk when splitting imported documents.
    /// Default: 300.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chunk_target_chars: Option<usize>,
    /// Trigger words mapped to actions (`pause`, `next`, etc.).
    #[serde(default)]
    pub voice_commands: VoiceCommandsSection,
    /// Speech-to-text engine settings.
    #[serde(default)]
    pub stt: SttSection,
    #[serde(default)]
    pub kokoro: KokoroSection,
    #[serde(default)]
    pub playback: PlaybackSection,
    #[serde(default)]
    pub mlx: MlxSection,
}

impl AppConfig {
    /// Load a config from an explicit path. Returns `Err` if the file is
    /// missing or fails to parse — callers decide whether to fall back to
    /// defaults or surface the error to the user.
    pub fn load_from(path: &std::path::Path) -> Result<Self, String> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        toml::from_str(&content)
            .map_err(|e| format!("cannot parse {}: {e}", path.display()))
    }

    /// Load from `MARGINALIA_CONFIG` env var, or from `default_path` if
    /// the env var is unset. Missing or unparseable files yield `Default`,
    /// with a warning via the `log` crate. Matches the TUI's existing
    /// forgiving behavior.
    pub fn load_or_default(default_path: &std::path::Path) -> Self {
        let path = std::env::var("MARGINALIA_CONFIG")
            .map(PathBuf::from)
            .unwrap_or_else(|_| default_path.to_path_buf());

        if !path.exists() {
            return Self::default();
        }

        match Self::load_from(&path) {
            Ok(cfg) => cfg,
            Err(e) => {
                log::warn!("{e}");
                Self::default()
            }
        }
    }

    /// Serialize this config to TOML and write it atomically to `path`.
    /// The output is canonical (clean, comment-less) — intended for
    /// programmatic writes from the GUI's Apply button. The hand-maintained
    /// template in `apps/tui-rs/marginalia.toml` remains the documentation
    /// reference.
    pub fn write_to(&self, path: &std::path::Path) -> Result<(), String> {
        let body = toml::to_string_pretty(self)
            .map_err(|e| format!("serialize config: {e}"))?;
        let header = "# Marginalia — written by the app. Edit via Settings UI.\n";
        let full = format!("{header}{body}");

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("mkdir {}: {e}", parent.display()))?;
        }
        // Write to a sibling `.tmp` then rename → atomic swap, avoids
        // leaving a half-written file on crash.
        let tmp = path.with_extension("toml.tmp");
        std::fs::write(&tmp, full)
            .map_err(|e| format!("write {}: {e}", tmp.display()))?;
        std::fs::rename(&tmp, path)
            .map_err(|e| format!("rename {} → {}: {e}", tmp.display(), path.display()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_minimal_config() {
        let mut cfg = AppConfig::default();
        cfg.mlx.voice = "if_sara".to_string();
        cfg.stt.engine = "apple".to_string();
        cfg.stt.language = Some("it-IT".to_string());

        let dir = std::env::temp_dir().join(format!("marginalia-cfg-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("marginalia.toml");

        cfg.write_to(&path).unwrap();
        let loaded = AppConfig::load_from(&path).unwrap();
        assert_eq!(loaded.mlx.voice, "if_sara");
        assert_eq!(loaded.stt.engine, "apple");
        assert_eq!(loaded.stt.language.as_deref(), Some("it-IT"));

        // Verify None fields are omitted (not written as `= null`).
        let contents = std::fs::read_to_string(&path).unwrap();
        assert!(!contents.contains("= null"));
    }
}
