//! Apple native STT via SFSpeechRecognizer.
//!
//! A single Swift helper process serves BOTH command recognition and note
//! dictation. The Rust side switches modes by writing `MODE COMMAND` /
//! `MODE DICTATION` lines to the helper's stdin; the helper applies the new
//! mode (different silence timeouts, fast-path on triggers vs accumulate, and
//! result framing) and routes its stdout output back through two channels:
//!
//!   `CMD <text>`        — recognized command-mode utterance
//!   `DICT_END <text>`   — finalized dictation-mode utterance
//!
//! Both Rust consumers (`AppleCommandRecognizer` and
//! `AppleDictationTranscriber`) share the same child process via
//! `Arc<AppleHelperShared>`, so there is exactly one Swift process and one
//! microphone stream open per session.

use marginalia_core::ports::{
    CommandRecognition, CommandRecognizer, DictationSegment, DictationTranscriber,
    DictationTranscript, ProviderCapabilities, ProviderExecutionMode, SpeechInterruptCapture,
    SpeechInterruptMonitor,
};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{mpsc, Arc, Mutex, Once};
use std::time::Duration;

const COMMAND_PROVIDER_NAME: &str = "apple-stt";
const DICTATION_PROVIDER_NAME: &str = "apple-dictation-stt";

/// Bump when SWIFT_HELPER_SOURCE changes so the cached binary gets recompiled.
const HELPER_VERSION: u32 = 4;

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
                "-framework",
                "AudioToolbox",
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .status();
        match status {
            Ok(s) if s.success() => {}
            _ => log::error!("[apple-stt] failed to compile Swift helper"),
        }
    });
    if path.exists() {
        Ok(path)
    } else {
        Err("Swift helper not compiled. Is Xcode installed?".to_string())
    }
}

/// Joins commands with `|` for the helper CLI arg. Pipe is safe because
/// trigger words are alphanumeric/space.
fn join_commands(commands: &[String]) -> String {
    commands.join("|")
}

// =============================================================================
// Shared helper handle
// =============================================================================

/// Shared state for the running Swift helper. Held by both the command
/// recognizer and the dictation transcriber via `Arc`. Provides a serialized
/// stdin write path used to send mode-switch commands.
pub struct AppleHelperShared {
    stdin: Mutex<ChildStdin>,
    child: Mutex<Child>,
}

impl AppleHelperShared {
    fn write_line(&self, line: &str) -> Result<(), String> {
        let mut stdin = self.stdin.lock().unwrap();
        stdin
            .write_all(line.as_bytes())
            .and_then(|_| stdin.write_all(b"\n"))
            .and_then(|_| stdin.flush())
            .map_err(|e| format!("write to helper stdin: {e}"))
    }

    fn switch_to_command(&self) -> Result<(), String> {
        self.write_line("MODE COMMAND")
    }

    fn switch_to_dictation(&self) -> Result<(), String> {
        self.write_line("MODE DICTATION")
    }
}

impl Drop for AppleHelperShared {
    fn drop(&mut self) {
        let mut child = self.child.lock().unwrap();
        let _ = child.kill();
        let _ = child.wait();
    }
}

// =============================================================================
// Constructor
// =============================================================================

