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
    pub voice_commands: VoiceCommandsSection,
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

#[derive(Debug, Clone)]
pub enum SpeechThreshold {
    Auto,
    Fixed(i16),
}

impl Default for SpeechThreshold {
    fn default() -> Self {
        Self::Auto
    }
}

fn default_vosk_threshold() -> SpeechThreshold {
    SpeechThreshold::Auto
}

impl<'de> Deserialize<'de> for SpeechThreshold {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de;
        struct Visitor;
        impl de::Visitor<'_> for Visitor {
            type Value = SpeechThreshold;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(f, r#""auto" or a number 0-32767"#)
            }

            fn visit_str<E: de::Error>(self, v: &str) -> Result<SpeechThreshold, E> {
                if v.eq_ignore_ascii_case("auto") {
                    Ok(SpeechThreshold::Auto)
                } else {
                    Err(E::custom(format!(
                        "expected \"auto\" or a number, got \"{v}\""
                    )))
                }
            }

            fn visit_i64<E: de::Error>(self, v: i64) -> Result<SpeechThreshold, E> {
                Ok(SpeechThreshold::Fixed(v as i16))
            }

            fn visit_u64<E: de::Error>(self, v: u64) -> Result<SpeechThreshold, E> {
                Ok(SpeechThreshold::Fixed(v as i16))
            }
        }
        deserializer.deserialize_any(Visitor)
    }
}

#[derive(Debug, Deserialize, Default)]
#[allow(dead_code)] // fields are read only when vosk-stt feature is enabled
pub struct VoskSection {
    /// Path to the Vosk acoustic model directory.
    pub model_path: Option<PathBuf>,
    /// Minimum audio peak to consider as speech.
    /// "auto" = adaptive noise floor (continuously adjusts to ambient noise).
    /// Or a fixed number 0-32767 (higher = less sensitive). Default: "auto".
    #[serde(default = "default_vosk_threshold")]
    pub speech_threshold: SpeechThreshold,
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
    /// Show STT raw text in Log pane (what the mic heard). Default: false.
    #[serde(default)]
    pub debug: bool,
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
            debug: false,
        }
    }
}

impl VoiceCommandsSection {
    /// Flat list of all trigger words (for the STT backend).
    pub fn all_words(&self) -> Vec<String> {
        let mut words = Vec::new();
        words.extend(self.pause.clone());
        words.extend(self.next.clone());
        words.extend(self.back.clone());
        words.extend(self.stop.clone());
        words.extend(self.repeat.clone());
        words.extend(self.resume.clone());
        words
    }

    /// Map a recognized word back to an action name.
    pub fn resolve_action(&self, word: &str) -> Option<&'static str> {
        let w = word.to_lowercase();
        if self.pause.iter().any(|t| w.contains(&t.to_lowercase())) {
            Some("pause")
        } else if self.next.iter().any(|t| w.contains(&t.to_lowercase())) {
            Some("next")
        } else if self.back.iter().any(|t| w.contains(&t.to_lowercase())) {
            Some("back")
        } else if self.stop.iter().any(|t| w.contains(&t.to_lowercase())) {
            Some("stop")
        } else if self.repeat.iter().any(|t| w.contains(&t.to_lowercase())) {
            Some("repeat")
        } else if self.resume.iter().any(|t| w.contains(&t.to_lowercase())) {
            Some("resume")
        } else {
            None
        }
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
