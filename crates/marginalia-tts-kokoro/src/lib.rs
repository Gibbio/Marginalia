use marginalia_core::ports::{ProviderCapabilities, ProviderExecutionMode};
use ort::session::Session;
use ort::value::Tensor;
use std::env;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::fs;
use std::path::{Path, PathBuf};

const DEFAULT_MODEL_FILE_CANDIDATES: &[&str] = &[
    "kokoro.onnx",
    "model.onnx",
    "kokoro-v1.0.onnx",
];
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KokoroConfig {
    pub assets_root: PathBuf,
    pub model_file_candidates: Vec<String>,
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
        let voice_path = self.resolve_voice_path();
        let mut missing = Vec::new();
        if model_path.is_none() {
            missing.push(format!(
                "missing model file ({})",
                self.model_file_candidates.join(", ")
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
        let session = builder
            .commit_from_file(&model_path)
            .map_err(KokoroInferenceError::Ort)?;

        Ok(Self { config, session })
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

#[derive(Debug)]
pub enum KokoroInferenceError {
    MissingModelAsset { searched: Vec<String> },
    MissingVoiceAsset { voice: String, searched: Vec<String> },
    MissingRuntimeLibrary { searched: Vec<String> },
    InvalidVoiceData(String),
    InvalidTokens(String),
    InvalidSpeed(String),
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
            Self::Io { context, error } => write!(f, "{context}: {error}"),
            Self::Ort(error) => Display::fmt(error, f),
        }
    }
}

impl Error for KokoroInferenceError {}

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
        pad_input_ids, write_wav_f32, KokoroConfig, KokoroInferenceError, STYLE_VECTOR_WIDTH,
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
        assert_eq!(report.missing.len(), 2);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn readiness_report_resolves_present_assets() {
        let root = temp_dir();
        fs::create_dir_all(root.join("voices")).unwrap();
        fs::write(root.join("kokoro.onnx"), b"onnx").unwrap();
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

    fn temp_dir() -> std::path::PathBuf {
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("marginalia-kokoro-test-{id}"))
    }

    fn temp_file(extension: &str) -> std::path::PathBuf {
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("marginalia-kokoro-test-{id}.{extension}"))
    }
}