/// Spawn the Swift helper and produce paired command/dictation handles. The
/// helper runs in COMMAND mode by default; the dictation transcriber switches
/// to DICTATION mode on demand and back when its `transcribe` returns.
pub fn new_apple_stt(
    language: &str,
    commands: Vec<String>,
    cmd_silence_timeout: f64,
    dict_silence_timeout: f64,
    dict_max_seconds: f64,
) -> Result<(AppleCommandRecognizer, AppleDictationTranscriber), String> {
    let helper = ensure_helper()?;

    // Smoke test: run the helper for 0.5s to surface immediate setup errors
    // such as Dictation being disabled in System Settings.
    let smoke = Command::new(&helper)
        .arg(language)
        .arg(format!("{cmd_silence_timeout}"))
        .arg(format!("{dict_silence_timeout}"))
        .arg(join_commands(&commands))
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .stdin(Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            std::thread::sleep(Duration::from_millis(500));
            let _ = child.kill();
            child.wait_with_output()
        })
        .map_err(|e| format!("helper smoke test failed: {e}"))?;

    let stderr = String::from_utf8_lossy(&smoke.stderr);
    if stderr.contains("Siri and Dictation are disabled") {
        return Err("Apple STT requires macOS Dictation to be enabled. \
             Enable in: System Settings → Keyboard → Dictation → ON"
            .to_string());
    }

    // Spawn the persistent helper.
    let mut child = Command::new(&helper)
        .arg(language)
        .arg(format!("{cmd_silence_timeout}"))
        .arg(format!("{dict_silence_timeout}"))
        .arg(join_commands(&commands))
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|e| format!("spawn helper: {e}"))?;

    let stdout = child.stdout.take().ok_or("no stdout")?;
    let stdin = child.stdin.take().ok_or("no stdin")?;

    let (cmd_tx, cmd_rx) = mpsc::channel::<String>();
    let (dict_tx, dict_rx) = mpsc::channel::<String>();

    // Reader thread: routes each line into the right channel based on prefix.
    std::thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.lines() {
            let Ok(text) = line else { break };
            let trimmed = text.trim_end();
            if let Some(rest) = trimmed.strip_prefix("CMD ") {
                if cmd_tx.send(rest.to_string()).is_err() {
                    break;
                }
            } else if let Some(rest) = trimmed.strip_prefix("DICT_END ") {
                if dict_tx.send(rest.to_string()).is_err() {
                    break;
                }
            } else if trimmed == "DICT_END" {
                if dict_tx.send(String::new()).is_err() {
                    break;
                }
            }
        }
    });

    let shared = Arc::new(AppleHelperShared {
        stdin: Mutex::new(stdin),
        child: Mutex::new(child),
    });

    let recognizer = AppleCommandRecognizer {
        language: language.to_string(),
        commands: commands.clone(),
        helper: shared.clone(),
        cmd_rx: Mutex::new(Some(cmd_rx)),
    };

    let transcriber = AppleDictationTranscriber {
        language: language.to_string(),
        helper: shared,
        dict_rx: Mutex::new(dict_rx),
        max_duration: Duration::from_secs_f64(dict_max_seconds),
    };

    Ok((recognizer, transcriber))
}

// =============================================================================
// Command recognizer
// =============================================================================

pub struct AppleCommandRecognizer {
    language: String,
    commands: Vec<String>,
    helper: Arc<AppleHelperShared>,
    cmd_rx: Mutex<Option<mpsc::Receiver<String>>>,
}

impl CommandRecognizer for AppleCommandRecognizer {
    fn describe_capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            provider_name: COMMAND_PROVIDER_NAME.to_string(),
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
            command,
            provider_name: COMMAND_PROVIDER_NAME.to_string(),
            confidence: 1.0,
            is_final: true,
            raw_text: capture.raw_text,
        })
    }

    fn capture_interrupt(&mut self, _timeout_seconds: Option<f64>) -> SpeechInterruptCapture {
        // Not used by the runtime — open_interrupt_monitor() is the hot path.
        SpeechInterruptCapture {
            provider_name: COMMAND_PROVIDER_NAME.to_string(),
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
        let cmd_rx = self
            .cmd_rx
            .lock()
            .unwrap()
            .take()
            .expect("apple-stt: command monitor opened more than once");
        Box::new(AppleInterruptMonitor {
            commands: self.commands.clone(),
            cmd_rx,
            _helper: self.helper.clone(),
        })
    }
}

struct AppleInterruptMonitor {
    commands: Vec<String>,
    cmd_rx: mpsc::Receiver<String>,
    _helper: Arc<AppleHelperShared>,
}

