//! Apple native STT via SFSpeechRecognizer.
//!
//! Uses a compiled Swift helper that calls SFSpeechRecognizer on the Neural Engine.
//! Faster and more accurate than Whisper, zero models to download.
//! macOS 10.15+ only. Requires microphone permission.

use marginalia_core::ports::{
    CommandRecognition, CommandRecognizer, ProviderCapabilities, ProviderExecutionMode,
    SpeechInterruptCapture, SpeechInterruptMonitor,
};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Once;

const PROVIDER_NAME: &str = "apple-stt";

static COMPILE_HELPER: Once = Once::new();

/// Get path to the compiled Swift helper binary.
fn helper_path() -> PathBuf {
    let dir = std::env::temp_dir().join("marginalia-stt-apple");
    dir.join("stt-helper")
}

/// Compile the Swift helper if not already compiled.
fn ensure_helper() -> Result<PathBuf, String> {
    let path = helper_path();
    COMPILE_HELPER.call_once(|| {
        let dir = path.parent().unwrap();
        let _ = std::fs::create_dir_all(dir);
        let swift_src = dir.join("stt-helper.swift");
        std::fs::write(&swift_src, SWIFT_HELPER_SOURCE).expect("write swift source");
        let status = Command::new("swiftc")
            .args([
                "-O",
                "-o",
                path.to_str().unwrap(),
                swift_src.to_str().unwrap(),
                "-framework",
                "Speech",
                "-framework",
                "AVFoundation",
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        match status {
            Ok(s) if s.success() => {}
            _ => eprintln!("[apple-stt] Failed to compile Swift helper"),
        }
    });
    if path.exists() {
        Ok(path)
    } else {
        Err("Swift helper not compiled. Is Xcode installed?".to_string())
    }
}

/// Run the Swift helper to recognize speech for a given duration.
fn recognize(helper: &Path, language: &str, duration_secs: f64) -> Result<String, String> {
    let output = Command::new(helper)
        .args([language, &format!("{duration_secs:.1}")])
        .output()
        .map_err(|e| format!("helper failed: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("helper error: {stderr}"));
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub struct AppleCommandRecognizer {
    language: String,
    commands: Vec<String>,
    helper: PathBuf,
}

impl AppleCommandRecognizer {
    pub fn new(language: &str, commands: Vec<String>) -> Result<Self, String> {
        let helper = ensure_helper()?;
        Ok(Self {
            language: language.to_string(),
            commands,
            helper,
        })
    }

    fn match_command(&self, text: &str) -> Option<String> {
        let t = text.to_lowercase();
        self.commands
            .iter()
            .find(|cmd| t.contains(&cmd.to_lowercase()))
            .cloned()
    }
}

impl CommandRecognizer for AppleCommandRecognizer {
    fn describe_capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            provider_name: PROVIDER_NAME.to_string(),
            interface_kind: "command_stt".to_string(),
            supported_languages: vec![self.language.clone()],
            supports_streaming: false,
            supports_partial_results: false,
            supports_timestamps: false,
            low_latency_suitable: true,
            offline_capable: true,
            execution_mode: ProviderExecutionMode::Local,
        }
    }

    fn listen_for_command(&mut self) -> Option<CommandRecognition> {
        let text = recognize(&self.helper, &self.language, 4.0).ok()?;
        let command = self.match_command(&text)?;
        Some(CommandRecognition {
            command: command.clone(),
            provider_name: PROVIDER_NAME.to_string(),
            confidence: 1.0,
            is_final: true,
            raw_text: Some(text),
        })
    }

    fn capture_interrupt(&mut self, timeout_seconds: Option<f64>) -> SpeechInterruptCapture {
        let timeout = timeout_seconds.unwrap_or(4.0);
        let (text, error) = match recognize(&self.helper, &self.language, timeout) {
            Ok(t) => (t, None),
            Err(e) => (String::new(), Some(format!("error: {e}"))),
        };
        let command = self.match_command(&text);

        SpeechInterruptCapture {
            provider_name: PROVIDER_NAME.to_string(),
            speech_detected: !text.is_empty(),
            capture_ended_ms: 0,
            speech_detected_ms: if text.is_empty() { None } else { Some(0) },
            capture_started_ms: Some(0),
            raw_text: error.or(Some(text)),
            recognized_command: command,
            timed_out: false,
            input_device_index: None,
            input_device_name: None,
            sample_rate: None,
        }
    }

    fn open_interrupt_monitor(&mut self) -> Box<dyn SpeechInterruptMonitor> {
        Box::new(AppleInterruptMonitor {
            language: self.language.clone(),
            commands: self.commands.clone(),
            helper: self.helper.clone(),
        })
    }
}

struct AppleInterruptMonitor {
    language: String,
    commands: Vec<String>,
    helper: PathBuf,
}

impl SpeechInterruptMonitor for AppleInterruptMonitor {
    fn capture_next_interrupt(&mut self, timeout_seconds: Option<f64>) -> SpeechInterruptCapture {
        let timeout = timeout_seconds.unwrap_or(4.0);
        let (text, error) = match recognize(&self.helper, &self.language, timeout) {
            Ok(t) => (t, None),
            Err(e) => (String::new(), Some(format!("error: {e}"))),
        };
        let command = self
            .commands
            .iter()
            .find(|cmd| text.to_lowercase().contains(&cmd.to_lowercase()))
            .cloned();

        SpeechInterruptCapture {
            provider_name: PROVIDER_NAME.to_string(),
            speech_detected: !text.is_empty(),
            capture_ended_ms: 0,
            speech_detected_ms: if text.is_empty() { None } else { Some(0) },
            capture_started_ms: Some(0),
            raw_text: error.or(Some(text)),
            recognized_command: command,
            timed_out: false,
            input_device_index: None,
            input_device_name: None,
            sample_rate: None,
        }
    }

    fn close(&mut self) {}
}

/// Swift helper source. Compiled once at runtime.
/// Uses SFSpeechRecognizer with on-device recognition (Neural Engine).
const SWIFT_HELPER_SOURCE: &str = r#"
import Foundation
import Speech
import AVFoundation

let language = CommandLine.arguments.count > 1 ? CommandLine.arguments[1] : "it-IT"
let duration = CommandLine.arguments.count > 2 ? Double(CommandLine.arguments[2]) ?? 4.0 : 4.0

let semaphore = DispatchSemaphore(value: 0)
var resultText = ""

SFSpeechRecognizer.requestAuthorization { status in
    guard status == .authorized else {
        fputs("Speech recognition not authorized\n", stderr)
        exit(1)
    }

    let locale = Locale(identifier: language)
    guard let recognizer = SFSpeechRecognizer(locale: locale), recognizer.isAvailable else {
        fputs("SFSpeechRecognizer not available for \(language)\n", stderr)
        exit(1)
    }

    // Force on-device recognition (Neural Engine, no network)
    if #available(macOS 13.0, *) {
        recognizer.supportsOnDeviceRecognition = true
    }

    let audioEngine = AVAudioEngine()
    let request = SFSpeechAudioBufferRecognitionRequest()
    request.shouldReportPartialResults = true

    if #available(macOS 13.0, *) {
        request.requiresOnDeviceRecognition = true
    }

    let inputNode = audioEngine.inputNode
    let format = inputNode.outputFormat(forBus: 0)

    inputNode.installTap(onBus: 0, bufferSize: 1024, format: format) { buffer, _ in
        request.append(buffer)
    }

    audioEngine.prepare()
    do {
        try audioEngine.start()
    } catch {
        fputs("Audio engine failed: \(error)\n", stderr)
        exit(1)
    }

    var silenceTimer: Timer?
    var lastResultTime = Date()

    let task = recognizer.recognitionTask(with: request) { result, error in
        if let result = result {
            resultText = result.bestTranscription.formattedString
            lastResultTime = Date()
        }
        if error != nil || (result?.isFinal ?? false) {
            audioEngine.stop()
            inputNode.removeTap(onBus: 0)
            request.endAudio()
            semaphore.signal()
        }
    }

    // Stop after duration seconds
    DispatchQueue.main.asyncAfter(deadline: .now() + duration) {
        task.cancel()
        audioEngine.stop()
        inputNode.removeTap(onBus: 0)
        request.endAudio()
        semaphore.signal()
    }

    RunLoop.main.run(until: Date(timeIntervalSinceNow: duration + 1))
}

semaphore.wait()
print(resultText)
"#;
