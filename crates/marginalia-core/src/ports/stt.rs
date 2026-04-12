use crate::ports::capabilities::ProviderCapabilities;

/// A recognized voice command from the STT engine.
#[derive(Debug, Clone, PartialEq)]
pub struct CommandRecognition {
    /// The matched command string (e.g. "avanti", "pausa").
    pub command: String,
    /// Name of the STT provider that produced this recognition.
    pub provider_name: String,
    /// Recognition confidence score, 0.0 to 1.0.
    pub confidence: f64,
    /// Whether this is a final (committed) recognition or an interim result.
    pub is_final: bool,
    /// Raw transcript text before command matching, if available.
    pub raw_text: Option<String>,
}

/// A time-aligned segment within a dictation transcript.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DictationSegment {
    /// Transcribed text for this segment.
    pub text: String,
    /// Start time in milliseconds from the beginning of the audio.
    pub start_ms: u32,
    /// End time in milliseconds from the beginning of the audio.
    pub end_ms: u32,
}

/// Full transcript from a dictation session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DictationTranscript {
    /// The complete transcribed text.
    pub text: String,
    /// Name of the STT provider that produced this transcript.
    pub provider_name: String,
    /// BCP-47 language code of the transcription (e.g. "it", "en").
    pub language: String,
    /// Whether this is the final transcript or an interim result.
    pub is_final: bool,
    /// Time-aligned segments within the transcript.
    pub segments: Vec<DictationSegment>,
    /// Raw transcript before post-processing, if available.
    pub raw_text: Option<String>,
}

/// Result of monitoring the microphone for a speech interrupt during playback.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpeechInterruptCapture {
    /// Name of the STT provider that performed the capture.
    pub provider_name: String,
    /// Whether speech was detected during the capture window.
    pub speech_detected: bool,
    /// Timestamp (ms) when the capture ended.
    pub capture_ended_ms: u32,
    /// Timestamp (ms) when speech was first detected, if any.
    pub speech_detected_ms: Option<u32>,
    /// Timestamp (ms) when the capture started, if available.
    pub capture_started_ms: Option<u32>,
    /// Command recognized from the speech, if any.
    pub recognized_command: Option<String>,
    /// Raw transcript text, if available.
    pub raw_text: Option<String>,
    /// Whether the capture ended due to timeout rather than speech.
    pub timed_out: bool,
    /// Index of the audio input device used.
    pub input_device_index: Option<u32>,
    /// Name of the audio input device used.
    pub input_device_name: Option<String>,
    /// Sample rate of the captured audio in Hz.
    pub sample_rate: Option<u32>,
}

/// Monitors the microphone for speech interrupts during playback.
pub trait SpeechInterruptMonitor: Send {
    fn capture_next_interrupt(&mut self, timeout_seconds: Option<f64>) -> SpeechInterruptCapture;
    fn close(&mut self);
}

/// Recognizes discrete voice commands from the microphone.
pub trait CommandRecognizer {
    fn describe_capabilities(&self) -> ProviderCapabilities;
    fn listen_for_command(&mut self) -> Option<CommandRecognition>;
    fn capture_interrupt(&mut self, timeout_seconds: Option<f64>) -> SpeechInterruptCapture;
    fn open_interrupt_monitor(&mut self) -> Box<dyn SpeechInterruptMonitor>;
}

/// Transcribes free-form speech for voice notes.
pub trait DictationTranscriber {
    fn describe_capabilities(&self) -> ProviderCapabilities;
    fn transcribe(
        &mut self,
        session_id: Option<&str>,
        note_id: Option<&str>,
    ) -> DictationTranscript;
}

/// Output of an STT engine factory: a matched pair of command recognizer and
/// dictation transcriber that share the same underlying engine (same mic
/// stream, same model, same process). The factory pattern ensures the two
/// sides are initialized together and can share resources.
pub struct SttEngineOutput {
    /// The command recognizer half of the engine pair.
    pub command_recognizer: Box<dyn CommandRecognizer + Send>,
    /// The dictation transcriber half of the engine pair.
    pub dictation_transcriber: Box<dyn DictationTranscriber + Send>,
    /// Human-readable label for the engine (e.g. "whisper", "apple").
    pub engine_label: String,
}
