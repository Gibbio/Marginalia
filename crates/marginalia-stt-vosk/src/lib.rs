use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use marginalia_core::ports::{
    CommandRecognition, CommandRecognizer, ProviderCapabilities, ProviderExecutionMode,
    SpeechInterruptCapture, SpeechInterruptMonitor,
};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::time::{Duration, Instant};
use vosk::{DecodingState, Model, Recognizer};

/// Wrapper to make `cpal::Stream` `Send`.
///
/// `cpal::Stream` is `!Send` on macOS due to an internal `PhantomData<*mut ()>`
/// marker, but the stream is only held as a drop guard — we never move it
/// across threads or access its internals after construction.
struct SendStream(#[allow(dead_code)] cpal::Stream);

// SAFETY: The wrapped stream is created and dropped on the same thread that
// owns the VoskSpeechInterruptMonitor. It is stored solely to keep the
// audio callback alive; no cross-thread access occurs.
unsafe impl Send for SendStream {}

const PROVIDER_NAME: &str = "vosk-command-stt";
const DEFAULT_SAMPLE_RATE: u32 = 16_000;
const DEFAULT_TIMEOUT_SECONDS: f64 = 4.0;
const DEFAULT_SILENCE_TIMEOUT_SECONDS: f64 = 1.2;
const DEFAULT_SPEECH_THRESHOLD: i16 = 3000;
const DEFAULT_MIN_SPEECH_DURATION_MS: u64 = 300;
const AUDIO_RECV_TIMEOUT_MS: u64 = 250;
/// Speech threshold = max(configured, noise_floor * NOISE_MULTIPLIER).
const NOISE_MULTIPLIER: f32 = 3.0;

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct VoskConfig {
    pub model_path: PathBuf,
    pub commands: Vec<String>,
    pub language: String,
    pub sample_rate: u32,
    pub timeout_seconds: f64,
    pub silence_timeout_seconds: f64,
    pub speech_threshold: i16,
    pub min_speech_duration_ms: u64,
    pub input_device_name: Option<String>,
}

impl VoskConfig {
    pub fn new(model_path: impl AsRef<Path>, commands: Vec<String>) -> Self {
        Self {
            model_path: model_path.as_ref().to_path_buf(),
            commands,
            language: "it".to_string(),
            sample_rate: DEFAULT_SAMPLE_RATE,
            timeout_seconds: DEFAULT_TIMEOUT_SECONDS,
            silence_timeout_seconds: DEFAULT_SILENCE_TIMEOUT_SECONDS,
            speech_threshold: DEFAULT_SPEECH_THRESHOLD,
            min_speech_duration_ms: DEFAULT_MIN_SPEECH_DURATION_MS,
            input_device_name: None,
        }
    }
}

// ---------------------------------------------------------------------------
// CommandRecognizer
// ---------------------------------------------------------------------------

pub struct VoskCommandRecognizer {
    config: VoskConfig,
}

impl VoskCommandRecognizer {
    pub fn new(config: VoskConfig) -> Self {
        Self { config }
    }
}

impl CommandRecognizer for VoskCommandRecognizer {
    fn describe_capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            provider_name: PROVIDER_NAME.to_string(),
            interface_kind: "command_stt".to_string(),
            supported_languages: vec![self.config.language.clone()],
            supports_streaming: true,
            supports_partial_results: true,
            supports_timestamps: false,
            low_latency_suitable: true,
            offline_capable: true,
            execution_mode: ProviderExecutionMode::Local,
        }
    }

    fn listen_for_command(&mut self) -> Option<CommandRecognition> {
        let capture = self.capture_interrupt(Some(self.config.timeout_seconds));
        let command = capture.recognized_command?;
        Some(CommandRecognition {
            command: command.clone(),
            provider_name: PROVIDER_NAME.to_string(),
            confidence: 1.0,
            is_final: true,
            raw_text: Some(command),
        })
    }

    fn capture_interrupt(&mut self, timeout_seconds: Option<f64>) -> SpeechInterruptCapture {
        match open_monitor(&self.config) {
            Ok(mut monitor) => monitor.capture_next_interrupt(timeout_seconds),
            Err(err) => failed_capture(err),
        }
    }

    fn open_interrupt_monitor(&mut self) -> Box<dyn SpeechInterruptMonitor> {
        match open_monitor(&self.config) {
            Ok(monitor) => Box::new(monitor),
            Err(err) => Box::new(ErrorMonitor(err)),
        }
    }
}

