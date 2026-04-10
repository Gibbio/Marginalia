use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use marginalia_core::ports::{
    CommandRecognition, CommandRecognizer, DictationSegment, DictationTranscriber,
    DictationTranscript, ProviderCapabilities, ProviderExecutionMode, SpeechInterruptCapture,
    SpeechInterruptMonitor,
};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::Once;
use std::time::{Duration, Instant};
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

static WHISPER_LOG_INIT: Once = Once::new();

/// Suppress whisper.cpp/ggml log output that would corrupt the TUI.
fn suppress_whisper_logs() {
    WHISPER_LOG_INIT.call_once(|| {
        // Redirect whisper/ggml logs to Rust's log framework instead of stderr.
        // With no log subscriber configured, these are silently discarded.
        whisper_rs::install_logging_hooks();
    });
}

const PROVIDER_NAME: &str = "whisper-dictation-stt";
const DEFAULT_SAMPLE_RATE: u32 = 16_000;
/// RMS amplitude threshold (i16 scale) for distinguishing speech from silence.
const DEFAULT_SPEECH_THRESHOLD: i16 = 500;
const DEFAULT_SILENCE_TIMEOUT_SECONDS: f64 = 1.5;
const DEFAULT_MAX_DURATION_SECONDS: f64 = 60.0;

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct WhisperConfig {
    pub model_path: PathBuf,
    pub language: String,
    pub max_duration_seconds: f64,
    pub silence_timeout_seconds: f64,
    /// Minimum RMS amplitude (i16 scale) considered as speech.
    pub speech_threshold: i16,
    pub sample_rate: u32,
    pub input_device_name: Option<String>,
}

impl WhisperConfig {
    pub fn new(model_path: impl AsRef<Path>) -> Self {
        Self {
            model_path: model_path.as_ref().to_path_buf(),
            language: "it".to_string(),
            max_duration_seconds: DEFAULT_MAX_DURATION_SECONDS,
            silence_timeout_seconds: DEFAULT_SILENCE_TIMEOUT_SECONDS,
            speech_threshold: DEFAULT_SPEECH_THRESHOLD,
            sample_rate: DEFAULT_SAMPLE_RATE,
            input_device_name: None,
        }
    }
}

// ---------------------------------------------------------------------------
// DictationTranscriber
// ---------------------------------------------------------------------------

pub struct WhisperDictationTranscriber {
    config: WhisperConfig,
}

impl WhisperDictationTranscriber {
    pub fn new(config: WhisperConfig) -> Self {
        Self { config }
    }
}

impl DictationTranscriber for WhisperDictationTranscriber {
    fn describe_capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            provider_name: PROVIDER_NAME.to_string(),
            interface_kind: "dictation_stt".to_string(),
            supported_languages: vec![self.config.language.clone()],
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
        match self.record_and_transcribe() {
            Ok(transcript) => transcript,
            Err(err) => DictationTranscript {
                text: format!("[Errore trascrizione: {err}]"),
                provider_name: PROVIDER_NAME.to_string(),
                language: self.config.language.clone(),
                is_final: true,
                segments: vec![],
                raw_text: None,
            },
        }
    }
}

impl WhisperDictationTranscriber {
    fn record_and_transcribe(&self) -> Result<DictationTranscript, String> {
        let samples_i16 = self.capture_audio_i16()?;
        if samples_i16.is_empty() {
            return Ok(DictationTranscript {
                text: String::new(),
                provider_name: PROVIDER_NAME.to_string(),
                language: self.config.language.clone(),
                is_final: true,
                segments: vec![],
                raw_text: Some(String::new()),
            });
        }
        // whisper.cpp expects f32 samples normalised to [-1.0, 1.0]
        let samples_f32: Vec<f32> = samples_i16.iter().map(|&s| s as f32 / 32768.0).collect();
        self.run_inference(samples_f32)
    }

