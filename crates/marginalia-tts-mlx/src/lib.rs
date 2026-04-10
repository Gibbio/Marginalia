//! Kokoro TTS via MLX Metal GPU — macOS Apple Silicon only.
//!
//! Implements `SpeechSynthesizer` trait using voice-tts (mlx-rs backend)
//! with `enable_compile()` for Metal kernel fusion.
//!
//! Performance: ~1000ms for 164-char chunk (12x realtime) on M4.

use marginalia_core::ports::{
    ProviderCapabilities, ProviderExecutionMode, SpeechSynthesizer, SynthesisError,
    SynthesisRequest, SynthesisResult,
};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

static AUDIO_COUNTER: AtomicU64 = AtomicU64::new(1);

pub struct MlxSpeechSynthesizer {
    model: voice_tts::KokoroModel,
    voice: mlx_rs::Array,
    output_dir: PathBuf,
    default_voice: String,
}

impl MlxSpeechSynthesizer {
    /// Create a new MLX synthesizer.
    ///
    /// * `model_repo` - HuggingFace repo or local path (e.g. "prince-canuma/Kokoro-82M")
    /// * `voice_name` - Voice preset name (e.g. "af_bella")
    /// * `output_dir` - Directory for WAV output files
    pub fn new(
        model_repo: &str,
        voice_name: &str,
        output_dir: impl AsRef<Path>,
    ) -> Result<Self, String> {
        let model = voice_tts::load_model(model_repo)
            .map_err(|e| format!("failed to load Kokoro MLX model: {e}"))?;

        let voice = voice_tts::load_voice(voice_name, None)
            .map_err(|e| format!("failed to load voice '{voice_name}': {e}"))?;

        let output_dir = output_dir.as_ref().to_path_buf();
        fs::create_dir_all(&output_dir)
            .map_err(|e| format!("failed to create output dir: {e}"))?;

        Ok(Self {
            model,
            voice,
            output_dir,
            default_voice: voice_name.to_string(),
        })
    }
}

impl SpeechSynthesizer for MlxSpeechSynthesizer {
    fn describe_capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            provider_name: "kokoro-mlx".to_string(),
            interface_kind: "tts".to_string(),
            supported_languages: vec![
                "it".into(), "en".into(), "fr".into(), "de".into(),
                "es".into(), "pt".into(), "ja".into(), "ko".into(),
                "zh".into(), "hi".into(),
            ],
            supports_streaming: false,
            supports_partial_results: false,
            supports_timestamps: false,
            low_latency_suitable: true,
            offline_capable: true,
            execution_mode: ProviderExecutionMode::Local,
        }
    }

    fn synthesize(&mut self, request: SynthesisRequest) -> Result<SynthesisResult, SynthesisError> {
        let err = |msg: String| SynthesisError::new("kokoro-mlx", msg);

        // Phonemize with espeak-ng
        let phonemes = phonemize(&request.text, &request.language)
            .map_err(|e| err(format!("phonemization failed: {e}")))?;

        // Synthesize with MLX compile enabled
        mlx_rs::transforms::compile::enable_compile();
        let audio = voice_tts::generate(&mut self.model, &phonemes, &self.voice, 1.0)
            .map_err(|e| err(format!("synthesis failed: {e}")))?;
        mlx_rs::transforms::compile::disable_compile();

        // Convert to samples and eval
        audio.eval().map_err(|e| err(format!("eval failed: {e}")))?;
        let samples: &[f32] = audio.as_slice();

        // Write WAV
        let n = AUDIO_COUNTER.fetch_add(1, Ordering::Relaxed);
        let voice = request.voice.as_deref().unwrap_or(&self.default_voice);
        let wav_path = self.output_dir.join(format!("mlx-{voice}-{n}.wav"));
        write_wav_16(&wav_path, 24000, samples)
            .map_err(|e| err(format!("failed to write WAV: {e}")))?;

        let byte_length = wav_path.metadata().map(|m| m.len() as usize).unwrap_or(0);
        let text_excerpt = request.text.chars().take(50).collect();

        let mut metadata = HashMap::new();
        metadata.insert("language".to_string(), request.language);
        metadata.insert("phonemes".to_string(), phonemes);

        Ok(SynthesisResult {
            provider_name: "kokoro-mlx".to_string(),
            voice: voice.to_string(),
            content_type: "audio/wav".to_string(),
            audio_reference: wav_path.display().to_string(),
            byte_length,
            text_excerpt,
            metadata,
        })
    }
}

fn phonemize(text: &str, language: &str) -> Result<String, String> {
    let voice_flag = match language {
        "it" => "it",
        "en" => "en",
        "fr" => "fr",
        "de" => "de",
        "es" => "es",
        "pt" => "pt",
        _ => "en",
    };

    let mut child = Command::new("espeak-ng")
        .args(["-v", voice_flag, "--ipa", "-q"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("espeak-ng not found: {e}"))?;

    child
        .stdin
        .take()
        .unwrap()
        .write_all(text.as_bytes())
        .map_err(|e| format!("espeak-ng write failed: {e}"))?;

    let output = child
        .wait_with_output()
        .map_err(|e| format!("espeak-ng failed: {e}"))?;

    Ok(String::from_utf8_lossy(&output.stdout)
        .trim()
        .replace('\n', " "))
}

fn write_wav_16(path: &Path, sample_rate: u32, samples: &[f32]) -> std::io::Result<()> {
    let data_size = samples.len() * 2;
    let mut bytes = Vec::with_capacity(44 + data_size);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data_size as u32).to_le_bytes());
    bytes.extend_from_slice(b"WAVE");
    bytes.extend_from_slice(b"fmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&sample_rate.to_le_bytes());
    bytes.extend_from_slice(&(sample_rate * 2).to_le_bytes());
    bytes.extend_from_slice(&2u16.to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&(data_size as u32).to_le_bytes());
    for &s in samples {
        let pcm = (s.clamp(-1.0, 1.0) * i16::MAX as f32).round() as i16;
        bytes.extend_from_slice(&pcm.to_le_bytes());
    }
    fs::write(path, bytes)
}