// ---------------------------------------------------------------------------
// Monitor construction
// ---------------------------------------------------------------------------

fn open_monitor(config: &VoskConfig) -> Result<VoskSpeechInterruptMonitor, String> {
    // Suppress Vosk/Kaldi log messages that would corrupt TUI output.
    vosk::set_log_level(vosk::LogLevel::Error);

    let model = Model::new(config.model_path.to_string_lossy().as_ref()).ok_or_else(|| {
        format!(
            "Failed to load Vosk model from {}",
            config.model_path.display()
        )
    })?;

    let (stream, audio_rx, device_name, actual_rate) =
        setup_audio(config.sample_rate, config.input_device_name.as_deref())?;

    Ok(VoskSpeechInterruptMonitor {
        model,
        commands: config.commands.clone(),
        actual_sample_rate: actual_rate,
        timeout_seconds: config.timeout_seconds,
        silence_timeout_ms: (config.silence_timeout_seconds * 1000.0) as u64,
        speech_threshold: config.speech_threshold,
        min_speech_duration_ms: config.min_speech_duration_ms,
        noise_floor: 0.0,
        audio_rx,
        device_name,
        _stream: SendStream(stream),
    })
}

// ---------------------------------------------------------------------------
// SpeechInterruptMonitor
// ---------------------------------------------------------------------------

pub struct VoskSpeechInterruptMonitor {
    model: Model,
    commands: Vec<String>,
    actual_sample_rate: u32,
    timeout_seconds: f64,
    silence_timeout_ms: u64,
    speech_threshold: i16,
    min_speech_duration_ms: u64,
    /// Running average of ambient noise peaks. Updated every capture cycle
    /// from silent chunks. Persists across calls so it adapts continuously
    /// to changing environments (window opened, AC, etc.).
    noise_floor: f32,
    audio_rx: Receiver<Vec<i16>>,
    device_name: String,
    _stream: SendStream,
}

