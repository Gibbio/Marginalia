//! Apple native STT via SFSpeechRecognizer.
//!
//! Uses a persistent Swift helper process that keeps the microphone open
//! and streams recognized text via stdout. The Rust side reads lines and
//! matches commands. Zero models to download — uses the Neural Engine.

use marginalia_core::ports::{
    CommandRecognition, CommandRecognizer, ProviderCapabilities, ProviderExecutionMode,
    SpeechInterruptCapture, SpeechInterruptMonitor,
};
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::sync::Once;
use std::time::Duration;

const PROVIDER_NAME: &str = "apple-stt";

static COMPILE_HELPER: Once = Once::new();

fn helper_path() -> PathBuf {
    std::env::temp_dir()
        .join("marginalia-stt-apple")
        .join("stt-helper")
}

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
            .stderr(Stdio::piped())
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

pub struct AppleCommandRecognizer {
    language: String,
    commands: Vec<String>,
}

impl AppleCommandRecognizer {
    pub fn new(language: &str, commands: Vec<String>) -> Result<Self, String> {
        ensure_helper()?;
        Ok(Self {
            language: language.to_string(),
            commands,
        })
    }
}

impl CommandRecognizer for AppleCommandRecognizer {
    fn describe_capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            provider_name: PROVIDER_NAME.to_string(),
            interface_kind: "command_stt".to_string(),
            supported_languages: vec![self.language.clone()],
            supports_streaming: true,
            supports_partial_results: true,
            supports_timestamps: false,
            low_latency_suitable: true,
            offline_capable: true,
            execution_mode: ProviderExecutionMode::Local,
        }
    }

    fn listen_for_command(&mut self) -> Option<CommandRecognition> {
        let capture = self.capture_interrupt(Some(4.0));
        let command = capture.recognized_command?;
        Some(CommandRecognition {
            command: command.clone(),
            provider_name: PROVIDER_NAME.to_string(),
            confidence: 1.0,
            is_final: true,
            raw_text: capture.raw_text,
        })
    }

    fn capture_interrupt(&mut self, _timeout_seconds: Option<f64>) -> SpeechInterruptCapture {
        SpeechInterruptCapture {
            provider_name: PROVIDER_NAME.to_string(),
            speech_detected: false,
            capture_ended_ms: 0,
            speech_detected_ms: None,
            capture_started_ms: None,
            raw_text: None,
            recognized_command: None,
            timed_out: true,
            input_device_index: None,
            input_device_name: None,
            sample_rate: None,
        }
    }

    fn open_interrupt_monitor(&mut self) -> Box<dyn SpeechInterruptMonitor> {
        match AppleInterruptMonitor::new(&self.language, self.commands.clone()) {
            Ok(m) => Box::new(m),
            Err(e) => {
                eprintln!("[apple-stt] monitor failed: {e}");
                Box::new(ErrorMonitor(e))
            }
        }
    }
}

/// Persistent monitor: starts the Swift helper once, keeps mic open,
/// reads recognized text from stdout line by line.
struct AppleInterruptMonitor {
    commands: Vec<String>,
    line_rx: mpsc::Receiver<String>,
    _child: Child,
}

