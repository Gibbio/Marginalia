use marginalia_core::ports::{
    ProviderCapabilities, ProviderExecutionMode, SpeechSynthesizer, SynthesisError,
    SynthesisRequest, SynthesisResult,
};
use ort::session::Session;
use ort::value::Tensor;
use serde_json::Value;
use std::collections::HashMap;
use std::env;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

const DEFAULT_MODEL_FILE_CANDIDATES: &[&str] = &[
    "kokoro.onnx",
    "model.onnx",
    "kokoro-v1.0.onnx",
];
const DEFAULT_CONFIG_FILE_CANDIDATES: &[&str] = &["config.json", "kokoro.config.json"];
const DEFAULT_ONNX_RUNTIME_CANDIDATES: &[&str] = &[
    "libonnxruntime.so",
    "libonnxruntime.so.1",
    "libonnxruntime.dylib",
    "onnxruntime.dll",
    "lib/libonnxruntime.so",
    "lib/libonnxruntime.so.1",
    "lib/libonnxruntime.dylib",
    "bin/onnxruntime.dll",
    "onnxruntime/lib/libonnxruntime.so",
    "onnxruntime/lib/libonnxruntime.so.1",
    "onnxruntime/lib/libonnxruntime.dylib",
    "onnxruntime/bin/onnxruntime.dll",
];
const LEGACY_VOICE_FILE_CANDIDATES: &[&str] = &["voices.bin", "voices.json", "voices-v1.0.bin"];
const DEFAULT_VOICE: &str = "af";
const STYLE_VECTOR_WIDTH: usize = 256;
const KOKORO_MAX_TOKEN_COUNT: usize = 510;

static AUDIO_OUTPUT_COUNTER: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KokoroConfig {
    pub assets_root: PathBuf,
    pub model_file_candidates: Vec<String>,
    pub config_file_candidates: Vec<String>,
    pub onnx_runtime_candidates: Vec<String>,
    pub default_voice: String,
    pub default_language: String,
    pub sample_rate_hz: u32,
}

impl KokoroConfig {
    pub fn from_assets_root(path: impl AsRef<Path>) -> Self {
        Self {
            assets_root: path.as_ref().to_path_buf(),
            model_file_candidates: DEFAULT_MODEL_FILE_CANDIDATES
                .iter()
                .map(ToString::to_string)
                .collect(),
            config_file_candidates: DEFAULT_CONFIG_FILE_CANDIDATES
                .iter()
                .map(ToString::to_string)
                .collect(),
            onnx_runtime_candidates: DEFAULT_ONNX_RUNTIME_CANDIDATES
                .iter()
                .map(ToString::to_string)
                .collect(),
            default_voice: DEFAULT_VOICE.to_string(),
            default_language: "it".to_string(),
            sample_rate_hz: 24_000,
        }
    }

    pub fn resolve_model_path(&self) -> Option<PathBuf> {
        self.model_file_candidates
            .iter()
            .map(|candidate| self.assets_root.join(candidate))
            .find(|path| path.exists())
    }

    pub fn resolve_config_path(&self) -> Option<PathBuf> {
        self.config_file_candidates
            .iter()
            .map(|candidate| self.assets_root.join(candidate))
            .find(|path| path.exists())
    }

    pub fn resolve_voice_path(&self) -> Option<PathBuf> {
        self.resolve_voice_path_for(&self.default_voice)
    }

    pub fn resolve_voice_path_for(&self, voice: &str) -> Option<PathBuf> {
        self.voice_file_candidates_for(voice)
            .iter()
            .map(|candidate| self.assets_root.join(candidate))
            .find(|path| path.exists())
    }

    pub fn voice_file_candidates_for(&self, voice: &str) -> Vec<String> {
        let mut candidates = vec![format!("voices/{voice}.bin"), format!("{voice}.bin")];
        candidates.extend(
            LEGACY_VOICE_FILE_CANDIDATES
                .iter()
                .map(ToString::to_string),
        );
        candidates
    }

    pub fn resolve_onnx_runtime_library_path(&self) -> Option<PathBuf> {
        if let Some(path) = env::var_os("ORT_DYLIB_PATH")
            .map(PathBuf::from)
            .filter(|path| path.exists())
        {
            return Some(path);
        }

        self.onnx_runtime_candidates
            .iter()
            .map(|candidate| self.assets_root.join(candidate))
            .find(|path| path.exists())
    }

    pub fn readiness_report(&self) -> KokoroReadinessReport {
        let model_path = self.resolve_model_path();
        let config_path = self.resolve_config_path();
        let voice_path = self.resolve_voice_path();
        let mut missing = Vec::new();
        if model_path.is_none() {
            missing.push(format!(
                "missing model file ({})",
                self.model_file_candidates.join(", ")
            ));
        }
        if config_path.is_none() {
            missing.push(format!(
                "missing config file ({})",
                self.config_file_candidates.join(", ")
            ));
        }
        if voice_path.is_none() {
            missing.push(format!(
                "missing voice asset for default voice {} ({})",
                self.default_voice,
                self.voice_file_candidates_for(&self.default_voice).join(", ")
            ));
        }

        KokoroReadinessReport {
            assets_root: self.assets_root.clone(),
            model_path,
            config_path,
            voice_path,
            default_voice: self.default_voice.clone(),
            default_language: self.default_language.clone(),
            sample_rate_hz: self.sample_rate_hz,
            missing,
        }
    }

