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

/// Bump when SWIFT_HELPER_SOURCE changes so the cached binary gets recompiled.
const HELPER_VERSION: u32 = 2;

static COMPILE_HELPER: Once = Once::new();

fn helper_path() -> PathBuf {
    std::env::temp_dir()
        .join("marginalia-stt-apple")
        .join(format!("stt-helper-v{HELPER_VERSION}"))
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
    silence_timeout: f64,
}

/// Joins commands with `|` for the helper CLI arg. Pipe is safe because trigger
/// words are alphanumeric/space (defined in user toml).
fn join_commands(commands: &[String]) -> String {
    commands.join("|")
}

impl AppleCommandRecognizer {
    pub fn new(
        language: &str,
        commands: Vec<String>,
        silence_timeout: f64,
    ) -> Result<Self, String> {
        let helper = ensure_helper()?;

        // Quick check: run helper for 0.5s to see if it errors immediately
        let output = Command::new(&helper)
            .arg(language)
            .arg(format!("{silence_timeout}"))
            .arg(join_commands(&commands))
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .and_then(|mut child| {
                std::thread::sleep(Duration::from_millis(500));
                let _ = child.kill();
                child.wait_with_output()
            })
            .map_err(|e| format!("helper check failed: {e}"))?;

        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("Siri and Dictation are disabled") {
            return Err("Apple STT requires macOS Dictation to be enabled. \
                 Enable in: System Settings → Keyboard → Dictation → ON"
                .to_string());
        }

        Ok(Self {
            language: language.to_string(),
            commands,
            silence_timeout,
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
        match AppleInterruptMonitor::new(
            &self.language,
            self.commands.clone(),
            self.silence_timeout,
        ) {
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
    fn new(
        language: &str,
        commands: Vec<String>,
        silence_timeout: f64,
    ) -> Result<Self, String> {
        let helper = ensure_helper()?;
        let mut child = Command::new(&helper)
            .arg(language)
            .arg(format!("{silence_timeout}"))
            .arg(join_commands(&commands))
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .stdin(Stdio::piped()) // kept open — helper exits when this closes
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
///
/// CLI: `stt-helper <language> <silence_timeout_seconds> <commands_pipe_separated>`
///
/// If a partial result already contains one of the trigger words, it is emitted
/// immediately (fast-path) instead of waiting for the silence timer.
const SWIFT_HELPER_SOURCE: &str = r#"
import Foundation
import Speech
import AVFoundation

let language = CommandLine.arguments.count > 1 ? CommandLine.arguments[1] : "it-IT"
let silenceTimeout = CommandLine.arguments.count > 2
    ? (Double(CommandLine.arguments[2]) ?? 0.8)
    : 0.8
let triggerWords: [String] = CommandLine.arguments.count > 3 && !CommandLine.arguments[3].isEmpty
    ? CommandLine.arguments[3].split(separator: "|").map { $0.lowercased() }
    : []

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

var currentRequest: SFSpeechAudioBufferRecognitionRequest?
var isRestarting = false

inputNode.installTap(onBus: 0, bufferSize: 4096, format: format) { buffer, _ in
    currentRequest?.append(buffer)
}

audioEngine.prepare()
do { try audioEngine.start() } catch {
    fputs("Audio engine failed: \(error)\n", stderr)
    exit(1)
}

func scheduleRestart() {
    guard !isRestarting else { return }
    isRestarting = true
    // End current request so the task can finalize
    currentRequest?.endAudio()
    currentRequest = nil
    DispatchQueue.main.asyncAfter(deadline: .now() + 0.3) {
        isRestarting = false
        startRecognitionTask()
    }
}

func containsTrigger(_ text: String) -> Bool {
    if triggerWords.isEmpty { return false }
    let lower = text.lowercased()
    return triggerWords.contains(where: { lower.contains($0) })
}

func startRecognitionTask() {
    let request = SFSpeechAudioBufferRecognitionRequest()
    request.shouldReportPartialResults = true
    if #available(macOS 13.0, *) {
        request.requiresOnDeviceRecognition = true
    }
    currentRequest = request

    var lastText = ""
    var emitted = false
    var silenceTimer: DispatchWorkItem?

    recognizer.recognitionTask(with: request) { result, error in
        silenceTimer?.cancel()

        if let result = result {
            lastText = result.bestTranscription.formattedString

            if result.isFinal {
                if !lastText.isEmpty && !emitted {
                    print(lastText)
                }
                lastText = ""
                emitted = false
                scheduleRestart()
                return
            }

            // Fast-path: if the current partial already contains a trigger word,
            // emit immediately and restart — no need to wait for silence.
            if !emitted && containsTrigger(lastText) {
                print(lastText)
                emitted = true
                scheduleRestart()
                return
            }

            // Silence timer: emit after `silenceTimeout` seconds of no new partials
            let timer = DispatchWorkItem {
                if !lastText.isEmpty && !emitted {
                    print(lastText)
                    emitted = true
                }
                scheduleRestart()
            }
            silenceTimer = timer
            DispatchQueue.main.asyncAfter(deadline: .now() + silenceTimeout, execute: timer)
        }

        if error != nil && !isRestarting {
            scheduleRestart()
        }
    }
}

// Monitor stdin — exit when parent process dies (pipe closes)
DispatchQueue.global().async {
    while let _ = readLine() {}
    // Parent closed stdin — clean exit
    audioEngine.stop()
    exit(0)
}

startRecognitionTask()
dispatchMain()
"#;
