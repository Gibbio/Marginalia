//! Kokoro TTS via MLX Metal GPU — macOS Apple Silicon only.
//!
//! Implements `SpeechSynthesizer` trait using voice-tts (mlx-rs backend)
//! with `enable_compile()` for Metal kernel fusion.
//!
//! Performance: ~1000ms for 164-char chunk (12x realtime) on M4.

// MLX C API — memory management functions not yet exposed by mlx-rs.
// The symbols are already linked via mlx-sys; we just need the declaration.
extern "C" {
    fn mlx_clear_cache() -> std::ffi::c_int;
}

use marginalia_core::ports::{
    ProviderCapabilities, ProviderExecutionMode, SpeechSynthesizer, SynthesisError,
    SynthesisRequest, SynthesisResult,
};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
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

        // Prefer a voice file bundled alongside the local model (avoids HF network access).
        // Falls back to HF Hub only for builtin voices or if the local file is absent.
        let local_voice = Path::new(model_repo)
            .join("voices")
            .join(format!("{voice_name}.safetensors"));
        let voice = if local_voice.exists() {
            voice_tts::voice::load_voice_from_file(&local_voice)
                .map_err(|e| format!("failed to load voice '{voice_name}' from local path: {e}"))?
        } else {
            voice_tts::load_voice(voice_name, None)
                .map_err(|e| format!("failed to load voice '{voice_name}': {e}"))?
        };

        let output_dir = output_dir.as_ref().to_path_buf();
        fs::create_dir_all(&output_dir).map_err(|e| format!("failed to create output dir: {e}"))?;

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
                "it".into(),
                "en".into(),
                "fr".into(),
                "de".into(),
                "es".into(),
                "pt".into(),
                "ja".into(),
                "ko".into(),
                "zh".into(),
                "hi".into(),
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

        // Kokoro has a hard limit of 512 tokens. Each IPA character ≈ 1 token.
        // Reject over-long inputs early with a clear error rather than panicking
        // inside voice_tts::generate and poisoning the runtime Mutex.
        const MAX_PHONEME_TOKENS: usize = 505; // leave margin for BOS/EOS tokens
        if phonemes.len() > MAX_PHONEME_TOKENS {
            return Err(err(format!(
                "phoneme sequence too long ({} chars > {MAX_PHONEME_TOKENS} limit) — chunk is too large for Kokoro",
                phonemes.len()
            )));
        }

        // Synthesize with MLX compile enabled (fused Metal kernels).
        mlx_rs::transforms::compile::enable_compile();
        let audio = voice_tts::generate(&mut self.model, &phonemes, &self.voice, 1.0)
            .map_err(|e| err(format!("synthesis failed: {e}")))?;
        mlx_rs::transforms::compile::disable_compile();

        // Force evaluation, then release BOTH the JIT compilation cache AND the
        // Metal buffer pool. Without this, MLX holds onto several GB of unified
        // memory (GPU buffers that macOS reports as system memory pressure).
        audio.eval().map_err(|e| err(format!("eval failed: {e}")))?;
        let samples: &[f32] = audio.as_slice();

        // Write FLAC
        let n = AUDIO_COUNTER.fetch_add(1, Ordering::Relaxed);
        let voice = request.voice.as_deref().unwrap_or(&self.default_voice);
        let wav_path = self.output_dir.join(format!("mlx-{voice}-{n}.flac"));
        write_flac_16(&wav_path, 24000, samples)
            .map_err(|e| err(format!("failed to write FLAC: {e}")))?;

        // WAV is on disk — now release all MLX caches. This frees the Metal
        // buffer pool and the JIT compilation cache. The audio Array (and its
        // backing Metal buffer) is dropped when it goes out of scope above.
        drop(audio);
        mlx_rs::transforms::compile::clear_cache();
        unsafe { mlx_clear_cache(); }

        let byte_length = wav_path.metadata().map(|m| m.len() as usize).unwrap_or(0);
        let text_excerpt = request.text.chars().take(50).collect();

        let mut metadata = HashMap::new();
        metadata.insert("language".to_string(), request.language);
        metadata.insert("phonemes".to_string(), phonemes);

        Ok(SynthesisResult {
            provider_name: "kokoro-mlx".to_string(),
            voice: voice.to_string(),
            content_type: "audio/flac".to_string(),
            audio_reference: wav_path.display().to_string(),
            byte_length,
            text_excerpt,
            metadata,
        })
    }
}