    pub fn probe_onnx_runtime(&self) -> KokoroOnnxProbeReport {
        let model_path = self.resolve_model_path();
        let runtime_library_path = self.resolve_onnx_runtime_library_path();

        if model_path.is_none() {
            return KokoroOnnxProbeReport {
                runtime_library_path,
                session_opened: false,
                input_count: 0,
                output_count: 0,
                error: Some("missing model file".to_string()),
            };
        }

        if runtime_library_path.is_none() {
            return KokoroOnnxProbeReport {
                runtime_library_path,
                session_opened: false,
                input_count: 0,
                output_count: 0,
                error: Some(
                    "missing ONNX Runtime dynamic library (set ORT_DYLIB_PATH or place it under the Kokoro assets directory)"
                        .to_string(),
                ),
            };
        }

        let model_path = model_path.expect("checked above");
        let runtime_library_path = runtime_library_path.expect("checked above");
        match ort::init_from(&runtime_library_path) {
            Ok(environment_builder) => {
                environment_builder.commit();
                match Session::builder() {
                    Ok(mut session_builder) => match session_builder.commit_from_file(&model_path) {
                        Ok(session) => KokoroOnnxProbeReport {
                            runtime_library_path: Some(runtime_library_path),
                            session_opened: true,
                            input_count: session.inputs().len(),
                            output_count: session.outputs().len(),
                            error: None,
                        },
                        Err(error) => KokoroOnnxProbeReport {
                            runtime_library_path: Some(runtime_library_path),
                            session_opened: false,
                            input_count: 0,
                            output_count: 0,
                            error: Some(error.to_string()),
                        },
                    },
                    Err(error) => KokoroOnnxProbeReport {
                        runtime_library_path: Some(runtime_library_path),
                        session_opened: false,
                        input_count: 0,
                        output_count: 0,
                        error: Some(error.to_string()),
                    },
                }
            }
            Err(error) => KokoroOnnxProbeReport {
                runtime_library_path: Some(runtime_library_path),
                session_opened: false,
                input_count: 0,
                output_count: 0,
                error: Some(error.to_string()),
            },
        }
    }

    pub fn doctor_report(&self) -> KokoroDoctorReport {
        KokoroDoctorReport {
            readiness: self.readiness_report(),
            onnx_probe: self.probe_onnx_runtime(),
        }
    }

    pub fn load_vocabulary(&self) -> Result<KokoroVocabulary, KokoroTokenizationError> {
        let config_path = self
            .resolve_config_path()
            .ok_or_else(|| KokoroTokenizationError::MissingConfigAsset {
                searched: self.config_file_candidates.clone(),
            })?;
        let raw = fs::read_to_string(&config_path).map_err(|error| KokoroTokenizationError::Io {
            context: format!("failed to read config file {}", config_path.display()),
            error,
        })?;
        let document: Value =
            serde_json::from_str(&raw).map_err(KokoroTokenizationError::ConfigParse)?;
        let vocab = document
            .get("vocab")
            .and_then(Value::as_object)
            .ok_or_else(|| {
                KokoroTokenizationError::InvalidConfig(
                    "config.json does not contain a vocab object".to_string(),
                )
            })?;

        let mut symbol_to_token = HashMap::new();
        for (symbol, value) in vocab {
            let token_id = value.as_i64().ok_or_else(|| {
                KokoroTokenizationError::InvalidConfig(format!(
                    "vocab entry for {symbol:?} is not an integer"
                ))
            })?;
            symbol_to_token.insert(symbol.clone(), token_id);
        }

        Ok(KokoroVocabulary {
            config_path,
            symbol_to_token,
        })
    }

    pub fn tokenize_phonemes(
        &self,
        phonemes: &str,
    ) -> Result<KokoroTokenizationResult, KokoroTokenizationError> {
        let vocabulary = self.load_vocabulary()?;
        vocabulary.encode_phonemes(phonemes)
    }

