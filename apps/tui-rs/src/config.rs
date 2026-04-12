use serde::Deserialize;
use std::path::PathBuf;

// Re-export shared config types so the rest of the TUI can use them
// via `crate::config::VoiceCommandsSection` etc., unchanged.
pub use marginalia_config::{
    KokoroSection, MlxSection, PlaybackSection, SttSection, VoiceCommandsSection,
};

/// TUI-specific top-level configuration. Combines shared section types from
/// `marginalia-config` with TUI-only fields (`database_path`, `chunk_target_chars`).
///
/// Loaded from `apps/tui-rs/marginalia.toml` by default, or from the path
/// specified in the `MARGINALIA_CONFIG` environment variable.
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
    /// Speech-to-text engine settings.
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
                log::warn!("cannot read config {}: {e}", path.display());
                Self::default()
            }
            Ok(content) => match toml::from_str(&content) {
                Err(e) => {
                    log::warn!("cannot parse config {}: {e}", path.display());
                    Self::default()
                }
                Ok(cfg) => cfg,
            },
        }
    }
}