    pub(crate) fn capture_audio_i16(&self) -> Result<Vec<i16>, String> {
        let host = cpal::default_host();
        let device = match &self.config.input_device_name {
            Some(name) => host
                .input_devices()
                .map_err(|e| format!("Cannot list input devices: {e}"))?
                .find(|d| d.name().ok().as_deref() == Some(name.as_str()))
                .ok_or_else(|| format!("Input device '{name}' not found"))?,
            None => host
                .default_input_device()
                .ok_or_else(|| "No default input device".to_string())?,
        };

        // Use the device's default config (macOS typically provides f32)
        // and convert to mono i16.
        let default_config = device
            .default_input_config()
            .map_err(|e| format!("No default input config: {e}"))?;
        let channels = default_config.channels() as usize;
        let actual_rate = default_config.sample_rate().0;

        let stream_config = cpal::StreamConfig {
            channels: default_config.channels(),
            sample_rate: default_config.sample_rate(),
            buffer_size: cpal::BufferSize::Default,
        };

        let (tx, rx) = mpsc::sync_channel::<Vec<i16>>(256);
        let stream = device
            .build_input_stream(
                &stream_config,
                move |data: &[f32], _| {
                    // Downmix to mono and convert f32 → i16
                    let samples: Vec<i16> = data
                        .chunks(channels)
                        .map(|frame| (frame[0] * 32767.0).clamp(-32768.0, 32767.0) as i16)
                        .collect();
                    let _ = tx.try_send(samples);
                },
                |err| eprintln!("[whisper-stt] cpal stream error: {err}"),
                None,
            )
            .map_err(|e| format!("Cannot build input stream: {e}"))?;

        stream
            .play()
            .map_err(|e| format!("Cannot start audio stream: {e}"))?;

        // Use actual device rate for duration calculations
        let max_samples = (actual_rate as f64 * self.config.max_duration_seconds) as usize;
        let silence_samples = (actual_rate as f64 * self.config.silence_timeout_seconds) as usize;
        let min_samples = actual_rate as usize / 2;
        let threshold = self.config.speech_threshold;
        let target_rate = self.config.sample_rate;

        let mut all_samples: Vec<i16> = Vec::with_capacity(max_samples);
        let mut silence_count: usize = 0;
        let hard_limit = Duration::from_secs_f64(self.config.max_duration_seconds + 2.0);
        let started = Instant::now();

        loop {
            if started.elapsed() > hard_limit || all_samples.len() >= max_samples {
                break;
            }

            match rx.recv_timeout(Duration::from_millis(200)) {
                Ok(chunk) => {
                    let rms = rms_i16(&chunk);
                    if rms >= threshold {
                        silence_count = 0;
                    } else {
                        silence_count += chunk.len();
                    }
                    all_samples.extend_from_slice(&chunk);
                    if silence_count >= silence_samples && all_samples.len() >= min_samples {
                        break;
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    // Approximate silence advancement when no chunk arrives
                    let approx = (200.0 * actual_rate as f64 / 1000.0) as usize;
                    silence_count += approx;
                    if silence_count >= silence_samples && !all_samples.is_empty() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }

        drop(stream);

        // Resample to target rate (16kHz) if device rate differs
        if actual_rate != target_rate && !all_samples.is_empty() {
            let ratio = target_rate as f64 / actual_rate as f64;
            let new_len = (all_samples.len() as f64 * ratio) as usize;
            let mut resampled = Vec::with_capacity(new_len);
            for i in 0..new_len {
                let src = (i as f64 / ratio) as usize;
                resampled.push(all_samples[src.min(all_samples.len() - 1)]);
            }
            return Ok(resampled);
        }

        Ok(all_samples)
    }

    pub(crate) fn run_inference(&self, samples: Vec<f32>) -> Result<DictationTranscript, String> {
        suppress_whisper_logs();

        let model_path = self
            .config
            .model_path
            .to_str()
            .ok_or_else(|| "Whisper model path is not valid UTF-8".to_string())?;

        let ctx = WhisperContext::new_with_params(model_path, WhisperContextParameters::default())
            .map_err(|e| format!("Cannot load Whisper model: {e}"))?;

        let mut state = ctx
            .create_state()
            .map_err(|e| format!("Cannot create Whisper state: {e}"))?;

        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        params.set_language(Some(&self.config.language));
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);

        state
            .full(params, &samples)
            .map_err(|e| format!("Whisper inference failed: {e}"))?;

        let n = state
            .full_n_segments()
            .map_err(|e| format!("Cannot count Whisper segments: {e}"))?;

        let mut segments: Vec<DictationSegment> = Vec::new();
        let mut full_text = String::new();

        for i in 0..n {
            let text = state
                .full_get_segment_text(i)
                .map_err(|e| format!("Cannot read Whisper segment {i}: {e}"))?;
            let text = text.trim().to_string();
            if text.is_empty() {
                continue;
            }
            let t0 = state
                .full_get_segment_t0(i)
                .map_err(|e| format!("Cannot read segment t0 {i}: {e}"))?;
            let t1 = state
                .full_get_segment_t1(i)
                .map_err(|e| format!("Cannot read segment t1 {i}: {e}"))?;

            if !full_text.is_empty() {
                full_text.push(' ');
            }
            full_text.push_str(&text);
            segments.push(DictationSegment {
                text,
                start_ms: (t0 * 10).max(0) as u32, // centiseconds → ms
                end_ms: (t1 * 10).max(0) as u32,
            });
        }

        Ok(DictationTranscript {
            text: full_text.clone(),
            provider_name: PROVIDER_NAME.to_string(),
            language: self.config.language.clone(),
            is_final: true,
            segments,
            raw_text: Some(full_text),
        })
    }
}

// ---------------------------------------------------------------------------
// CommandRecognizer (Whisper-based)
// ---------------------------------------------------------------------------

const CMD_PROVIDER_NAME: &str = "whisper-command-stt";
const CMD_MAX_DURATION_SECONDS: f64 = 4.0;
const CMD_SILENCE_TIMEOUT_SECONDS: f64 = 1.0;

/// Whisper-based command recognizer. Transcribes speech then fuzzy-matches
/// against a list of known commands. More accurate than grammar-based
/// recognizers (Vosk) because it does full speech recognition first.
pub struct WhisperCommandRecognizer {
    config: WhisperConfig,
    commands: Vec<String>,
}

impl WhisperCommandRecognizer {
    pub fn new(config: WhisperConfig, commands: Vec<String>) -> Self {
        Self { config, commands }
    }

    fn transcribe_short(&self) -> Result<String, String> {
        // Use short timeouts for command recognition
        let short_config = WhisperConfig {
            max_duration_seconds: CMD_MAX_DURATION_SECONDS,
            silence_timeout_seconds: CMD_SILENCE_TIMEOUT_SECONDS,
            ..self.config.clone()
        };
        let transcriber = WhisperDictationTranscriber::new(short_config);
        let samples = transcriber.capture_audio_i16()?;
        if samples.is_empty() {
            return Ok(String::new());
        }
        let samples_f32: Vec<f32> = samples.iter().map(|&s| s as f32 / 32768.0).collect();
        let transcript = transcriber.run_inference(samples_f32)?;
        Ok(transcript.text.to_lowercase())
    }

    /// Find the best matching command in the transcribed text.
    fn match_command(&self, text: &str) -> Option<String> {
        let text = text.to_lowercase();
        // Exact substring match
        for cmd in &self.commands {
            if text.contains(&cmd.to_lowercase()) {
                return Some(cmd.clone());
            }
        }
        None
    }
}

impl CommandRecognizer for WhisperCommandRecognizer {
    fn describe_capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            provider_name: CMD_PROVIDER_NAME.to_string(),
            interface_kind: "command_stt".to_string(),
            supported_languages: vec![self.config.language.clone()],
            supports_streaming: false,
            supports_partial_results: false,
            supports_timestamps: false,
            low_latency_suitable: false,
            offline_capable: true,
            execution_mode: ProviderExecutionMode::Local,
        }
    }

