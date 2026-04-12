//! Model discovery, download, and cache management for Marginalia.
//!
//! Provides a programmatic API to ensure models are available locally.
//! Desktop apps can use this as an alternative to `make bootstrap-*`;
//! mobile apps MUST use this since they can't run Make.
//!
//! Models are downloaded from HuggingFace Hub and cached in the standard
//! HF cache (`~/.cache/huggingface/hub/`). The API returns local file
//! paths that the caller can pass to provider constructors.

use hf_hub::api::sync::Api;
use std::path::PathBuf;

/// Errors that can occur during model management operations.
#[derive(Debug)]
pub enum ModelError {
    /// A model download from HuggingFace Hub failed.
    Download(String),
    /// The requested model was not found locally or remotely.
    NotFound(String),
}

impl std::fmt::Display for ModelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Download(e) => write!(f, "model download failed: {e}"),
            Self::NotFound(e) => write!(f, "model not found: {e}"),
        }
    }
}

impl std::error::Error for ModelError {}

/// Manages model discovery and download. Uses HuggingFace Hub for
/// retrieval and caching.
pub struct ModelManager {
    api: Api,
}

impl ModelManager {
    /// Create a new model manager, initializing the HuggingFace Hub API.
    pub fn new() -> Result<Self, ModelError> {
        let api = Api::new().map_err(|e| ModelError::Download(e.to_string()))?;
        Ok(Self { api })
    }

    /// Ensure the Whisper GGML model is available locally.
    /// Downloads from `ggerganov/whisper.cpp` on HuggingFace if not cached.
    /// Returns the local path to the `.bin` file.
    pub fn ensure_whisper(&self, model_name: &str) -> Result<PathBuf, ModelError> {
        let repo = self.api.model("ggerganov/whisper.cpp".to_string());
        log::info!("[models] ensuring whisper model: {model_name}");
        let path = repo
            .get(model_name)
            .map_err(|e| ModelError::Download(format!("{model_name}: {e}")))?;
        log::info!("[models] whisper model ready: {}", path.display());
        Ok(path)
    }

    /// Ensure a Kokoro ONNX model file is available locally.
    /// Downloads from `onnx-community/Kokoro-82M` on HuggingFace if not cached.
    /// Returns the local path to the model file.
    pub fn ensure_kokoro_onnx(&self, file_name: &str) -> Result<PathBuf, ModelError> {
        let repo = self.api.model("onnx-community/Kokoro-82M".to_string());
        log::info!("[models] ensuring kokoro onnx: {file_name}");
        let path = repo
            .get(file_name)
            .map_err(|e| ModelError::Download(format!("{file_name}: {e}")))?;
        log::info!("[models] kokoro onnx ready: {}", path.display());
        Ok(path)
    }

    /// Ensure a Kokoro voice embedding is available locally.
    /// Downloads from `hexgrad/Kokoro-82M` on HuggingFace if not cached.
    /// Returns the local path to the voice file.
    pub fn ensure_kokoro_voice(&self, voice_name: &str) -> Result<PathBuf, ModelError> {
        let repo = self.api.model("hexgrad/Kokoro-82M".to_string());
        let file_name = format!("voices/{voice_name}.pt");
        log::info!("[models] ensuring kokoro voice: {voice_name}");
        let path = repo
            .get(&file_name)
            .map_err(|e| ModelError::Download(format!("{voice_name}: {e}")))?;
        log::info!("[models] kokoro voice ready: {}", path.display());
        Ok(path)
    }

    /// Ensure a Kokoro config.json is available locally.
    pub fn ensure_kokoro_config(&self) -> Result<PathBuf, ModelError> {
        let repo = self.api.model("hexgrad/Kokoro-82M".to_string());
        let path = repo
            .get("config.json")
            .map_err(|e| ModelError::Download(format!("config.json: {e}")))?;
        Ok(path)
    }

    /// Check if a local file exists at the given path. Convenience for
    /// callers that manage their own model paths.
    pub fn is_local(path: &std::path::Path) -> bool {
        path.exists()
    }
}

impl Default for ModelManager {
    fn default() -> Self {
        Self::new().expect("failed to initialize HuggingFace Hub API")
    }
}