impl SpeechInterruptMonitor for VoskSpeechInterruptMonitor {
    fn capture_next_interrupt(&mut self, timeout_seconds: Option<f64>) -> SpeechInterruptCapture {
        let timeout = timeout_seconds.unwrap_or(self.timeout_seconds);
        let deadline = Instant::now() + Duration::from_secs_f64(timeout);

        // Clone commands to a local Vec so &commands doesn't borrow self,
        // leaving self.model as the only field borrowed by the recognizer.
        let commands = self.commands.clone();
        let rate = self.actual_sample_rate as f32;

        let mut recognizer = match Recognizer::new_with_grammar(&self.model, rate, &commands) {
            Some(r) => r,
            None => return failed_capture("Failed to create Vosk grammar recognizer".to_string()),
        };

        // Discard stale audio accumulated while the recognizer was being set up.
        while self.audio_rx.try_recv().is_ok() {}

        let started_at = Instant::now();
        let mut speech_detected_ms: Option<u32> = None;
        let mut capture_started_ms: Option<u32> = None;
        let mut silence_started: Option<Instant> = None;
        let mut recognized: Option<String> = None;
        let mut speech_duration_ms: u64 = 0;

        loop {
            if Instant::now() >= deadline {
                break;
            }

            let samples = match self
                .audio_rx
                .recv_timeout(Duration::from_millis(AUDIO_RECV_TIMEOUT_MS))
            {
                Ok(s) => s,
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            };

            let now = Instant::now();
            let now_ms = elapsed_ms(started_at, now);
            let peak = audio_peak(&samples);

            // Adaptive threshold: max(configured, noise_floor * 3).
            // noise_floor updates continuously during silence (see else branch).
            let adaptive_threshold = self
                .speech_threshold
                .max((self.noise_floor * NOISE_MULTIPLIER) as i16);

            if peak >= adaptive_threshold {
                silence_started = None;
                if speech_detected_ms.is_none() {
                    speech_detected_ms = Some(now_ms);
                    capture_started_ms = Some(now_ms);
                }
                speech_duration_ms += AUDIO_RECV_TIMEOUT_MS;
            } else {
                // Silence — update noise floor (EMA, alpha=0.1 → adapts over ~10 chunks ≈ 2.5s)
                self.noise_floor = self.noise_floor * 0.9 + peak as f32 * 0.1;

                if speech_detected_ms.is_some() {
                    let silence_start = *silence_started.get_or_insert(now);
                    if now.duration_since(silence_start).as_millis() as u64
                        >= self.silence_timeout_ms
                    {
                        break;
                    }
                }
            }

            if matches!(
                recognizer.accept_waveform(&samples),
                Ok(DecodingState::Finalized)
            ) {
                let text = extract_text(recognizer.result().single().map(|a| a.text));
                // Only accept if there was sustained speech, not a brief noise spike
                if !text.is_empty() && speech_duration_ms >= self.min_speech_duration_ms {
                    recognized = Some(text);
                    break;
                }
            }
        }

        // Flush final result — only accept if sustained speech was detected.
        if recognized.is_none() && speech_duration_ms >= self.min_speech_duration_ms {
            let text = extract_text(recognizer.final_result().single().map(|a| a.text));
            if !text.is_empty() {
                recognized = Some(text);
            }
        }

        let capture_ended_ms = elapsed_ms(started_at, Instant::now());
        let timed_out = recognized.is_none() && speech_detected_ms.is_none();

        SpeechInterruptCapture {
            provider_name: PROVIDER_NAME.to_string(),
            speech_detected: speech_detected_ms.is_some(),
            capture_ended_ms,
            speech_detected_ms,
            capture_started_ms,
            raw_text: recognized.clone(),
            recognized_command: recognized,
            timed_out,
            input_device_index: None,
            input_device_name: Some(self.device_name.clone()),
            sample_rate: Some(self.actual_sample_rate),
        }
    }

    fn close(&mut self) {
        // _stream is dropped with the struct.
    }
}

// ---------------------------------------------------------------------------
// Audio setup (cpal)
// ---------------------------------------------------------------------------

fn setup_audio(
    desired_rate: u32,
    device_name: Option<&str>,
) -> Result<(cpal::Stream, Receiver<Vec<i16>>, String, u32), String> {
    let host = cpal::default_host();

    let device = match device_name {
        Some(name) => find_input_device_by_name(&host, name)?,
        None => host
            .default_input_device()
            .ok_or_else(|| "No default audio input device available".to_string())?,
    };

    let dev_name = device.name().unwrap_or_else(|_| "unknown".to_string());

    // Prefer desired_rate, fall back to the device default.
    let (stream_config, actual_rate) = preferred_config(&device, desired_rate)?;

    let (tx, rx) = mpsc::sync_channel::<Vec<i16>>(64);
    let channels = stream_config.channels as usize;

    let stream = build_input_stream(&device, &stream_config, tx, channels)?;
    stream
        .play()
        .map_err(|e| format!("Failed to start audio stream: {e}"))?;

    Ok((stream, rx, dev_name, actual_rate))
}

fn preferred_config(
    device: &cpal::Device,
    desired_rate: u32,
) -> Result<(cpal::StreamConfig, u32), String> {
    let supports_desired = device
        .supported_input_configs()
        .map_err(|e| format!("Cannot query device configs: {e}"))?
        .any(|c| c.min_sample_rate().0 <= desired_rate && c.max_sample_rate().0 >= desired_rate);

    if supports_desired {
        return Ok((
            cpal::StreamConfig {
                channels: 1,
                sample_rate: cpal::SampleRate(desired_rate),
                buffer_size: cpal::BufferSize::Default,
            },
            desired_rate,
        ));
    }

    // Fall back to the device's default config.
    let default = device
        .default_input_config()
        .map_err(|e| format!("No default input config: {e}"))?;
    let actual_rate = default.sample_rate().0;
    Ok((
        cpal::StreamConfig {
            channels: default.channels(),
            sample_rate: default.sample_rate(),
            buffer_size: cpal::BufferSize::Default,
        },
        actual_rate,
    ))
}

