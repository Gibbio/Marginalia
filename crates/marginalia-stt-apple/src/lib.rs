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

pub mod aec_pipeline;

use marginalia_core::ports::{
    CommandRecognition, CommandRecognizer, DictationSegment, DictationTranscriber,
    DictationTranscript, ProviderCapabilities, ProviderExecutionMode, SpeechInterruptCapture,
    SpeechInterruptMonitor,
};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{mpsc, Arc, Mutex, Once};
use std::time::Duration;

const COMMAND_PROVIDER_NAME: &str = "apple-stt";
const DICTATION_PROVIDER_NAME: &str = "apple-dictation-stt";

/// Bump when SWIFT_HELPER_SOURCE changes so the cached binary gets recompiled.
///
/// Also bump when modifying anything in `helper/stt-helper.swift`.
const HELPER_VERSION: u32 = 10;

static COMPILE_HELPER: Once = Once::new();

/// File name used both by the bundled binary and the dev-mode cache.
fn helper_filename() -> String {
    format!("stt-helper-v{HELPER_VERSION}")
}

/// Cache path used by the dev-mode fallback (compile-on-the-fly).
fn cache_helper_path() -> PathBuf {
    std::env::temp_dir()
        .join("marginalia-stt-apple")
        .join(helper_filename())
}

/// Returns the helper binary path using a 3-tier resolver:
/// 1. `MARGINALIA_STT_HELPER` env var (fully explicit — used by the Xcode
///    launch scheme and by the Makefile target `run-stt-helper`).
/// 2. `<app-bundle>/Contents/Helpers/stt-helper-vN` if the current executable
///    is inside a `.app` bundle (production path for the signed GUI).
/// 3. `$TMPDIR/marginalia-stt-apple/stt-helper-vN`, compile-on-the-fly via
///    `swiftc` (dev-mode fallback — what the TUI has always done).
///
/// Resolving via (1) or (2) is zero-cost and does not invoke `swiftc` — that
/// matters inside a signed/sandboxed `.app` where `swiftc` isn't available.
fn ensure_helper() -> Result<PathBuf, String> {
    // (1) Explicit override.
    if let Ok(p) = std::env::var("MARGINALIA_STT_HELPER") {
        let path = PathBuf::from(&p);
        if path.is_file() {
            return Ok(path);
        }
        return Err(format!(
            "MARGINALIA_STT_HELPER points to {p} but the file does not exist"
        ));
    }

    // (2) macOS `.app` bundle — look for the helper under Contents/Helpers/.
    if let Some(bundle_helper) = bundled_helper_path() {
        if bundle_helper.is_file() {
            return Ok(bundle_helper);
        }
    }

    // (3) Dev-mode cache + compile-on-the-fly fallback.
    let path = cache_helper_path();
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
        Err("Swift helper not compiled. Is Xcode installed, or set \
             MARGINALIA_STT_HELPER to a prebuilt binary."
            .to_string())
    }
}

/// Walk up from the current executable's path looking for a `.app` bundle,
/// then return `<app>/Contents/Helpers/stt-helper-vN`. Returns `None` when
/// the executable is not inside a bundle (e.g. running under `cargo` or
/// `swift run`).
fn bundled_helper_path() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let mut cur: &Path = exe.as_path();
    while let Some(parent) = cur.parent() {
        if parent.extension().and_then(|e: &std::ffi::OsStr| e.to_str()) == Some("app") {
            return Some(parent.join("Contents").join("Helpers").join(helper_filename()));
        }
        cur = parent;
    }
    None
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

/// TLV frame types for the binary stdin protocol.
const FRAME_AUDIO: u8 = 0x41; // 'A'
const FRAME_MODE: u8 = 0x4D; // 'M'

impl AppleHelperShared {
    /// Write a TLV frame to the helper's stdin.
    fn write_frame(&self, frame_type: u8, payload: &[u8]) -> Result<(), String> {
        let len = payload.len();
        if len > u16::MAX as usize {
            return Err(format!("frame payload too large: {len}"));
        }
        let mut stdin = self.stdin.lock().unwrap();
        stdin
            .write_all(&[frame_type, (len >> 8) as u8, len as u8])
            .and_then(|_| stdin.write_all(payload))
            .and_then(|_| stdin.flush())
            .map_err(|e| format!("write to helper stdin: {e}"))
    }