/// Phonemize text by clause, preserving punctuation for Kokoro prosody.
///
/// espeak-ng strips punctuation from IPA output. Kokoro needs `,` `.` `?` `!`
/// in the phoneme stream for natural pauses and intonation.
///
/// Strategy: split text on clause boundaries (punctuation), phonemize each
/// clause in a single espeak-ng call, and re-insert the punctuation.
/// Works for any language espeak-ng supports — no language-specific code.
fn phonemize(text: &str, language: &str) -> Result<String, String> {
    // Normalize brackets/dashes to comma pauses before phonemization
    let text = normalize_pauses(text);

    let mut result = String::new();
    let mut clause_start = 0;

    for (i, ch) in text.char_indices() {
        if is_clause_punct(ch) {
            let clause = &text[clause_start..i];
            let clean = clause.trim();
            if !clean.is_empty() {
                let phonemes = espeak_ipa(clean, language)?;
                if !result.is_empty() && !phonemes.is_empty() {
                    result.push(' ');
                }
                result.push_str(&phonemes);
            }
            result.push(ch);
            clause_start = i + ch.len_utf8();
        }
    }

    // Remaining text after last punctuation
    let tail = text[clause_start..].trim();
    if !tail.is_empty() {
        let phonemes = espeak_ipa(tail, language)?;
        if !result.is_empty() && !phonemes.is_empty() {
            result.push(' ');
        }
        result.push_str(&phonemes);
    }

    Ok(result)
}

/// Normalize text before phonemization.
///
/// Based on misaki's EspeakG2P preprocessing (the official Kokoro G2P):
/// - Parentheses/brackets → commas (Kokoro pauses on commas)
/// - Dashes → commas
/// - Curly quotes normalized
/// - Multiple spaces collapsed
fn normalize_pauses(text: &str) -> String {
    text.replace('(', ", ")
        .replace(')', ", ")
        .replace('[', ", ")
        .replace(']', ", ")
        .replace('{', ", ")
        .replace('}', ", ")
        .replace(" — ", ", ")
        .replace(" – ", ", ")
        .replace("--", ", ")
        .replace("\",", "…") // closing quote + comma → ellipsis (longer pause)
        .replace('"', ", ") // other quotes → comma pause
        .replace('\u{201C}', ", ") // left double quote "
        .replace('\u{201D}', ", ") // right double quote "
        .replace('\u{00AB}', ", ") // «
        .replace('\u{00BB}', ", ") // »
        .replace('\u{2018}', "'") // left single quote
        .replace('\u{2019}', "'") // right single quote
        .replace("  ", " ")
        .replace(", ,", ",")
        .replace(",,", ",")
}

fn is_clause_punct(c: char) -> bool {
    matches!(
        c,
        '.' | ',' | '!' | '?' | ':' | ';' | '…'
        | '。' | '、' | '！' | '？' | '；' | '：'  // CJK
        | '¿' | '¡' // Spanish
    )
}

/// Clean espeak IPA output to match Kokoro's expected phoneme format.
/// Based on misaki's post-processing rules.
fn clean_ipa(ipa: &str) -> String {
    ipa.replace('^', "") // tie character
        .replace('\u{0329}', "") // combining vertical line below
        .replace('\u{032A}', "") // combining bridge below
        .replace('-', "") // hyphens in IPA
}

fn espeak_ipa(text: &str, language: &str) -> Result<String, String> {
    let clauses = espeak_rs::text_to_phonemes(text, language, None, false, false)
        .map_err(|e| format!("espeak-rs phonemization failed: {e}"))?;
    let raw = clauses.join(" ");
    Ok(clean_ipa(raw.trim()))
}

fn write_flac_16(path: &Path, sample_rate: u32, samples: &[f32]) -> std::io::Result<()> {
    use flacenc::bitsink::ByteSink;
    use flacenc::component::BitRepr;
    use flacenc::error::Verify;
    use flacenc::source::MemSource;

    let pcm: Vec<i32> = samples
        .iter()
        .map(|s| (s.clamp(-1.0, 1.0) * i16::MAX as f32).round() as i32)
        .collect();

    let source = MemSource::from_samples(&pcm, 1, 16, sample_rate as usize);

    let config = flacenc::config::Encoder::default()
        .into_verified()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("{e:?}")))?;

    let block_size = config.block_size;
    let stream = flacenc::encode_with_fixed_block_size(&config, source, block_size)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("{e:?}")))?;

    let mut sink = ByteSink::new();
    stream
        .write(&mut sink)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("{e:?}")))?;

    fs::write(path, sink.as_slice())
}
