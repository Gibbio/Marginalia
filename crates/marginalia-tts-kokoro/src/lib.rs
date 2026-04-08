use marginalia_core::ports::{ProviderCapabilities, ProviderExecutionMode};
use std::env;
use std::path::{Path, PathBuf};

const DEFAULT_MODEL_FILE_CANDIDATES: &[&str] = &[
    "kokoro.onnx",
    "model.onnx",
    "kokoro-v1.0.onnx",
];
const DEFAULT_VOICE_FILE_CANDIDATES: &[&str] = &[
    "voices.json",
    "voices.bin",
    "voices-v1.0.bin",
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KokoroConfig {
    pub assets_root: PathBuf,
    pub model_file_candidates: Vec<String>,
    pub voice_file_candidates: Vec<String>,
    pub onnx_runtime_candidates: Vec<String>,
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
            voice_file_candidates: DEFAULT_VOICE_FILE_CANDIDATES
                .iter()
                .map(ToString::to_string)
                .collect(),
            onnx_runtime_candidates: DEFAULT_ONNX_RUNTIME_CANDIDATES
                .iter()
                .map(ToString::to_string)
                .collect(),
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
        self.voice_file_candidates
            .iter()
            .map(|candidate| self.assets_root.join(candidate))
            .find(|path| path.exists())
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
                "missing voice file ({})",
                self.voice_file_candidates.join(", ")
            ));
        }

        KokoroReadinessReport {
            assets_root: self.assets_root.clone(),
            model_path,
            voice_path,
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
                match ort::session::Session::builder() {
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KokoroReadinessReport {
    pub assets_root: PathBuf,
    pub model_path: Option<PathBuf>,
    pub voice_path: Option<PathBuf>,
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

#[cfg(test)]
mod tests {
    use super::KokoroConfig;
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
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("kokoro.onnx"), b"onnx").unwrap();
        fs::write(root.join("voices.json"), b"{}").unwrap();

        let report = KokoroConfig::from_assets_root(&root).readiness_report();

        assert!(report.is_ready());
        assert!(report.model_path.is_some());
        assert!(report.voice_path.is_some());

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

    fn temp_dir() -> std::path::PathBuf {
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("marginalia-kokoro-test-{id}"))
    }
}