    pub fn load_voice_style(
        &self,
        voice: Option<&str>,
        token_count: usize,
    ) -> Result<KokoroVoiceStyle, KokoroInferenceError> {
        let voice = voice.unwrap_or(&self.default_voice);
        let path = self
            .resolve_voice_path_for(voice)
            .ok_or_else(|| KokoroInferenceError::MissingVoiceAsset {
                voice: voice.to_string(),
                searched: self.voice_file_candidates_for(voice),
            })?;

        let bytes = fs::read(&path).map_err(|error| KokoroInferenceError::Io {
            context: format!("failed to read voice file {}", path.display()),
            error,
        })?;
        if bytes.len() % 4 != 0 {
            return Err(KokoroInferenceError::InvalidVoiceData(format!(
                "voice file {} is not aligned to f32 samples",
                path.display()
            )));
        }

        let values = bytes
            .chunks_exact(4)
            .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
            .collect::<Vec<_>>();
        if values.len() % STYLE_VECTOR_WIDTH != 0 {
            return Err(KokoroInferenceError::InvalidVoiceData(format!(
                "voice file {} does not contain {}-wide style vectors",
                path.display(),
                STYLE_VECTOR_WIDTH
            )));
        }

        let frame_count = values.len() / STYLE_VECTOR_WIDTH;
        if frame_count == 0 {
            return Err(KokoroInferenceError::InvalidVoiceData(format!(
                "voice file {} does not contain any style frames",
                path.display()
            )));
        }

        let selected_index = token_count.min(frame_count - 1);
        let start = selected_index * STYLE_VECTOR_WIDTH;
        let end = start + STYLE_VECTOR_WIDTH;

        Ok(KokoroVoiceStyle {
            voice: voice.to_string(),
            path,
            frame_count,
            selected_index,
            style: values[start..end].to_vec(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KokoroReadinessReport {
    pub assets_root: PathBuf,
    pub model_path: Option<PathBuf>,
    pub config_path: Option<PathBuf>,
    pub voice_path: Option<PathBuf>,
    pub default_voice: String,
    pub default_language: String,
    pub sample_rate_hz: u32,
    pub missing: Vec<String>,
}

impl KokoroReadinessReport {
    pub fn is_ready(&self) -> bool {
        self.missing.is_empty()
    }

    pub fn provider_capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            provider_name: "kokoro-beta".to_string(),
            interface_kind: "tts".to_string(),
            supported_languages: vec!["it".to_string(), "en".to_string()],
            supports_streaming: false,
            supports_partial_results: false,
            supports_timestamps: false,
            low_latency_suitable: true,
            offline_capable: true,
            execution_mode: ProviderExecutionMode::Local,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KokoroOnnxProbeReport {
    pub runtime_library_path: Option<PathBuf>,
    pub session_opened: bool,
    pub input_count: usize,
    pub output_count: usize,
    pub error: Option<String>,
}

impl KokoroOnnxProbeReport {
    pub fn is_ready(&self) -> bool {
        self.session_opened
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KokoroDoctorReport {
    pub readiness: KokoroReadinessReport,
    pub onnx_probe: KokoroOnnxProbeReport,
}

impl KokoroDoctorReport {
    pub fn is_ready(&self) -> bool {
        self.readiness.is_ready() && self.onnx_probe.is_ready()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KokoroVocabulary {
    pub config_path: PathBuf,
    pub symbol_to_token: HashMap<String, i64>,
}

impl KokoroVocabulary {
    pub fn encode_phonemes(
        &self,
        phonemes: &str,
    ) -> Result<KokoroTokenizationResult, KokoroTokenizationError> {
        let normalized = normalize_phoneme_text(phonemes);
        let mut token_ids = Vec::with_capacity(normalized.chars().count());
        let mut unknown_symbols = Vec::new();

        for symbol in normalized.chars() {
            let symbol = symbol.to_string();
            if let Some(token_id) = self.symbol_to_token.get(&symbol) {
                token_ids.push(*token_id);
            } else {
                unknown_symbols.push(symbol);
            }
        }

        if !unknown_symbols.is_empty() {
            return Err(KokoroTokenizationError::UnknownSymbols {
                normalized_phonemes: normalized,
                unknown_symbols,
            });
        }

        Ok(KokoroTokenizationResult {
            normalized_phonemes: normalized,
            token_ids,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KokoroTokenizationResult {
    pub normalized_phonemes: String,
    pub token_ids: Vec<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KokoroTextPreparation {
    pub normalized_phonemes: String,
    pub token_ids: Vec<i64>,
    pub mode: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KokoroTextProcessor {
    config: KokoroConfig,
    mode: KokoroTextProcessorMode,
}

impl KokoroTextProcessor {
    pub fn new(config: KokoroConfig) -> Self {
        Self {
            config,
            mode: KokoroTextProcessorMode::ExplicitPrefix,
        }
    }

    pub fn with_external_command(
        config: KokoroConfig,
        command: KokoroExternalPhonemizerConfig,
    ) -> Self {
        Self {
            config,
            mode: KokoroTextProcessorMode::ExternalCommand(command),
        }
    }

    pub fn prepare_text(
        &self,
        text: &str,
    ) -> Result<KokoroTextPreparation, KokoroTextProcessingError> {
        let phonemization = self.phonemize_text(text)?;
        let tokenization = self
            .config
            .tokenize_phonemes(&phonemization.phonemes)
            .map_err(KokoroTextProcessingError::Tokenization)?;
        Ok(KokoroTextPreparation {
            normalized_phonemes: tokenization.normalized_phonemes,
            token_ids: tokenization.token_ids,
            mode: phonemization.mode,
        })
    }

    pub fn phonemize_text(
        &self,
        text: &str,
    ) -> Result<KokoroPhonemization, KokoroTextProcessingError> {
        match &self.mode {
            KokoroTextProcessorMode::ExplicitPrefix => {
                let raw = text.trim();
                let Some((mode, phonemes)) = extract_explicit_phonemes(raw) else {
                    return Err(KokoroTextProcessingError::UnsupportedText(
                        "Kokoro Beta does not yet include grapheme-to-phoneme. Use `phon:` or `ipa:` prefixes."
                            .to_string(),
                    ));
                };
                Ok(KokoroPhonemization {
                    mode: mode.to_string(),
                    phonemes: normalize_phoneme_text(phonemes),
                })
            }
            KokoroTextProcessorMode::ExternalCommand(command) => {
                command.phonemize(text).map_err(KokoroTextProcessingError::ExternalCommand)
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KokoroTextProcessorMode {
    ExplicitPrefix,
    ExternalCommand(KokoroExternalPhonemizerConfig),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KokoroPhonemization {
    pub mode: String,
    pub phonemes: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KokoroExternalPhonemizerConfig {
    pub program: String,
    pub args: Vec<String>,
}

impl KokoroExternalPhonemizerConfig {
    pub fn phonemize(
        &self,
        text: &str,
    ) -> Result<KokoroPhonemization, KokoroExternalPhonemizerError> {
        let mut child = Command::new(&self.program)
            .args(&self.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| KokoroExternalPhonemizerError::Io {
                context: format!("failed to spawn external phonemizer {}", self.program),
                error,
            })?;

        if let Some(stdin) = child.stdin.as_mut() {
            use std::io::Write;
            if let Err(error) = stdin.write_all(text.as_bytes()).and_then(|_| stdin.flush()) {
                if error.kind() != std::io::ErrorKind::BrokenPipe {
                    return Err(KokoroExternalPhonemizerError::Io {
                        context: format!(
                            "failed to write to external phonemizer {}",
                            self.program
                        ),
                        error,
                    });
                }
            }
        }

        let output = child
            .wait_with_output()
            .map_err(|error| KokoroExternalPhonemizerError::Io {
                context: format!("failed to wait for external phonemizer {}", self.program),
                error,
            })?;

        if !output.status.success() {
            return Err(KokoroExternalPhonemizerError::ProcessFailed {
                status: output.status.code(),
                stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
            });
        }

        let phonemes = String::from_utf8(output.stdout)
            .map_err(KokoroExternalPhonemizerError::Utf8)?
            .trim()
            .to_string();
        if phonemes.is_empty() {
            return Err(KokoroExternalPhonemizerError::EmptyOutput);
        }

        Ok(KokoroPhonemization {
            mode: "external-command".to_string(),
            phonemes: normalize_phoneme_text(&phonemes),
        })
    }
}

#[derive(Debug)]
pub enum KokoroTextProcessingError {
    UnsupportedText(String),
    Tokenization(KokoroTokenizationError),
    ExternalCommand(KokoroExternalPhonemizerError),
}

impl Display for KokoroTextProcessingError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedText(message) => f.write_str(message),
            Self::Tokenization(error) => Display::fmt(error, f),
            Self::ExternalCommand(error) => Display::fmt(error, f),
        }
    }
}

impl Error for KokoroTextProcessingError {}

#[derive(Debug)]
pub enum KokoroExternalPhonemizerError {
    Io { context: String, error: std::io::Error },
    Utf8(std::string::FromUtf8Error),
    ProcessFailed { status: Option<i32>, stderr: String },
    EmptyOutput,
}

impl Display for KokoroExternalPhonemizerError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io { context, error } => write!(f, "{context}: {error}"),
            Self::Utf8(error) => Display::fmt(error, f),
            Self::ProcessFailed { status, stderr } => {
                write!(
                    f,
                    "external phonemizer failed with status {}{}",
                    status
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| "unknown".to_string()),
                    if stderr.is_empty() {
                        "".to_string()
                    } else {
                        format!(": {stderr}")
                    }
                )
            }
            Self::EmptyOutput => f.write_str("external phonemizer returned empty output"),
        }
    }
}

impl Error for KokoroExternalPhonemizerError {}

#[derive(Debug, Clone, PartialEq)]
pub struct KokoroVoiceStyle {
    pub voice: String,
    pub path: PathBuf,
    pub frame_count: usize,
    pub selected_index: usize,
    pub style: Vec<f32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct KokoroInferenceRequest {
    pub token_ids: Vec<i64>,
    pub voice: Option<String>,
    pub speed: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct KokoroInferenceResult {
    pub voice: String,
    pub sample_rate_hz: u32,
    pub output_shape: Vec<i64>,
    pub input_token_count: usize,
    pub audio: Vec<f32>,
}

#[derive(Debug)]
pub struct KokoroOnnxModel {
    config: KokoroConfig,
    session: Session,
}

#[derive(Debug, Clone, PartialEq)]
pub struct KokoroSpeechSynthesizerConfig {
    pub output_dir: PathBuf,
    pub default_speed: f32,
}

impl KokoroSpeechSynthesizerConfig {
    pub fn new(output_dir: impl AsRef<Path>) -> Self {
        Self {
            output_dir: output_dir.as_ref().to_path_buf(),
            default_speed: 1.0,
        }
    }
}

#[derive(Debug)]
pub struct KokoroSpeechSynthesizer {
    provider_name: String,
    config: KokoroConfig,
    runtime: KokoroSpeechSynthesizerConfig,
    text_processor: KokoroTextProcessor,
    model: Option<KokoroOnnxModel>,
}

impl KokoroSpeechSynthesizer {
    pub fn new(config: KokoroConfig, runtime: KokoroSpeechSynthesizerConfig) -> Self {
        let text_processor = KokoroTextProcessor::new(config.clone());
        Self::with_text_processor(config, runtime, text_processor)
    }

    pub fn with_text_processor(
        config: KokoroConfig,
        runtime: KokoroSpeechSynthesizerConfig,
        text_processor: KokoroTextProcessor,
    ) -> Self {
        Self {
            provider_name: "kokoro-beta".to_string(),
            config,
            runtime,
            text_processor,
            model: None,
        }
    }

    fn ensure_model(&mut self) -> Result<&mut KokoroOnnxModel, KokoroInferenceError> {
        if self.model.is_none() {
            self.model = Some(KokoroOnnxModel::load(self.config.clone())?);
        }
        Ok(self.model.as_mut().expect("model initialized"))
    }
}

#[derive(Debug)]
pub enum KokoroTokenizationError {
    MissingConfigAsset { searched: Vec<String> },
    InvalidConfig(String),
    UnknownSymbols {
        normalized_phonemes: String,
        unknown_symbols: Vec<String>,
    },
    Io { context: String, error: std::io::Error },
    ConfigParse(serde_json::Error),
}

impl Display for KokoroTokenizationError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingConfigAsset { searched } => {
                write!(f, "missing config asset ({})", searched.join(", "))
            }
            Self::InvalidConfig(message) => f.write_str(message),
            Self::UnknownSymbols {
                unknown_symbols, ..
            } => write!(f, "unsupported phoneme symbols: {}", unknown_symbols.join(", ")),
            Self::Io { context, error } => write!(f, "{context}: {error}"),
            Self::ConfigParse(error) => Display::fmt(error, f),
        }
    }
}

impl Error for KokoroTokenizationError {}

impl KokoroOnnxModel {
    pub fn load(config: KokoroConfig) -> Result<Self, KokoroInferenceError> {
        let model_path = config
            .resolve_model_path()
            .ok_or_else(|| KokoroInferenceError::MissingModelAsset {
                searched: config.model_file_candidates.clone(),
            })?;
        let runtime_library_path = config
            .resolve_onnx_runtime_library_path()
            .ok_or_else(|| KokoroInferenceError::MissingRuntimeLibrary {
                searched: config.onnx_runtime_candidates.clone(),
            })?;

        ort::init_from(&runtime_library_path)
            .map_err(KokoroInferenceError::Ort)?
            .commit();

        let mut builder = Session::builder().map_err(KokoroInferenceError::Ort)?;
        // Register hardware execution providers when compiled with the relevant feature.
        // ORT silently falls back to CPU if the EP is not available in the loaded library.
        #[cfg(feature = "coreml")]
        let mut builder = builder
            .with_execution_providers([
                ort::execution_providers::CoreMLExecutionProvider::default().build(),
            ])
            .map_err(|e| KokoroInferenceError::Ort(e.into()))?;
        #[cfg(feature = "cuda")]
        let mut builder = builder
            .with_execution_providers([
                ort::execution_providers::CUDAExecutionProvider::default().build(),
            ])
            .map_err(|e| KokoroInferenceError::Ort(e.into()))?;

        let session = builder
            .commit_from_file(&model_path)
            .map_err(KokoroInferenceError::Ort)?;

        Ok(Self { config, session })
    }

    pub fn infer_phonemes(
        &mut self,
        phonemes: &str,
        voice: Option<String>,
        speed: f32,
    ) -> Result<KokoroInferenceResult, KokoroInferenceError> {
        let tokenization = self
            .config
            .tokenize_phonemes(phonemes)
            .map_err(KokoroInferenceError::Tokenization)?;
        self.infer(KokoroInferenceRequest {
            token_ids: tokenization.token_ids,
            voice,
            speed,
        })
    }

    pub fn infer(
        &mut self,
        request: KokoroInferenceRequest,
    ) -> Result<KokoroInferenceResult, KokoroInferenceError> {
        if request.token_ids.is_empty() {
            return Err(KokoroInferenceError::InvalidTokens(
                "token_ids must not be empty".to_string(),
            ));
        }
        if request.token_ids.len() > KOKORO_MAX_TOKEN_COUNT {
            return Err(KokoroInferenceError::InvalidTokens(format!(
                "token_ids must contain at most {KOKORO_MAX_TOKEN_COUNT} items"
            )));
        }
        if !(request.speed.is_finite() && request.speed > 0.0) {
            return Err(KokoroInferenceError::InvalidSpeed(
                "speed must be a positive finite number".to_string(),
            ));
        }

        let voice_style = self
            .config
            .load_voice_style(request.voice.as_deref(), request.token_ids.len())?;
        let padded_tokens = pad_input_ids(&request.token_ids);
        let token_shape = vec![1_i64, padded_tokens.len() as i64];
        let style_shape = vec![1_i64, STYLE_VECTOR_WIDTH as i64];
        let speed_shape = vec![1_i64];

        let outputs = self
            .session
            .run(ort::inputs! {
                "input_ids" => Tensor::<i64>::from_array((token_shape, padded_tokens)).map_err(KokoroInferenceError::Ort)?,
                "style" => Tensor::<f32>::from_array((style_shape, voice_style.style)).map_err(KokoroInferenceError::Ort)?,
                "speed" => Tensor::<f32>::from_array((speed_shape, vec![request.speed])).map_err(KokoroInferenceError::Ort)?,
            })
            .map_err(KokoroInferenceError::Ort)?;

        let output = &outputs[0];
        let (shape, data) = output
            .try_extract_tensor::<f32>()
            .map_err(KokoroInferenceError::Ort)?;

        Ok(KokoroInferenceResult {
            voice: voice_style.voice,
            sample_rate_hz: self.config.sample_rate_hz,
            output_shape: shape.iter().copied().collect(),
            input_token_count: request.token_ids.len(),
            audio: data.to_vec(),
        })
    }
}

impl SpeechSynthesizer for KokoroSpeechSynthesizer {
    fn describe_capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            provider_name: self.provider_name.clone(),
            interface_kind: "tts".to_string(),
            supported_languages: vec!["it".to_string(), "en".to_string()],
            supports_streaming: false,
            supports_partial_results: false,
            supports_timestamps: false,
            low_latency_suitable: true,
            offline_capable: true,
            execution_mode: ProviderExecutionMode::Local,
        }
    }

    fn synthesize(&mut self, request: SynthesisRequest) -> Result<SynthesisResult, SynthesisError> {
        let prepared = self
            .text_processor
            .prepare_text(&request.text)
            .map_err(|error| {
                SynthesisError::new(self.provider_name.clone(), error.to_string())
                    .with_metadata("language", request.language.clone())
            })?;

        let selected_voice = request
            .voice
            .clone()
            .unwrap_or_else(|| self.config.default_voice.clone());
        let default_speed = self.runtime.default_speed;
        let inference = self
            .ensure_model()
            .and_then(|model| {
                model.infer(KokoroInferenceRequest {
                    token_ids: prepared.token_ids.clone(),
                    voice: Some(selected_voice.clone()),
                    speed: default_speed,
                })
            })
            .map_err(|error| {
                SynthesisError::new(self.provider_name.clone(), error.to_string())
                    .with_metadata("voice", selected_voice.clone())
                    .with_metadata("language", request.language.clone())
                    .with_metadata("mode", prepared.mode.clone())
            })?;

        fs::create_dir_all(&self.runtime.output_dir).map_err(|error| {
            SynthesisError::new(
                self.provider_name.clone(),
                format!(
                    "failed to create output directory {}: {error}",
                    self.runtime.output_dir.display()
                ),
            )
        })?;
        let output_path = next_audio_output_path(&self.runtime.output_dir, &selected_voice);
        write_wav_f32(&output_path, inference.sample_rate_hz, &inference.audio).map_err(|error| {
            SynthesisError::new(
                self.provider_name.clone(),
                format!("failed to write wav {}: {error}", output_path.display()),
            )
        })?;

        let mut metadata = HashMap::new();
        metadata.insert("language".to_string(), request.language);
        metadata.insert("mode".to_string(), prepared.mode);
        metadata.insert("phonemes".to_string(), prepared.normalized_phonemes);
        metadata.insert(
            "token_count".to_string(),
            inference.input_token_count.to_string(),
        );

        Ok(SynthesisResult {
            provider_name: self.provider_name.clone(),
            voice: selected_voice,
            content_type: "audio/wav".to_string(),
            audio_reference: output_path.display().to_string(),
            byte_length: output_path
                .metadata()
                .map(|metadata| metadata.len() as usize)
                .unwrap_or(0),
            text_excerpt: request.text.chars().take(120).collect(),
            metadata,
        })
    }
}

#[derive(Debug)]
pub enum KokoroInferenceError {
    MissingModelAsset { searched: Vec<String> },
    MissingVoiceAsset { voice: String, searched: Vec<String> },
    MissingRuntimeLibrary { searched: Vec<String> },
    InvalidVoiceData(String),
    InvalidTokens(String),
    InvalidSpeed(String),
    Tokenization(KokoroTokenizationError),
    Io { context: String, error: std::io::Error },
    Ort(ort::Error),
}

impl Display for KokoroInferenceError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingModelAsset { searched } => {
                write!(f, "missing model asset ({})", searched.join(", "))
            }
            Self::MissingVoiceAsset { voice, searched } => write!(
                f,
                "missing voice asset for {voice} ({})",
                searched.join(", ")
            ),
            Self::MissingRuntimeLibrary { searched } => write!(
                f,
                "missing ONNX Runtime dynamic library ({})",
                searched.join(", ")
            ),
            Self::InvalidVoiceData(message) => f.write_str(message),
            Self::InvalidTokens(message) => f.write_str(message),
            Self::InvalidSpeed(message) => f.write_str(message),
            Self::Tokenization(error) => Display::fmt(error, f),
            Self::Io { context, error } => write!(f, "{context}: {error}"),
            Self::Ort(error) => Display::fmt(error, f),
        }
    }
}

impl Error for KokoroInferenceError {}

fn extract_explicit_phonemes(text: &str) -> Option<(&'static str, &str)> {
    for (prefix, mode) in [
        ("phonemes:", "phonemes"),
        ("phon:", "phonemes"),
        ("ipa:", "ipa"),
    ] {
        if let Some(rest) = text.strip_prefix(prefix) {
            return Some((mode, rest.trim()));
        }
    }
    None
}

fn next_audio_output_path(output_dir: &Path, voice: &str) -> PathBuf {
    let id = AUDIO_OUTPUT_COUNTER.fetch_add(1, Ordering::Relaxed);
    output_dir.join(format!(
        "kokoro-{id}-{}.wav",
        sanitize_path_fragment(voice)
    ))
}

fn sanitize_path_fragment(input: &str) -> String {
    let rendered = input
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    let trimmed = rendered.trim_matches('_');
    if trimmed.is_empty() {
        "voice".to_string()
    } else {
        trimmed.chars().take(24).collect()
    }
}

pub fn write_wav_f32(
    path: impl AsRef<Path>,
    sample_rate_hz: u32,
    audio: &[f32],
) -> Result<(), std::io::Error> {
    let channels = 1u16;
    let bits_per_sample = 16u16;
    let bytes_per_sample = (bits_per_sample / 8) as usize;
    let data_size = audio.len() * bytes_per_sample;
    let byte_rate = sample_rate_hz * channels as u32 * bytes_per_sample as u32;
    let block_align = channels * bits_per_sample / 8;
    let riff_size = 36 + data_size as u32;

    let mut bytes = Vec::with_capacity(44 + data_size);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&riff_size.to_le_bytes());
    bytes.extend_from_slice(b"WAVE");
    bytes.extend_from_slice(b"fmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&channels.to_le_bytes());
    bytes.extend_from_slice(&sample_rate_hz.to_le_bytes());
    bytes.extend_from_slice(&byte_rate.to_le_bytes());
    bytes.extend_from_slice(&block_align.to_le_bytes());
    bytes.extend_from_slice(&bits_per_sample.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&(data_size as u32).to_le_bytes());
    for sample in audio {
        let pcm = (sample.clamp(-1.0, 1.0) * i16::MAX as f32).round() as i16;
        bytes.extend_from_slice(&pcm.to_le_bytes());
    }
    fs::write(path, bytes)
}

fn normalize_phoneme_text(input: &str) -> String {
    let mut normalized = String::with_capacity(input.len());
    let mut previous_was_space = false;
    for ch in input.chars() {
        let mapped = if ch.is_whitespace() { ' ' } else { ch };
        if mapped == ' ' {
            if !previous_was_space && !normalized.is_empty() {
                normalized.push(' ');
            }
            previous_was_space = true;
        } else {
            normalized.push(mapped);
            previous_was_space = false;
        }
    }
    normalized.trim().to_string()
}

fn pad_input_ids(token_ids: &[i64]) -> Vec<i64> {
    let mut padded = Vec::with_capacity(token_ids.len() + 2);
    padded.push(0);
    padded.extend_from_slice(token_ids);
    padded.push(0);
    padded
}

#[cfg(test)]
mod tests {
    use super::{
        normalize_phoneme_text, pad_input_ids, write_wav_f32, KokoroConfig,
        KokoroExternalPhonemizerConfig, KokoroExternalPhonemizerError, KokoroInferenceError,
        KokoroTextProcessingError, KokoroTextProcessor, KokoroTokenizationError,
        STYLE_VECTOR_WIDTH,
    };
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(1);

    #[test]
    fn readiness_report_detects_missing_assets() {
        let root = temp_dir();
        fs::create_dir_all(&root).unwrap();

        let report = KokoroConfig::from_assets_root(&root).readiness_report();

        assert!(!report.is_ready());
        assert_eq!(report.missing.len(), 3);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn readiness_report_resolves_present_assets() {
        let root = temp_dir();
        fs::create_dir_all(root.join("voices")).unwrap();
        fs::write(root.join("kokoro.onnx"), b"onnx").unwrap();
        fs::write(root.join("config.json"), sample_config_json()).unwrap();
        fs::write(root.join("voices").join("af.bin"), vec![0_u8; STYLE_VECTOR_WIDTH * 8]).unwrap();

        let report = KokoroConfig::from_assets_root(&root).readiness_report();

        assert!(report.is_ready());
        assert!(report.model_path.is_some());
        assert!(report.voice_path.is_some());
        assert_eq!(report.default_voice, "af");

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn resolve_onnx_runtime_library_uses_assets_directory() {
        let root = temp_dir();
        fs::create_dir_all(root.join("lib")).unwrap();
        fs::write(root.join("lib").join("libonnxruntime.so"), b"runtime").unwrap();

        let config = KokoroConfig::from_assets_root(&root);

        assert_eq!(
            config.resolve_onnx_runtime_library_path(),
            Some(root.join("lib").join("libonnxruntime.so"))
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn load_voice_style_selects_frame_by_token_count() {
        let root = temp_dir();
        fs::create_dir_all(root.join("voices")).unwrap();

        let mut values = Vec::new();
        for frame in 0..3 {
            for _ in 0..STYLE_VECTOR_WIDTH {
                values.push(frame as f32);
            }
        }
        let bytes = values
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect::<Vec<_>>();
        fs::write(root.join("voices").join("af.bin"), bytes).unwrap();

        let config = KokoroConfig::from_assets_root(&root);
        let style = config.load_voice_style(None, 2).unwrap();

        assert_eq!(style.voice, "af");
        assert_eq!(style.frame_count, 3);
        assert_eq!(style.selected_index, 2);
        assert_eq!(style.style.len(), STYLE_VECTOR_WIDTH);
        assert!(style.style.iter().all(|sample| *sample == 2.0));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn load_voice_style_reports_missing_voice() {
        let root = temp_dir();
        fs::create_dir_all(&root).unwrap();

        let config = KokoroConfig::from_assets_root(&root);
        let error = config.load_voice_style(Some("af_bella"), 12).unwrap_err();

        match error {
            KokoroInferenceError::MissingVoiceAsset { voice, .. } => {
                assert_eq!(voice, "af_bella");
            }
            other => panic!("unexpected error: {other}"),
        }

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn pad_input_ids_wraps_with_zero_tokens() {
        assert_eq!(pad_input_ids(&[11, 22, 33]), vec![0, 11, 22, 33, 0]);
    }

    #[test]
    fn write_wav_f32_creates_pcm_file() {
        let path = temp_file("wav");
        write_wav_f32(&path, 24_000, &[0.0, 0.25, -0.25]).unwrap();

        let bytes = fs::read(&path).unwrap();
        assert!(bytes.starts_with(b"RIFF"));
        assert!(bytes.len() > 44);

        let _ = fs::remove_file(path);
    }

    #[test]
    fn tokenize_phonemes_uses_vocab_from_config() {
        let root = temp_dir();
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("config.json"), sample_config_json()).unwrap();

        let config = KokoroConfig::from_assets_root(&root);
        let tokenization = config.tokenize_phonemes("h ə l o").unwrap();

        assert_eq!(tokenization.normalized_phonemes, "h ə l o");
        assert_eq!(tokenization.token_ids, vec![50, 16, 83, 16, 54, 16, 57]);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn tokenize_phonemes_reports_unknown_symbols() {
        let root = temp_dir();
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("config.json"), sample_config_json()).unwrap();

        let config = KokoroConfig::from_assets_root(&root);
        let error = config.tokenize_phonemes("h λ").unwrap_err();

        match error {
            KokoroTokenizationError::UnknownSymbols { unknown_symbols, .. } => {
                assert_eq!(unknown_symbols, vec!["λ".to_string()]);
            }
            other => panic!("unexpected error: {other}"),
        }

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn normalize_phoneme_text_collapses_whitespace() {
        assert_eq!(normalize_phoneme_text(" h\tə \n l o "), "h ə l o");
    }

    #[test]
    fn text_processor_accepts_explicit_phoneme_prefix() {
        let root = temp_dir();
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("config.json"), sample_config_json()).unwrap();

        let processor = KokoroTextProcessor::new(KokoroConfig::from_assets_root(&root));
        let prepared = processor.prepare_text("phon: h ə l o").unwrap();

        assert_eq!(prepared.mode, "phonemes");
        assert_eq!(prepared.normalized_phonemes, "h ə l o");
        assert_eq!(prepared.token_ids, vec![50, 16, 83, 16, 54, 16, 57]);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn text_processor_rejects_plain_text_without_g2p() {
        let processor = KokoroTextProcessor::new(KokoroConfig::from_assets_root(temp_dir()));
        let error = processor.prepare_text("ciao mondo").unwrap_err();

        match error {
            KokoroTextProcessingError::UnsupportedText(message) => {
                assert!(message.contains("grapheme-to-phoneme"));
            }
            other => panic!("unexpected error: {other}"),
        }
    }

    #[test]
    fn external_command_phonemizer_can_prepare_text() {
        let root = temp_dir();
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("config.json"), sample_config_json()).unwrap();

        let processor = KokoroTextProcessor::with_external_command(
            KokoroConfig::from_assets_root(&root),
            KokoroExternalPhonemizerConfig {
                program: "/bin/sh".to_string(),
                args: vec!["-c".to_string(), "printf 'h ə l o'".to_string()],
            },
        );
        let prepared = processor.prepare_text("ignored plain text").unwrap();

        assert_eq!(prepared.mode, "external-command");
        assert_eq!(prepared.normalized_phonemes, "h ə l o");
        assert_eq!(prepared.token_ids, vec![50, 16, 83, 16, 54, 16, 57]);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn external_command_phonemizer_reports_process_failure() {
        let root = temp_dir();
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("config.json"), sample_config_json()).unwrap();

        let processor = KokoroTextProcessor::with_external_command(
            KokoroConfig::from_assets_root(&root),
            KokoroExternalPhonemizerConfig {
                program: "/bin/sh".to_string(),
                args: vec![
                    "-c".to_string(),
                    "echo boom >&2; exit 4".to_string(),
                ],
            },
        );
        let error = processor.prepare_text("ignored plain text").unwrap_err();

        match error {
            KokoroTextProcessingError::ExternalCommand(
                KokoroExternalPhonemizerError::ProcessFailed { status, stderr },
            ) => {
                assert_eq!(status, Some(4));
                assert_eq!(stderr, "boom");
            }
            other => panic!("unexpected error: {other}"),
        }

        let _ = fs::remove_dir_all(root);
    }

    fn temp_dir() -> std::path::PathBuf {
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("marginalia-kokoro-test-{id}"))
    }

    fn temp_file(extension: &str) -> std::path::PathBuf {
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("marginalia-kokoro-test-{id}.{extension}"))
    }

    fn sample_config_json() -> &'static str {
        r#"{
  "vocab": {
    " ": 16,
    "h": 50,
    "l": 54,
    "o": 57,
    "ə": 83
  }
}"#
    }
}
