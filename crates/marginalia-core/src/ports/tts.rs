use crate::ports::capabilities::ProviderCapabilities;
use std::collections::HashMap;
use std::error::Error;
use std::fmt::{Display, Formatter};

/// Request to synthesize speech from text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SynthesisRequest {
    /// The text to synthesize into speech.
    pub text: String,
    /// Voice identifier (e.g. "if_sara"). Uses provider default if None.
    pub voice: Option<String>,
    /// Language code for synthesis (e.g. "it", "en").
    pub language: String,
}

/// Successful result of a TTS synthesis operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SynthesisResult {
    /// Name of the TTS provider that produced this audio.
    pub provider_name: String,
    /// Voice identifier used for synthesis.
    pub voice: String,
    /// MIME type of the audio (e.g. "audio/wav").
    pub content_type: String,
    /// File path or URI where the synthesized audio is stored.
    pub audio_reference: String,
    /// Size of the audio data in bytes.
    pub byte_length: usize,
    /// Truncated excerpt of the source text for logging/display.
    pub text_excerpt: String,
    /// Provider-specific metadata (e.g. latency, phonemes used).
    pub metadata: HashMap<String, String>,
}

/// Error returned when TTS synthesis fails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SynthesisError {
    /// Name of the TTS provider that reported the error.
    pub provider_name: String,
    /// Human-readable error message.
    pub message: String,
    /// Provider-specific error metadata.
    pub metadata: HashMap<String, String>,
}

impl SynthesisError {
    pub fn new(provider_name: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            provider_name: provider_name.into(),
            message: message.into(),
            metadata: HashMap::new(),
        }
    }

    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}

impl Display for SynthesisError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.provider_name, self.message)
    }
}

impl Error for SynthesisError {}

/// Port for text-to-speech synthesis.
pub trait SpeechSynthesizer {
    fn describe_capabilities(&self) -> ProviderCapabilities;
    fn synthesize(&mut self, request: SynthesisRequest) -> Result<SynthesisResult, SynthesisError>;
}
