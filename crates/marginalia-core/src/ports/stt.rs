use crate::ports::capabilities::ProviderCapabilities;

#[derive(Debug, Clone, PartialEq)]
pub struct CommandRecognition {
    pub command: String,
    pub provider_name: String,
    pub confidence: f64,
    pub is_final: bool,
    pub raw_text: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DictationSegment {
    pub text: String,
    pub start_ms: u32,
    pub end_ms: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DictationTranscript {
    pub text: String,
    pub provider_name: String,
    pub language: String,
    pub is_final: bool,
    pub segments: Vec<DictationSegment>,
    pub raw_text: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpeechInterruptCapture {
    pub provider_name: String,
    pub speech_detected: bool,
    pub capture_ended_ms: u32,
    pub speech_detected_ms: Option<u32>,
    pub capture_started_ms: Option<u32>,
    pub recognized_command: Option<String>,
    pub raw_text: Option<String>,
    pub timed_out: bool,
    pub input_device_index: Option<u32>,
    pub input_device_name: Option<String>,
    pub sample_rate: Option<u32>,
}

pub trait SpeechInterruptMonitor {
    fn capture_next_interrupt(
        &mut self,
        timeout_seconds: Option<f64>,
    ) -> SpeechInterruptCapture;
    fn close(&mut self);
}

pub trait CommandRecognizer {
    fn describe_capabilities(&self) -> ProviderCapabilities;
    fn listen_for_command(&mut self) -> Option<CommandRecognition>;
    fn capture_interrupt(&mut self, timeout_seconds: Option<f64>) -> SpeechInterruptCapture;
    fn open_interrupt_monitor(&mut self) -> Box<dyn SpeechInterruptMonitor>;
}

pub trait DictationTranscriber {
    fn describe_capabilities(&self) -> ProviderCapabilities;
    fn transcribe(
        &mut self,
        session_id: Option<&str>,
        note_id: Option<&str>,
    ) -> DictationTranscript;
}