    fn listen_for_command(&mut self) -> Option<CommandRecognition> {
        let text = self.transcribe_short().ok()?;
        let command = self.match_command(&text)?;
        Some(CommandRecognition {
            command: command.clone(),
            provider_name: CMD_PROVIDER_NAME.to_string(),
            confidence: 1.0,
            is_final: true,
            raw_text: Some(text),
        })
    }

    fn capture_interrupt(&mut self, _timeout_seconds: Option<f64>) -> SpeechInterruptCapture {
        let started = Instant::now();
        let (text, raw_error) = match self.transcribe_short() {
            Ok(t) => (t, None),
            Err(e) => (String::new(), Some(format!("error: {e}"))),
        };
        let command = self.match_command(&text);
        let elapsed_ms = started.elapsed().as_millis().min(u32::MAX as u128) as u32;

        SpeechInterruptCapture {
            provider_name: CMD_PROVIDER_NAME.to_string(),
            speech_detected: !text.is_empty(),
            capture_ended_ms: elapsed_ms,
            speech_detected_ms: if text.is_empty() { None } else { Some(0) },
            capture_started_ms: Some(0),
            raw_text: raw_error.clone().or(Some(text)),
            recognized_command: command,
            timed_out: false,
            input_device_index: None,
            input_device_name: None,
            sample_rate: Some(self.config.sample_rate),
        }
    }

    fn open_interrupt_monitor(&mut self) -> Box<dyn SpeechInterruptMonitor> {
        Box::new(WhisperInterruptMonitor {
            config: self.config.clone(),
            commands: self.commands.clone(),
        })
    }
}

struct WhisperInterruptMonitor {
    config: WhisperConfig,
    commands: Vec<String>,
}

impl SpeechInterruptMonitor for WhisperInterruptMonitor {
    fn capture_next_interrupt(&mut self, _timeout_seconds: Option<f64>) -> SpeechInterruptCapture {
        let mut recognizer =
            WhisperCommandRecognizer::new(self.config.clone(), self.commands.clone());
        recognizer.capture_interrupt(None)
    }

    fn close(&mut self) {}
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn rms_i16(samples: &[i16]) -> i16 {
    if samples.is_empty() {
        return 0;
    }
    let sum_sq: i64 = samples.iter().map(|&s| (s as i64) * (s as i64)).sum();
    ((sum_sq / samples.len() as i64) as f64).sqrt() as i16
}
