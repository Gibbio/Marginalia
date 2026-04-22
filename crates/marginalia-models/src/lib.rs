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
use hf_hub::api::Progress;
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

    /// Ensure a Kokoro MLX core weight file is available locally. Downloads
    /// from `prince-canuma/Kokoro-82M` — the safetensors mirror the MLX
    /// runtime consumes (hexgrad/Kokoro-82M ships `.pth` PyTorch files, which
    /// MLX can't load directly). Returns the HF-cache path to the file.
    ///
    /// Typical filename: `kokoro-v1_0.safetensors`.
    pub fn ensure_mlx_core(&self, file_name: &str) -> Result<PathBuf, ModelError> {
        let repo = self.api.model("prince-canuma/Kokoro-82M".to_string());
        log::info!("[models] ensuring MLX core: {file_name}");
        let path = repo
            .get(file_name)
            .map_err(|e| ModelError::Download(format!("{file_name}: {e}")))?;
        log::info!("[models] MLX core ready: {}", path.display());
        Ok(path)
    }

    /// Ensure an MLX voice embedding (`voices/{id}.safetensors`) is available
    /// locally. Same source as `ensure_mlx_core`. Returns the HF-cache path.
    ///
    /// Voice id format: `{lang}{gender}_{name}` — e.g. `if_sara` (Italian
    /// female, Sara), `im_nicola` (Italian male), `af_bella` (English female).
    pub fn ensure_mlx_voice(&self, voice_id: &str) -> Result<PathBuf, ModelError> {
        let repo = self.api.model("prince-canuma/Kokoro-82M".to_string());
        let file_name = format!("voices/{voice_id}.safetensors");
        log::info!("[models] ensuring MLX voice: {voice_id}");
        let path = repo
            .get(&file_name)
            .map_err(|e| ModelError::Download(format!("{voice_id}: {e}")))?;
        log::info!("[models] MLX voice ready: {}", path.display());
        Ok(path)
    }

    /// Check if a local file exists at the given path. Convenience for
    /// callers that manage their own model paths.
    pub fn is_local(path: &std::path::Path) -> bool {
        path.exists()
    }

    /// Download `file` from `repo` with a progress callback. Thin wrapper
    /// around `hf-hub`'s `ApiRepo::download_with_progress` so FFI callers
    /// don't have to pull `hf-hub` into their own dep graph just to
    /// implement the `Progress` trait.
    ///
    /// The callback runs on the download thread and is invoked:
    ///   • once on `init` with `(total_bytes, filename)`
    ///   • repeatedly on `update` with `(chunk_bytes)` (typically ~64 KB)
    ///   • once on `finish` with no args
    ///
    /// Return the resolved path to the cached file (same path semantics
    /// as `ensure_*`).
    pub fn download_with_progress<P: Progress>(
        &self,
        repo: &str,
        file: &str,
        progress: P,
    ) -> Result<PathBuf, ModelError> {
        let repo_handle = self.api.model(repo.to_string());
        repo_handle
            .download_with_progress(file, progress)
            .map_err(|e| ModelError::Download(format!("{file}: {e}")))
    }

    /// Remove a previously-downloaded file from the HF cache. Best-effort:
    /// the file may already be gone (e.g. the user cleared the cache
    /// manually) — that's not an error for the caller. Returns the path
    /// that was (or would have been) removed when found, `None` if the
    /// cache layout didn't match.
    ///
    /// The HF cache layout is
    /// `{HF_HOME}/hub/models--{author}--{name}/snapshots/{rev}/{file}`
    /// with a symlink through `blobs/…`. We rely on offline-mode `api.get`
    /// to resolve the current path and then unlink it. The cached
    /// `refs/{rev}` pointer is left alone — it's only a few bytes and
    /// re-download overwrites it.
    pub fn uninstall_from_repo(repo: &str, file: &str) -> Result<Option<PathBuf>, ModelError> {
        // Force offline while probing so we don't accidentally refetch.
        // Safe: single-threaded within the install worker.
        unsafe { std::env::set_var("HF_HUB_OFFLINE", "1") };
        let api = Api::new().map_err(|e| ModelError::Download(e.to_string()))?;
        let repo_handle = api.model(repo.to_string());
        match repo_handle.get(file) {
            Ok(path) => {
                // `path` is typically a symlink into blobs/. Delete both
                // the symlink and the blob target so the cache reclaims
                // real bytes.
                if let Ok(target) = std::fs::read_link(&path) {
                    let blob = path.parent().map(|p| p.join(target));
                    if let Some(blob) = blob {
                        let _ = std::fs::remove_file(&blob);
                    }
                }
                let _ = std::fs::remove_file(&path);
                Ok(Some(path))
            }
            Err(_) => Ok(None), // not cached — nothing to remove
        }
    }
}

impl Default for ModelManager {
    fn default() -> Self {
        Self::new().expect("failed to initialize HuggingFace Hub API")
    }
}
