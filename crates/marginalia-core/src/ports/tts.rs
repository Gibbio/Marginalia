use crate::ports::capabilities::ProviderCapabilities;
use std::collections::HashMap;
use std::error::Error;
use std::fmt::{Display, Formatter};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SynthesisRequest {
    pub text: String,
    pub voice: Option<String>,
    pub language: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SynthesisResult {
    pub provider_name: String,
    pub voice: String,
    pub content_type: String,
    pub audio_reference: String,
    pub byte_length: usize,
    pub text_excerpt: String,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SynthesisError {
    pub provider_name: String,
    pub message: String,
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

pub trait SpeechSynthesizer {
    fn describe_capabilities(&self) -> ProviderCapabilities;
    fn synthesize(&mut self, request: SynthesisRequest) -> Result<SynthesisResult, SynthesisError>;
}
