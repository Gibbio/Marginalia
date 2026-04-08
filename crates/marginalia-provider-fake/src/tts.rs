use marginalia_core::ports::{
    ProviderCapabilities, ProviderExecutionMode, SpeechSynthesizer, SynthesisRequest,
    SynthesisResult,
};
use std::collections::HashMap;

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

    fn synthesize(&mut self, request: SynthesisRequest) -> SynthesisResult {
        let voice = request
            .voice
            .clone()
            .unwrap_or_else(|| self.default_voice.clone());
        let excerpt = request.text.chars().take(80).collect::<String>();
        let mut metadata = HashMap::new();
        metadata.insert("language".to_string(), request.language);

        SynthesisResult {
            provider_name: self.provider_name.clone(),
            voice: voice.clone(),
            content_type: "audio/wav".to_string(),
            audio_reference: format!("memory://{}/{}", voice, excerpt.replace(' ', "_")),
            byte_length: excerpt.len(),
            text_excerpt: excerpt,
            metadata,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::FakeSpeechSynthesizer;
    use marginalia_core::ports::{SpeechSynthesizer, SynthesisRequest};

    #[test]
    fn fake_synthesizer_returns_deterministic_audio_reference() {
        let mut synthesizer = FakeSpeechSynthesizer::new();
        let result = synthesizer.synthesize(SynthesisRequest {
            text: "Alpha beta gamma".to_string(),
            voice: None,
            language: "it".to_string(),
        });

        assert_eq!(result.provider_name, "fake-tts");
        assert_eq!(result.voice, "narrator");
        assert!(result.audio_reference.starts_with("memory://"));
    }
}