impl SpeechInterruptMonitor for AppleInterruptMonitor {
    fn capture_next_interrupt(&mut self, timeout_seconds: Option<f64>) -> SpeechInterruptCapture {
        let timeout = Duration::from_secs_f64(timeout_seconds.unwrap_or(4.0));

        let text = match self.cmd_rx.recv_timeout(timeout) {
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
            provider_name: COMMAND_PROVIDER_NAME.to_string(),
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
        // The Swift helper is shared with the dictation transcriber via the
        // Arc — closing here would race. Cleanup happens when the last Arc is dropped.
    }
}

// =============================================================================
// Dictation transcriber
// =============================================================================

pub struct AppleDictationTranscriber {
    language: String,
    helper: Arc<AppleHelperShared>,
    dict_rx: Mutex<mpsc::Receiver<String>>,
    max_duration: Duration,
}

impl DictationTranscriber for AppleDictationTranscriber {
    fn describe_capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            provider_name: DICTATION_PROVIDER_NAME.to_string(),
            interface_kind: "dictation_stt".to_string(),
            supported_languages: vec![self.language.clone()],
            supports_streaming: false,
            supports_partial_results: false,
            supports_timestamps: false,
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
        let result = (|| -> Result<String, String> {
            // Drain any stale dictation lines that may have arrived between
            // sessions (e.g. a delayed DICT_END from a previous timeout).
            {
                let dict_rx = self.dict_rx.lock().unwrap();
                while dict_rx.try_recv().is_ok() {}
            }

            self.helper.switch_to_dictation()?;

            let dict_rx = self.dict_rx.lock().unwrap();
            let text = dict_rx
                .recv_timeout(self.max_duration)
                .map_err(|_| "dictation timed out".to_string())?;
            Ok(text.trim().to_string())
        })();

        // Always switch back to command mode so the monitor resumes catching
        // commands, even if dictation errored out.
        if let Err(e) = self.helper.switch_to_command() {
            log::warn!("[apple-stt] mode switch back to command failed: {e}");
        }

        match result {
            Ok(text) => DictationTranscript {
                text: text.clone(),
                provider_name: DICTATION_PROVIDER_NAME.to_string(),
                language: self.language.clone(),
                is_final: true,
                segments: vec![DictationSegment {
                    text,
                    start_ms: 0,
                    end_ms: 0,
                }],
                raw_text: None,
            },
            Err(err) => DictationTranscript {
                text: format!("[Apple dictation error: {err}]"),
                provider_name: DICTATION_PROVIDER_NAME.to_string(),
                language: self.language.clone(),
                is_final: true,
                segments: vec![],
                raw_text: None,
            },
        }
    }
}

// =============================================================================
// Swift helper source
// =============================================================================

/// Persistent Swift helper. Reads stdin for `MODE COMMAND` / `MODE DICTATION`
/// lines and switches the active recognition profile accordingly. Output
/// lines are prefixed so the Rust side can route them to the right channel.
///
/// CLI: `stt-helper <language> <cmd_silence> <dict_silence> <triggers_pipe_separated>`
const SWIFT_HELPER_SOURCE: &str = r#"
import Foundation
import Speech
import AVFoundation
import AudioToolbox

let language = CommandLine.arguments.count > 1 ? CommandLine.arguments[1] : "it-IT"
let cmdSilenceTimeout = CommandLine.arguments.count > 2
    ? (Double(CommandLine.arguments[2]) ?? 0.8)
    : 0.8
let dictSilenceTimeout = CommandLine.arguments.count > 3
    ? (Double(CommandLine.arguments[3]) ?? 1.5)
    : 1.5
let triggerWords: [String] = CommandLine.arguments.count > 4 && !CommandLine.arguments[4].isEmpty
    ? CommandLine.arguments[4].split(separator: "|").map { $0.lowercased() }
    : []

setbuf(stdout, nil)

// Mode state. All mutations happen on the main queue.
enum HelperMode { case command; case dictation }
var currentMode: HelperMode = .command

