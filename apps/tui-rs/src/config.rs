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
    #[serde(default)]
    #[cfg_attr(not(feature = "mlx-tts"), allow(dead_code))]
    pub mlx: MlxSection,
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

#[derive(Debug, Deserialize, Default)]
#[allow(dead_code)] // fields are read only when vosk-stt feature is enabled
pub struct VoskSection {
    /// Path to the Vosk acoustic model directory.
    pub model_path: Option<PathBuf>,
    /// Voice commands recognised by Vosk. Defaults to `["pausa", "avanti", "indietro", "stop"]`.
    #[serde(default)]
    pub commands: Vec<String>,
    /// Minimum audio peak (0-32767) to consider as speech. Higher = less sensitive.
    /// Default: 3000. MacBook mic ambient noise is ~500-2000.
    pub speech_threshold: Option<i16>,
    /// Seconds of silence after speech before finalizing. Default: 1.2.
    pub silence_timeout: Option<f64>,
    /// Minimum milliseconds of sustained speech to accept a result. Default: 300.
    /// Filters out brief noise spikes that Vosk would force-match to a command.
    pub min_speech_ms: Option<u64>,
}

#[derive(Debug, Deserialize, Default)]
#[allow(dead_code)] // fields are read only when whisper-stt feature is enabled
pub struct WhisperSection {
    /// Path to the Whisper ggml model file (e.g. `ggml-base.bin`).
    pub model_path: Option<PathBuf>,
    /// BCP-47 language code passed to whisper.cpp. Default: `"it"`.
    pub language: Option<String>,
    /// Also use Whisper for voice commands (replaces Vosk). Default: false.
    /// More accurate than Vosk but higher latency (~2s vs instant).
    #[serde(default)]
    pub use_for_commands: bool,
    /// Minimum RMS amplitude (0-32767) to consider as speech. Default: 500.
    /// Lower = more sensitive to quiet speech. Higher = ignores background noise.
    pub speech_threshold: Option<i16>,
    /// Max seconds to record before forcing inference. Default: 4.
    pub max_record_seconds: Option<f64>,
    /// Seconds of silence after speech before finalizing. Default: 1.0.
    pub silence_timeout: Option<f64>,
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