    /// Send a mode-switch command ("COMMAND" or "DICTATION").
    fn switch_to_command(&self) -> Result<(), String> {
        self.write_frame(FRAME_MODE, b"COMMAND")
    }

    fn switch_to_dictation(&self) -> Result<(), String> {
        self.write_frame(FRAME_MODE, b"DICTATION")
    }

    /// Send an audio frame (f32 samples, little-endian) to the helper for
    /// SFSpeechRecognizer to process.
    pub fn write_audio_frame(&self, samples: &[f32]) -> Result<(), String> {
        let bytes: &[u8] = unsafe {
            std::slice::from_raw_parts(
                samples.as_ptr() as *const u8,
                std::mem::size_of_val(samples),
            )
        };
        self.write_frame(FRAME_AUDIO, bytes)
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
/// Sample rate used for the audio pipeline (mic → AEC → helper).
pub const AEC_SAMPLE_RATE: u32 = 24_000;

pub fn new_apple_stt(
    language: &str,
    commands: Vec<String>,
    cmd_silence_timeout: f64,
    dict_silence_timeout: f64,
    dict_max_seconds: f64,
    recorded_audio_dir: std::path::PathBuf,
) -> Result<
    (
        AppleCommandRecognizer,
        AppleDictationTranscriber,
        aec_pipeline::AecPipeline,
    ),
    String,
> {
    let helper = ensure_helper()?;

    // Smoke test: run the helper for 0.5s to surface immediate setup errors
    // such as Dictation being disabled in System Settings.
    let smoke = Command::new(&helper)
        .arg(language)
        .arg(format!("{cmd_silence_timeout}"))
        .arg(format!("{dict_silence_timeout}"))
        .arg(join_commands(&commands))
        .arg(format!("{AEC_SAMPLE_RATE}"))
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
        .arg(format!("{AEC_SAMPLE_RATE}"))
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|e| format!("spawn helper: {e}"))?;

    let stdout = child.stdout.take().ok_or("no stdout")?;
    let stdin = child.stdin.take().ok_or("no stdin")?;

    let (cmd_tx, cmd_rx) = mpsc::channel::<String>();
    let (dict_tx, dict_rx) = mpsc::channel::<String>();
    // Live partial transcripts go into a shared slot rather than a
    // channel — the host polls the latest value on its tick rather
    // than draining a queue (we only ever care about the most recent
    // partial; older intermediate states are discardable).
    let dict_partial: Arc<Mutex<String>> = Arc::new(Mutex::new(String::new()));
    let dict_partial_writer = dict_partial.clone();

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
            } else if let Some(rest) = trimmed.strip_prefix("DICT_PARTIAL ") {
                if let Ok(mut slot) = dict_partial_writer.lock() {
                    *slot = rest.to_string();
                }
            } else if let Some(rest) = trimmed.strip_prefix("DICT_END ") {
                if let Ok(mut slot) = dict_partial_writer.lock() {
                    slot.clear();
                }
                if dict_tx.send(rest.to_string()).is_err() {
                    break;
                }
            } else if trimmed == "DICT_END" {
                if let Ok(mut slot) = dict_partial_writer.lock() {
                    slot.clear();
                }
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

    // Start the AEC pipeline: cpal mic → AEC3 → cleaned audio → helper stdin.
    let aec = aec_pipeline::AecPipeline::start(shared.clone())
        .map_err(|e| format!("AEC pipeline: {e}"))?;

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
        dict_partial,
        max_duration: Duration::from_secs_f64(dict_max_seconds),
        recorder_slot: aec.recorder_slot(),
        recorded_audio_dir,
        last_audio_path: Arc::new(Mutex::new(None)),
    };

    Ok((recognizer, transcriber, aec))
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
    /// Latest partial transcript from the helper. Updated by the reader
    /// thread on every `DICT_PARTIAL` line, cleared on `DICT_END`. The
    /// host polls this on its event tick to render the running transcript
    /// in the live-note card without waiting for the silence-final.
    dict_partial: Arc<Mutex<String>>,
    max_duration: Duration,
    /// Shared slot that tells the AEC thread to write each capture
    /// frame into a WAV. We flip it on before `switch_to_dictation()`
    /// and off when DICT_END arrives, then stash the finalized path
    /// in `last_audio_path` so the FFI dictation thread can read it
    /// and attach to the created note.
    recorder_slot: aec_pipeline::DictationRecorderSlot,
    recorded_audio_dir: std::path::PathBuf,
    last_audio_path: Arc<Mutex<Option<std::path::PathBuf>>>,
}

