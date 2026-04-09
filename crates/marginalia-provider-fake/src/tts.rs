use marginalia_core::ports::{
    ProviderCapabilities, ProviderExecutionMode, SpeechSynthesizer, SynthesisError, SynthesisRequest,
    SynthesisResult,
};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static AUDIO_COUNTER: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone)]
pub struct FakeSpeechSynthesizer {
    provider_name: String,
    default_voice: String,
}

impl Default for FakeSpeechSynthesizer {
    fn default() -> Self {
        Self {
            provider_name: "fake-tts".to_string(),
            default_voice: "narrator".to_string(),
        }
    }
}

impl FakeSpeechSynthesizer {
    pub fn new() -> Self {
        Self::default()
    }
}

impl SpeechSynthesizer for FakeSpeechSynthesizer {
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
        let voice = request
            .voice
            .clone()
            .unwrap_or_else(|| self.default_voice.clone());
        let excerpt = request.text.chars().take(80).collect::<String>();
        let audio_reference = write_temp_wav(&voice, &excerpt).unwrap_or_else(|| {
            format!("memory://{}/{}", voice, excerpt.replace(' ', "_"))
        });
        let mut metadata = HashMap::new();
        metadata.insert("language".to_string(), request.language);

        Ok(SynthesisResult {
            provider_name: self.provider_name.clone(),
            voice: voice.clone(),
            content_type: "audio/wav".to_string(),
            audio_reference,
            byte_length: excerpt.len(),
            text_excerpt: excerpt,
            metadata,
        })
    }
}

fn write_temp_wav(voice: &str, excerpt: &str) -> Option<String> {
    let id = AUDIO_COUNTER.fetch_add(1, Ordering::Relaxed);
    let safe_voice = sanitize_path_fragment(voice);
    let safe_excerpt = sanitize_path_fragment(excerpt);
    let path = std::env::temp_dir().join(format!(
        "marginalia-fake-tts-{id}-{safe_voice}-{safe_excerpt}.wav"
    ));
    let sample_count = (excerpt.chars().count() * 700).clamp(4_000, 64_000);
    write_silence_wav(&path, sample_count).ok()?;
    Some(path.display().to_string())
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
    rendered.trim_matches('_').chars().take(24).collect::<String>()
}

fn write_silence_wav(path: &PathBuf, sample_count: usize) -> std::io::Result<()> {
    let sample_rate = 16_000u32;
    let channels = 1u16;
    let bits_per_sample = 16u16;
    let bytes_per_sample = (bits_per_sample / 8) as usize;
    let data_size = sample_count * bytes_per_sample;
    let byte_rate = sample_rate * channels as u32 * bytes_per_sample as u32;
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
    bytes.extend_from_slice(&sample_rate.to_le_bytes());
    bytes.extend_from_slice(&byte_rate.to_le_bytes());
    bytes.extend_from_slice(&block_align.to_le_bytes());
    bytes.extend_from_slice(&bits_per_sample.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&(data_size as u32).to_le_bytes());
    bytes.resize(44 + data_size, 0);
    fs::write(path, bytes)
}

#[cfg(test)]
mod tests {
    use super::FakeSpeechSynthesizer;
    use marginalia_core::ports::{SpeechSynthesizer, SynthesisRequest};

    #[test]
    fn fake_synthesizer_returns_local_wav_reference() {
        let mut synthesizer = FakeSpeechSynthesizer::new();
        let result = synthesizer.synthesize(SynthesisRequest {
            text: "Alpha beta gamma".to_string(),
            voice: None,
            language: "it".to_string(),
        }).unwrap();

        assert_eq!(result.provider_name, "fake-tts");
        assert_eq!(result.voice, "narrator");
        assert!(result.audio_reference.ends_with(".wav"));
        assert!(std::path::Path::new(&result.audio_reference).exists());
    }
}
