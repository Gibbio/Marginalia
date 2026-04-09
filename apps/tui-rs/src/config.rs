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
    #[serde(default)]
    pub kokoro: KokoroSection,
    #[serde(default)]
    #[cfg_attr(not(feature = "vosk-stt"), allow(dead_code))]
    pub vosk: VoskSection,
    #[serde(default)]
    #[cfg_attr(not(feature = "whisper-stt"), allow(dead_code))]
    pub whisper: WhisperSection,
    #[serde(default)]
    pub playback: PlaybackSection,
}

#[derive(Debug, Deserialize, Default)]
pub struct KokoroSection {
    /// Directory containing `kokoro.onnx`, `config.json`, `voices/`, and `lib/`.
    pub assets_root: Option<PathBuf>,
    /// Directory where synthesised WAV files are cached.
    /// Default: `<database_dir>/.marginalia-tts-cache/`.
    pub tts_cache_dir: Option<PathBuf>,
}

#[derive(Debug, Deserialize, Default)]
#[allow(dead_code)] // fields are read only when vosk-stt feature is enabled
pub struct VoskSection {
    /// Path to the Vosk acoustic model directory.
    pub model_path: Option<PathBuf>,
    /// Voice commands recognised by Vosk. Defaults to `["pausa", "avanti", "indietro", "stop"]`.
    #[serde(default)]
    pub commands: Vec<String>,
}

#[derive(Debug, Deserialize, Default)]
#[allow(dead_code)] // fields are read only when whisper-stt feature is enabled
pub struct WhisperSection {
    /// Path to the Whisper ggml model file (e.g. `ggml-base.bin`).
    pub model_path: Option<PathBuf>,
    /// BCP-47 language code passed to whisper.cpp. Default: `"it"`.
    pub language: Option<String>,
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
