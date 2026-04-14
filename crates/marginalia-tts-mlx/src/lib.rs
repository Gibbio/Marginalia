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

        // Phonemize — may return multiple pieces if the total phoneme count
        // would exceed Kokoro's ~505-token budget (common with number-heavy
        // text like annual reports: "2025" → "duemilaventicinque" inflates
        // phoneme count well beyond the char-count heuristic used by the
        // chunker in marginalia-core).
        let pieces = phonemize(&request.text, &request.language)
            .map_err(|e| err(format!("phonemization failed: {e}")))?;

        if pieces.is_empty() {
            return Err(err("phonemization produced no output".to_string()));
        }

        // Safety net: if a single clause still exceeds the budget after the
        // greedy split there is nothing left to divide — surface the error
        // instead of panicking inside voice_tts::generate.
        for piece in &pieces {
            if piece.len() > MAX_PHONEME_TOKENS {
                return Err(err(format!(
                    "phoneme piece too long ({} chars > {MAX_PHONEME_TOKENS} limit) — a single clause cannot be split",
                    piece.len()
                )));
            }
        }

        if pieces.len() > 1 {
            log::info!(
                "tts-mlx: chunk phonemized into {} pieces (over {}-phoneme budget) — auto-split",
                pieces.len(),
                MAX_PHONEME_TOKENS
            );
        }

        // Generate audio for each piece with MLX compile enabled (fused Metal
        // kernels) and concatenate the PCM samples into one buffer.
        let mut all_samples: Vec<f32> = Vec::new();
        for piece in &pieces {
            mlx_rs::transforms::compile::enable_compile();
            let audio = voice_tts::generate(&mut self.model, piece, &self.voice, 1.0)
                .map_err(|e| err(format!("synthesis failed: {e}")))?;
            mlx_rs::transforms::compile::disable_compile();

            audio.eval().map_err(|e| err(format!("eval failed: {e}")))?;
            all_samples.extend_from_slice(audio.as_slice());
            drop(audio);
        }

        // Write FLAC
        let n = AUDIO_COUNTER.fetch_add(1, Ordering::Relaxed);
        let voice = request.voice.as_deref().unwrap_or(&self.default_voice);
        let wav_path = self.output_dir.join(format!("mlx-{voice}-{n}.flac"));
        write_flac_16(&wav_path, 24000, &all_samples)
            .map_err(|e| err(format!("failed to write FLAC: {e}")))?;

        // FLAC is on disk — release the MLX JIT cache AND the Metal buffer
        // pool so MLX doesn't hold onto several GB of unified memory between
        // synthesis calls.
        mlx_rs::transforms::compile::clear_cache();
        unsafe {
            mlx_clear_cache();
        }

        let byte_length = wav_path.metadata().map(|m| m.len() as usize).unwrap_or(0);
        let text_excerpt = request.text.chars().take(50).collect();

        let mut metadata = HashMap::new();
        metadata.insert("language".to_string(), request.language);
        metadata.insert("phonemes".to_string(), pieces.join(" "));

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

// Kokoro has a hard limit of 512 tokens per inference call. Each IPA character
// ≈ 1 token; leave a small margin for BOS/EOS.
const MAX_PHONEME_TOKENS: usize = 505;

/// Phonemize text by clause, preserving punctuation for Kokoro prosody.
///
/// espeak-ng strips punctuation from IPA output. Kokoro needs `,` `.` `?` `!`
/// in the phoneme stream for natural pauses and intonation.
///
/// Strategy: split text on clause boundaries (punctuation), phonemize each
/// clause in a single espeak-ng call, and re-insert the punctuation.
/// Works for any language espeak-ng supports — no language-specific code.
///
/// Returns one or more pieces, each guaranteed not to exceed
/// `MAX_PHONEME_TOKENS`. Splits happen at clause boundaries when the running
/// accumulator would overflow; callers synthesize each piece separately and
/// concatenate the resulting audio.
fn phonemize(text: &str, language: &str) -> Result<Vec<String>, String> {
    let text = normalize_pauses(text);
    let bytes = text.as_bytes();

    let mut pieces: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut clause_start = 0;

    // Flush `current` into `pieces` if adding `addition` more chars would
    // overflow the phoneme budget. `addition` already includes any separator.
    let try_flush = |current: &mut String, pieces: &mut Vec<String>, addition: usize| {
        if !current.is_empty() && current.len() + addition > MAX_PHONEME_TOKENS {
            pieces.push(std::mem::take(current));
        }
    };

    for (i, ch) in text.char_indices() {
        if is_clause_punct(ch) {
            // Don't split on '.' or ',' between ASCII digits: they are either
            // decimal separators or thousands separators depending on locale
            // (IT: "2,5" / "1.000.000"; EN: "2.5" / "1,000,000"). espeak-ng
            // already knows the language rules and expands them correctly as
            // long as the number reaches it as a single token.
            if ch == ',' || ch == '.' {
                let before = i.checked_sub(1).map(|j| bytes[j]).unwrap_or(0);
                let after = bytes.get(i + 1).copied().unwrap_or(0);
                if before.is_ascii_digit() && after.is_ascii_digit() {
                    continue;
                }
            }

            let clean = text[clause_start..i].trim();
            if !clean.is_empty() {
                let phonemes = espeak_ipa(clean, language)?;
                let separator = if current.is_empty() { 0 } else { 1 };
                // +1 accounts for the punctuation byte we push below.
                try_flush(&mut current, &mut pieces, separator + phonemes.len() + 1);
                if !current.is_empty() && !phonemes.is_empty() {
                    current.push(' ');
                }
                current.push_str(&phonemes);
            }
            current.push(ch);
            clause_start = i + ch.len_utf8();
        }
    }

    let tail = text[clause_start..].trim();
    if !tail.is_empty() {
        let phonemes = espeak_ipa(tail, language)?;
        let separator = if current.is_empty() { 0 } else { 1 };
        try_flush(&mut current, &mut pieces, separator + phonemes.len());
        if !current.is_empty() && !phonemes.is_empty() {
            current.push(' ');
        }
        current.push_str(&phonemes);
    }

    if !current.is_empty() {
        pieces.push(current);
    }

    Ok(pieces)
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