impl AppleInterruptMonitor {
    fn new(language: &str, commands: Vec<String>) -> Result<Self, String> {
        let helper = ensure_helper()?;
        let mut child = Command::new(&helper)
            .arg(language)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .stdin(Stdio::null())
            .spawn()
            .map_err(|e| format!("spawn helper: {e}"))?;

        let stdout = child.stdout.take().ok_or("no stdout")?;
        let (tx, rx) = mpsc::channel();

        // Reader thread: sends each line of recognized text
        std::thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                match line {
                    Ok(text) => {
                        if tx.send(text).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        Ok(Self {
            commands,
            line_rx: rx,
            _child: child,
        })
    }
}

impl SpeechInterruptMonitor for AppleInterruptMonitor {
    fn capture_next_interrupt(&mut self, timeout_seconds: Option<f64>) -> SpeechInterruptCapture {
        let timeout = Duration::from_secs_f64(timeout_seconds.unwrap_or(4.0));

        // Wait for a line from the helper
        let text = match self.line_rx.recv_timeout(timeout) {
            Ok(t) => t.trim().to_string(),
            Err(_) => String::new(),
        };

        let command = if !text.is_empty() {
            self.commands
                .iter()
                .find(|cmd| text.to_lowercase().contains(&cmd.to_lowercase()))
                .cloned()
        } else {
            None
        };

        SpeechInterruptCapture {
            provider_name: PROVIDER_NAME.to_string(),
            speech_detected: !text.is_empty(),
            capture_ended_ms: 0,
            speech_detected_ms: if text.is_empty() { None } else { Some(0) },
            capture_started_ms: Some(0),
            raw_text: Some(text),
            recognized_command: command,
            timed_out: false,
            input_device_index: None,
            input_device_name: None,
            sample_rate: None,
        }
    }

    fn close(&mut self) {
        let _ = self._child.kill();
    }
}

impl Drop for AppleInterruptMonitor {
    fn drop(&mut self) {
        let _ = self._child.kill();
        let _ = self._child.wait();
    }
}

struct ErrorMonitor(String);

impl SpeechInterruptMonitor for ErrorMonitor {
    fn capture_next_interrupt(&mut self, _: Option<f64>) -> SpeechInterruptCapture {
        std::thread::sleep(Duration::from_secs(5));
        SpeechInterruptCapture {
            provider_name: PROVIDER_NAME.to_string(),
            speech_detected: false,
            capture_ended_ms: 0,
            speech_detected_ms: None,
            capture_started_ms: None,
            raw_text: Some(format!("error: {}", self.0)),
            recognized_command: None,
            timed_out: true,
            input_device_index: None,
            input_device_name: None,
            sample_rate: None,
        }
    }

    fn close(&mut self) {}
}

/// Swift helper: persistent process that keeps the mic open and prints
/// each recognized phrase on a new line to stdout. Runs until killed.
const SWIFT_HELPER_SOURCE: &str = r#"
import Foundation
import Speech
import AVFoundation

let language = CommandLine.arguments.count > 1 ? CommandLine.arguments[1] : "it-IT"

// Unbuffered stdout
setbuf(stdout, nil)

let semaphore = DispatchSemaphore(value: 0)

SFSpeechRecognizer.requestAuthorization { status in
    guard status == .authorized else {
        fputs("Speech recognition not authorized\n", stderr)
        exit(1)
    }
    semaphore.signal()
}
semaphore.wait()

let locale = Locale(identifier: language)
guard let recognizer = SFSpeechRecognizer(locale: locale), recognizer.isAvailable else {
    fputs("SFSpeechRecognizer not available for \(language)\n", stderr)
    exit(1)
}

if #available(macOS 13.0, *) {
    recognizer.supportsOnDeviceRecognition = true
}

let audioEngine = AVAudioEngine()
let inputNode = audioEngine.inputNode
let format = inputNode.outputFormat(forBus: 0)

var currentTask: SFSpeechRecognitionTask?

func startRecognition() {
    let request = SFSpeechAudioBufferRecognitionRequest()
    request.shouldReportPartialResults = true
    if #available(macOS 13.0, *) {
        request.requiresOnDeviceRecognition = true
    }

    inputNode.installTap(onBus: 0, bufferSize: 4096, format: format) { buffer, _ in
        request.append(buffer)
    }

    audioEngine.prepare()
    do { try audioEngine.start() } catch {
        fputs("Audio engine failed: \(error)\n", stderr)
        exit(1)
    }

    fputs("[apple-stt] listening...\n", stderr)

    var lastText = ""
    var silenceTimer: DispatchWorkItem?

    currentTask = recognizer.recognitionTask(with: request) { result, error in
        // Cancel any pending silence timer
        silenceTimer?.cancel()

        if let result = result {
            lastText = result.bestTranscription.formattedString

            if result.isFinal {
                // Final result — emit and restart
                if !lastText.isEmpty {
                    print(lastText)
                    fputs("[apple-stt] final: \(lastText)\n", stderr)
                }
                lastText = ""
                audioEngine.stop()
                inputNode.removeTap(onBus: 0)
                DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) {
                    startRecognition()
                }
                return
            }

            // Partial result — start silence timer (1.5s)
            // If no new partial arrives within 1.5s, emit what we have
            let timer = DispatchWorkItem {
                if !lastText.isEmpty {
                    print(lastText)
                    fputs("[apple-stt] timeout: \(lastText)\n", stderr)
                    lastText = ""
                }
                // Cancel and restart
                currentTask?.cancel()
                audioEngine.stop()
                inputNode.removeTap(onBus: 0)
                DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) {
                    startRecognition()
                }
            }
            silenceTimer = timer
            DispatchQueue.main.asyncAfter(deadline: .now() + 1.5, execute: timer)
        }

        if let error = error {
            fputs("[apple-stt] error: \(error.localizedDescription)\n", stderr)
            audioEngine.stop()
            inputNode.removeTap(onBus: 0)
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.5) {
                startRecognition()
            }
        }
    }
}

startRecognition()
dispatchMain()
"#;
