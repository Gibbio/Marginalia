use marginalia_core::ports::{
    CommandRecognition, CommandRecognizer, DictationSegment, DictationTranscript,
    DictationTranscriber, ProviderCapabilities, ProviderExecutionMode, SpeechInterruptCapture,
    SpeechInterruptMonitor,
};
use std::collections::VecDeque;

#[derive(Debug, Clone)]
pub struct FakeInterruptMonitor {
    captures: VecDeque<SpeechInterruptCapture>,
}

impl FakeInterruptMonitor {
    pub fn new(captures: Vec<SpeechInterruptCapture>) -> Self {
        Self {
            captures: VecDeque::from(captures),
        }
    }
}

impl SpeechInterruptMonitor for FakeInterruptMonitor {
    fn capture_next_interrupt(&mut self, _timeout_seconds: Option<f64>) -> SpeechInterruptCapture {
        self.captures.pop_front().unwrap_or_else(|| SpeechInterruptCapture {
            provider_name: "fake-command-stt".to_string(),
            speech_detected: false,
            capture_ended_ms: 0,
            speech_detected_ms: None,
            capture_started_ms: Some(0),
            recognized_command: None,
            raw_text: None,
            timed_out: true,
            input_device_index: None,
            input_device_name: None,
            sample_rate: None,
        })
    }

    fn close(&mut self) {}
}

#[derive(Debug, Clone)]
pub struct FakeCommandRecognizer {
    commands: VecDeque<CommandRecognition>,
    interrupts: Vec<SpeechInterruptCapture>,
}

impl Default for FakeCommandRecognizer {
    fn default() -> Self {
        Self {
            commands: VecDeque::new(),
            interrupts: Vec::new(),
        }
    }
}

impl FakeCommandRecognizer {
    pub fn new(commands: Vec<CommandRecognition>) -> Self {
        Self {
            commands: VecDeque::from(commands),
            interrupts: Vec::new(),
        }
    }

    pub fn with_interrupts(mut self, interrupts: Vec<SpeechInterruptCapture>) -> Self {
        self.interrupts = interrupts;
        self
    }
}

impl CommandRecognizer for FakeCommandRecognizer {
    fn describe_capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            provider_name: "fake-command-stt".to_string(),
            interface_kind: "command_stt".to_string(),
            supported_languages: vec!["it".to_string(), "en".to_string()],
            supports_streaming: false,
            supports_partial_results: false,
            supports_timestamps: false,
            low_latency_suitable: true,
            offline_capable: true,
            execution_mode: ProviderExecutionMode::Local,
        }
    }

    fn listen_for_command(&mut self) -> Option<CommandRecognition> {
        self.commands.pop_front()
    }

    fn capture_interrupt(&mut self, timeout_seconds: Option<f64>) -> SpeechInterruptCapture {
        self.open_interrupt_monitor()
            .capture_next_interrupt(timeout_seconds)
    }

    fn open_interrupt_monitor(&mut self) -> Box<dyn SpeechInterruptMonitor> {
        Box::new(FakeInterruptMonitor::new(self.interrupts.clone()))
    }
}

#[derive(Debug, Clone)]
pub struct FakeDictationTranscriber {
    transcript: DictationTranscript,
}

impl Default for FakeDictationTranscriber {
    fn default() -> Self {
        Self {
            transcript: DictationTranscript {
                text: "Deterministic fake transcript.".to_string(),
                provider_name: "fake-dictation".to_string(),
                language: "it".to_string(),
                is_final: true,
                segments: vec![DictationSegment {
                    text: "Deterministic fake transcript.".to_string(),
                    start_ms: 0,
                    end_ms: 1200,
                }],
                raw_text: Some("Deterministic fake transcript.".to_string()),
            },
        }
    }
}

impl FakeDictationTranscriber {
    pub fn new(transcript: DictationTranscript) -> Self {
        Self { transcript }
    }
}

impl DictationTranscriber for FakeDictationTranscriber {
    fn describe_capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            provider_name: "fake-dictation".to_string(),
            interface_kind: "dictation_stt".to_string(),
            supported_languages: vec!["it".to_string(), "en".to_string()],
            supports_streaming: false,
            supports_partial_results: false,
            supports_timestamps: true,
            low_latency_suitable: false,
            offline_capable: true,
            execution_mode: ProviderExecutionMode::Local,
        }
    }

    fn transcribe(
        &mut self,
        _session_id: Option<&str>,
        _note_id: Option<&str>,
    ) -> DictationTranscript {
        self.transcript.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::{FakeCommandRecognizer, FakeDictationTranscriber};
    use marginalia_core::ports::{
        CommandRecognition, CommandRecognizer, DictationTranscriber, SpeechInterruptCapture,
    };

    #[test]
    fn fake_command_recognizer_returns_scripted_commands() {
        let mut recognizer = FakeCommandRecognizer::new(vec![CommandRecognition {
            command: "pause".to_string(),
            provider_name: "fake-command-stt".to_string(),
            confidence: 1.0,
            is_final: true,
            raw_text: Some("pausa".to_string()),
        }]);

        let recognition = recognizer.listen_for_command().unwrap();
        assert_eq!(recognition.command, "pause");
        assert!(recognizer.listen_for_command().is_none());
    }

    #[test]
    fn fake_command_recognizer_can_capture_interrupts() {
        let mut recognizer = FakeCommandRecognizer::default().with_interrupts(vec![
            SpeechInterruptCapture {
                provider_name: "fake-command-stt".to_string(),
                speech_detected: true,
                capture_ended_ms: 250,
                speech_detected_ms: Some(100),
                capture_started_ms: Some(0),
                recognized_command: Some("stop".to_string()),
                raw_text: Some("ferma".to_string()),
                timed_out: false,
                input_device_index: None,
                input_device_name: None,
                sample_rate: None,
            },
        ]);

        let capture = recognizer.capture_interrupt(Some(1.0));
        assert_eq!(capture.recognized_command.as_deref(), Some("stop"));
    }

    #[test]
    fn fake_dictation_transcriber_returns_configured_transcript() {
        let mut transcriber = FakeDictationTranscriber::default();
        let transcript = transcriber.transcribe(Some("session-1"), Some("note-1"));

        assert_eq!(transcript.provider_name, "fake-dictation");
        assert!(transcript.is_final);
    }
}
