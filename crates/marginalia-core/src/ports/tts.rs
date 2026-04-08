use crate::ports::capabilities::ProviderCapabilities;
use std::collections::HashMap;

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

pub trait SpeechSynthesizer {
    fn describe_capabilities(&self) -> ProviderCapabilities;
    fn synthesize(&mut self, request: SynthesisRequest) -> SynthesisResult;
}