fn build_input_stream(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    tx: SyncSender<Vec<i16>>,
    channels: usize,
) -> Result<cpal::Stream, String> {
    device
        .build_input_stream(
            config,
            move |data: &[f32], _| {
                // Downmix to mono (first channel) and convert f32 → i16.
                let samples: Vec<i16> = data
                    .chunks(channels)
                    .map(|frame| {
                        let s = frame[0];
                        (s * 32_767.0).clamp(-32_768.0, 32_767.0) as i16
                    })
                    .collect();
                let _ = tx.try_send(samples);
            },
            |err| log::error!("[vosk-stt] audio error: {err}"),
            None,
        )
        .map_err(|e| format!("Failed to build audio stream: {e}"))
}

fn find_input_device_by_name(host: &cpal::Host, name: &str) -> Result<cpal::Device, String> {
    let normalized = name.trim().to_lowercase();
    host.input_devices()
        .map_err(|e| format!("Cannot enumerate audio devices: {e}"))?
        .find(|d| {
            d.name()
                .map(|n| n.to_lowercase().contains(&normalized))
                .unwrap_or(false)
        })
        .ok_or_else(|| format!("Audio input device '{name}' not found"))
}

// ---------------------------------------------------------------------------
// Error fallback monitor
// ---------------------------------------------------------------------------

struct ErrorMonitor(String);

impl SpeechInterruptMonitor for ErrorMonitor {
    fn capture_next_interrupt(&mut self, _timeout_seconds: Option<f64>) -> SpeechInterruptCapture {
        failed_capture(self.0.clone())
    }

    fn close(&mut self) {}
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn extract_text(text: Option<&str>) -> String {
    text.unwrap_or("").trim().to_lowercase()
}

fn audio_peak(samples: &[i16]) -> i16 {
    samples
        .iter()
        .map(|s| s.unsigned_abs() as i16)
        .max()
        .unwrap_or(0)
}

fn elapsed_ms(started_at: Instant, now: Instant) -> u32 {
    now.duration_since(started_at)
        .as_millis()
        .min(u32::MAX as u128) as u32
}

fn failed_capture(reason: String) -> SpeechInterruptCapture {
    SpeechInterruptCapture {
        provider_name: PROVIDER_NAME.to_string(),
        speech_detected: false,
        capture_ended_ms: 0,
        speech_detected_ms: None,
        capture_started_ms: None,
        recognized_command: None,
        raw_text: Some(format!("error: {reason}")),
        timed_out: true,
        input_device_index: None,
        input_device_name: None,
        sample_rate: None,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audio_peak_returns_max_absolute_value() {
        let samples = vec![0i16, 100, -500, 200, -100];
        assert_eq!(audio_peak(&samples), 500);
    }

    #[test]
    fn audio_peak_empty_returns_zero() {
        assert_eq!(audio_peak(&[]), 0);
    }

    #[test]
    fn extract_text_trims_and_lowercases() {
        assert_eq!(extract_text(Some("  Pausa  ")), "pausa");
        assert_eq!(extract_text(None), "");
    }

    #[test]
    fn elapsed_ms_is_non_negative() {
        let t = Instant::now();
        let ms = elapsed_ms(t, t);
        assert_eq!(ms, 0);
    }

    #[test]
    fn vosk_config_defaults() {
        let cfg = VoskConfig::new("/tmp/model", vec!["pausa".to_string()]);
        assert_eq!(cfg.sample_rate, DEFAULT_SAMPLE_RATE);
        assert_eq!(cfg.language, "it");
        assert!(cfg.input_device_name.is_none());
    }

    #[test]
    fn vosk_command_recognizer_describes_capabilities() {
        let cfg = VoskConfig::new("/tmp/model", vec!["pausa".to_string()]);
        let rec = VoskCommandRecognizer::new(cfg);
        let caps = rec.describe_capabilities();
        assert_eq!(caps.provider_name, PROVIDER_NAME);
        assert_eq!(caps.interface_kind, "command_stt");
        assert!(caps.offline_capable);
    }
}