impl AppleDictationTranscriber {
    /// Returns the absolute path of the last dictation recording and
    /// clears the stored value. Call AFTER `transcribe()` returns —
    /// the FFI dictation thread uses it to set `raw_audio_path` on
    /// the created note.
    pub fn take_last_audio_path(&self) -> Option<std::path::PathBuf> {
        self.last_audio_path.lock().ok()?.take()
    }

    /// Clone of the partial-transcript shared slot. Hands it to the
    /// runtime so polling skips the transcriber mutex (which is
    /// owned by the dictation thread for the entire `transcribe()`).
    pub fn dict_partial_slot(&self) -> Arc<Mutex<String>> {
        self.dict_partial.clone()
    }
}

impl DictationTranscriber for AppleDictationTranscriber {
    fn peek_partial(&self) -> String {
        self.dict_partial
            .lock()
            .map(|s| s.clone())
            .unwrap_or_default()
    }

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
        // Generate a unique path for this recording. We record FIRST,
        // transcribe SECOND — so we don't know the note id at this
        // point; uuid is the lightweight stand-in, and the runtime
        // attaches the path to whatever note it creates on success.
        let audio_path = self
            .recorded_audio_dir
            .join(format!("{}.wav", uuid::Uuid::new_v4()));

        let recording_started =
            match aec_pipeline::start_dictation_recording(
                &self.recorder_slot,
                audio_path.clone(),
            ) {
                Ok(()) => true,
                Err(e) => {
                    log::warn!("[apple-stt] dictation recording failed to start: {e}");
                    false
                }
            };

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

        // Always close the WAV (success OR error) so we don't leak the
        // writer and the header gets finalized. On success, remember
        // the path so the FFI thread can pick it up and attach to
        // the created note.
        let finalized = if recording_started {
            aec_pipeline::stop_dictation_recording(&self.recorder_slot)
        } else {
            None
        };
        if result.is_ok() {
            if let Some(p) = finalized.clone() {
                if let Ok(mut slot) = self.last_audio_path.lock() {
                    *slot = Some(p);
                }
            }
        } else if let Some(p) = finalized {
            // Transcription failed — don't hand the caller a dangling
            // audio file. Removing it keeps the notes/audio dir tidy.
            let _ = std::fs::remove_file(p);
        }

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
                raw_audio_path: self.take_last_audio_path(),
            },
            Err(err) => DictationTranscript {
                text: format!("[Apple dictation error: {err}]"),
                provider_name: DICTATION_PROVIDER_NAME.to_string(),
                language: self.language.clone(),
                is_final: true,
                segments: vec![],
                raw_text: None,
                raw_audio_path: None,
            },
        }
    }
}

// =============================================================================
// Swift helper source
// =============================================================================

/// Persistent Swift helper v8. Receives AEC-cleaned audio from Rust via a
/// binary TLV protocol on stdin (no longer captures the mic itself). Output
/// lines are prefixed so the Rust reader thread can route them:
///
///   `CMD <text>`       — recognized in command mode
///   `DICT_END <text>`  — finalized dictation
///
/// Stdin binary protocol:
///   byte 0     — type: 0x41 ('A') audio frame, 0x4D ('M') mode command
///   byte 1-2   — payload length (big-endian uint16)
///   byte 3..   — payload
///
/// Audio payload: raw f32 samples (little-endian), mono, at the sample rate
/// passed as CLI arg 5 (default 24000). 10ms per frame = rate/100 samples.
///
/// Mode payload: UTF-8 text, one of "COMMAND" or "DICTATION".
///
/// CLI: `stt-helper <language> <cmd_silence> <dict_silence> <triggers> <sample_rate>`
///
/// The Swift source lives in `helper/stt-helper.swift` so the Xcode project
/// can compile it as a build phase without re-implementing the text. Keeping
/// the `include_str!` here means the dev-mode fallback (compile-on-the-fly
/// via `swiftc`) and the bundled build use the exact same source.
const SWIFT_HELPER_SOURCE: &str = include_str!("../helper/stt-helper.swift");