// System feedback sounds for dictation start/end. Loaded lazily from the
// built-in macOS sound library; silent fallback if loading fails.
var dictationStartSoundID: SystemSoundID = 0
var dictationEndSoundID: SystemSoundID = 0
let startSoundURL = URL(fileURLWithPath: "/System/Library/Sounds/Tink.aiff")
let endSoundURL = URL(fileURLWithPath: "/System/Library/Sounds/Pop.aiff")
AudioServicesCreateSystemSoundID(startSoundURL as CFURL, &dictationStartSoundID)
AudioServicesCreateSystemSoundID(endSoundURL as CFURL, &dictationEndSoundID)

func playDictationStartBeep() {
    if dictationStartSoundID != 0 {
        AudioServicesPlaySystemSound(dictationStartSoundID)
    }
}

func playDictationEndBeep() {
    if dictationEndSoundID != 0 {
        AudioServicesPlaySystemSound(dictationEndSoundID)
    }
}

// Authorization
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
var silenceTimer: DispatchWorkItem?

inputNode.installTap(onBus: 0, bufferSize: 4096, format: format) { buffer, _ in
    currentRequest?.append(buffer)
}

audioEngine.prepare()
do { try audioEngine.start() } catch {
    fputs("Audio engine failed: \(error)\n", stderr)
    exit(1)
}

func containsTrigger(_ text: String) -> Bool {
    if triggerWords.isEmpty { return false }
    let lower = text.lowercased()
    return triggerWords.contains(where: { lower.contains($0) })
}

func emit(_ text: String, mode: HelperMode) {
    if text.isEmpty { return }
    if mode == .command {
        print("CMD \(text)")
    } else {
        print("DICT_END \(text)")
    }
}

func scheduleRestart() {
    silenceTimer?.cancel()
    silenceTimer = nil
    guard !isRestarting else { return }
    isRestarting = true
    currentRequest?.endAudio()
    currentRequest = nil
    DispatchQueue.main.asyncAfter(deadline: .now() + 0.3) {
        isRestarting = false
        startRecognitionTask()
    }
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

    recognizer.recognitionTask(with: request) { result, error in
        silenceTimer?.cancel()
        silenceTimer = nil

        // Mode is read live so a switch that happens during recognition is
        // honored on the next callback / timer firing.
        let mode = currentMode

        if let result = result {
            lastText = result.bestTranscription.formattedString

            if result.isFinal {
                if !emitted {
                    emit(lastText, mode: mode)
                }
                lastText = ""
                emitted = false
                scheduleRestart()
                return
            }

            // Fast-path: only in command mode.
            if mode == .command && !emitted && containsTrigger(lastText) {
                emit(lastText, mode: .command)
                emitted = true
                scheduleRestart()
                return
            }

            // Silence timer: longer in dictation mode so multi-clause notes
            // aren't truncated mid-sentence.
            let timeout = (mode == .command) ? cmdSilenceTimeout : dictSilenceTimeout
            let textSnapshot = lastText
            let timer = DispatchWorkItem {
                let liveMode = currentMode
                if !emitted {
                    emit(textSnapshot, mode: liveMode)
                    emitted = true
                }
                scheduleRestart()
            }
            silenceTimer = timer
            DispatchQueue.main.asyncAfter(deadline: .now() + timeout, execute: timer)
        }

        if error != nil && !isRestarting {
            scheduleRestart()
        }
    }
}

// Stdin reader: handles MODE commands and exits when parent closes the pipe.
DispatchQueue.global().async {
    while let line = readLine() {
        let trimmed = line.trimmingCharacters(in: .whitespaces)
        switch trimmed {
        case "MODE COMMAND":
            DispatchQueue.main.async {
                if currentMode == .dictation {
                    playDictationEndBeep()
                }
                currentMode = .command
                scheduleRestart()
            }
        case "MODE DICTATION":
            DispatchQueue.main.async {
                if currentMode == .command {
                    playDictationStartBeep()
                }
                currentMode = .dictation
                scheduleRestart()
            }
        default:
            break
        }
    }
    // Parent closed stdin → clean exit.
    audioEngine.stop()
    exit(0)
}

startRecognitionTask()
dispatchMain()
"#;
