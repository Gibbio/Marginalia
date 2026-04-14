//! Acoustic echo cancellation pipeline.
//!
//! Opens the default mic via cpal, runs WebRTC AEC3 to subtract the TTS
//! playback signal (render/reference) from the mic input (capture/near-end),
//! and writes the cleaned audio frames to the Swift helper's stdin.
//!
//! The render reference is set as a whole buffer (the WAV being played) and
//! advanced frame-by-frame in lockstep with the mic capture so that render
//! and capture stay perfectly aligned in time.

use crate::AppleHelperShared;
use aec3::voip::VoipAec3;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};

/// Number of samples in a 10ms frame at the AEC sample rate.
const FRAME_SAMPLES: usize = (crate::AEC_SAMPLE_RATE as usize) / 100; // 240 at 24kHz

/// Max number of level entries in the waveform ring buffer (~1.3s at 10ms/frame).
const WAVEFORM_HISTORY: usize = 128;

/// Commands sent to the AEC thread to set/clear the render reference.
pub enum RenderCommand {
    /// Full WAV samples (f32 mono at AEC_SAMPLE_RATE) of the chunk about to play.
    SetReference(Vec<f32>),
    /// Playback stopped — no more render to subtract.
    ClearReference,
}

/// Real-time audio level data for UI waveform visualization. Updated by the
/// AEC thread every 10ms frame. Read by the TUI render loop.
#[derive(Default)]
pub struct WaveformData {
    /// Peak amplitude (0.0–1.0) of recent TTS render frames.
    pub tts_levels: Vec<f32>,
    /// Peak amplitude (0.0–1.0) of recent mic capture frames.
    pub mic_levels: Vec<f32>,
}

impl WaveformData {
    fn push_tts(&mut self, level: f32) {
        if self.tts_levels.len() >= WAVEFORM_HISTORY {
            self.tts_levels.remove(0);
        }
        self.tts_levels.push(level);
    }

    fn push_mic(&mut self, level: f32) {
        if self.mic_levels.len() >= WAVEFORM_HISTORY {
            self.mic_levels.remove(0);
        }
        self.mic_levels.push(level);
    }
}

/// Handle to the running AEC pipeline. Dropping it stops the mic stream.
pub struct AecPipeline {
    _stream: cpal::Stream,
    render_tx: mpsc::SyncSender<RenderCommand>,
    waveform: Arc<Mutex<WaveformData>>,
}

impl AecPipeline {
    /// Start the pipeline: open mic, spawn the AEC processing thread, and
    /// begin feeding cleaned audio to the helper.
    pub fn start(helper: Arc<AppleHelperShared>) -> Result<Self, String> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or_else(|| "no default input device".to_string())?;

        let default_config = device
            .default_input_config()
            .map_err(|e| format!("no default input config: {e}"))?;
        let device_rate = default_config.sample_rate().0;
        let channels = default_config.channels() as usize;

        let stream_config = cpal::StreamConfig {
            channels: default_config.channels(),
            sample_rate: default_config.sample_rate(),
            buffer_size: cpal::BufferSize::Default,
        };

        // Channel: cpal callback → AEC thread (mic frames).
        let (mic_tx, mic_rx) = mpsc::sync_channel::<Vec<f32>>(64);
        // Channel: playback → AEC thread (render reference commands).
        let (render_tx, render_rx) = mpsc::sync_channel::<RenderCommand>(4);
        // Shared waveform data for UI visualization.
        let waveform = Arc::new(Mutex::new(WaveformData::default()));

        let stream = device
            .build_input_stream(
                &stream_config,
                move |data: &[f32], _| {
                    let mono: Vec<f32> = data.chunks(channels).map(|frame| frame[0]).collect();
                    let _ = mic_tx.try_send(mono);
                },
                |err| log::error!("[aec] cpal stream error: {err}"),
                None,
            )
            .map_err(|e| format!("cannot build input stream: {e}"))?;
        stream
            .play()
            .map_err(|e| format!("cannot start input stream: {e}"))?;

        let target_rate = crate::AEC_SAMPLE_RATE as usize;

        // AEC processing thread.
        let waveform_writer = waveform.clone();
        std::thread::spawn(move || {
            let mut aec = match VoipAec3::builder(target_rate, 1, 1).build() {
                Ok(a) => a,
                Err(e) => {
                    log::error!("[aec] failed to build AEC3 pipeline: {e}");
                    return;
                }
            };

            let mut mic_accum: Vec<f32> = Vec::with_capacity(FRAME_SAMPLES * 4);
            let mut out_buf = vec![0.0f32; FRAME_SAMPLES];

            // Render reference: the full WAV samples and our read position.
            let mut render_ref: Option<Vec<f32>> = None;
            let mut render_pos: usize = 0;

            while let Ok(raw_mic) = mic_rx.recv() {
                // Check for render commands (non-blocking).
                while let Ok(cmd) = render_rx.try_recv() {
                    match cmd {
                        RenderCommand::SetReference(samples) => {
                            render_ref = Some(samples);
                            render_pos = 0;
                        }
                        RenderCommand::ClearReference => {
                            render_ref = None;
                            render_pos = 0;
                        }
                    }
                }

                // Resample mic to target_rate if needed.
                let resampled = if device_rate as usize != target_rate {
                    linear_resample(&raw_mic, device_rate as usize, target_rate)
                } else {
                    raw_mic
                };
                mic_accum.extend_from_slice(&resampled);

                // Process complete 10ms frames.
                while mic_accum.len() >= FRAME_SAMPLES {
                    let frame: Vec<f32> = mic_accum.drain(..FRAME_SAMPLES).collect();

                    // Feed the corresponding render frame (in lockstep with mic).
                    if let Some(ref samples) = render_ref {
                        if render_pos + FRAME_SAMPLES <= samples.len() {
                            let render_frame = &samples[render_pos..render_pos + FRAME_SAMPLES];
                            if let Err(e) = aec.handle_render_frame(render_frame) {
                                log::warn!("[aec] render frame error: {e}");
                            }
                            render_pos += FRAME_SAMPLES;
                        }
                        // If reference is exhausted, no more render frames to feed
                        // (silence period after chunk ends). AEC passes mic through.
                    }

                    // Process capture through AEC.
                    match aec.process_capture_frame(&frame, false, &mut out_buf) {
                        Ok(_) => {}
                        Err(e) => {
                            log::warn!("[aec] capture frame error: {e}");
                            out_buf.copy_from_slice(&frame);
                        }
                    }

                    // Update waveform levels for UI visualization.
                    if let Ok(mut wf) = waveform_writer.try_lock() {
                        let mic_peak = frame.iter().fold(0.0f32, |a, &s| a.max(s.abs()));
                        wf.push_mic(mic_peak.min(1.0));

                        let tts_peak = if let Some(ref samples) = render_ref {
                            let start = render_pos.saturating_sub(FRAME_SAMPLES);
                            let end = start + FRAME_SAMPLES;
                            if end <= samples.len() {
                                samples[start..end]
                                    .iter()
                                    .fold(0.0f32, |a, &s| a.max(s.abs()))
                            } else {
                                0.0
                            }
                        } else {
                            0.0
                        };
                        wf.push_tts(tts_peak.min(1.0));
                    }

                    if let Err(e) = helper.write_audio_frame(&out_buf) {
                        log::error!("[aec] failed to write to helper: {e}");
                        return;
                    }
                }
            }
            log::info!("[aec] pipeline stopped");
        });

        Ok(Self {
            _stream: stream,
            render_tx,
            waveform,
        })
    }

    /// Set the render reference: the full WAV samples of the chunk about to
    /// be played. Call this right when `PlaybackEngine::start()` is called.
    /// Samples must be f32 mono at `AEC_SAMPLE_RATE`.
    pub fn set_render_reference(&self, samples: Vec<f32>) {
        let _ = self
            .render_tx
            .try_send(RenderCommand::SetReference(samples));
    }

    /// Clear the render reference (playback stopped).
    pub fn clear_render_reference(&self) {
        let _ = self.render_tx.try_send(RenderCommand::ClearReference);
    }

    /// Get a clone of the render command sender (for passing to the playback
    /// engine or other components that need to feed the reference signal).
    pub fn render_sender(&self) -> mpsc::SyncSender<RenderCommand> {
        self.render_tx.clone()
    }

    /// Shared waveform data for UI visualization. The AEC thread updates this
    /// every 10ms frame with peak levels; the UI reads it on each render cycle.
    pub fn waveform_data(&self) -> Arc<Mutex<WaveformData>> {
        self.waveform.clone()
    }
}

/// Simple linear interpolation resampling.
fn linear_resample(input: &[f32], from_rate: usize, to_rate: usize) -> Vec<f32> {
    if from_rate == to_rate || input.is_empty() {
        return input.to_vec();
    }
    let ratio = from_rate as f64 / to_rate as f64;
    let out_len = (input.len() as f64 / ratio) as usize;
    let mut output = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let src_pos = i as f64 * ratio;
        let idx = src_pos as usize;
        let frac = (src_pos - idx as f64) as f32;
        let a = input[idx.min(input.len() - 1)];
        let b = input[(idx + 1).min(input.len() - 1)];
        output.push(a + (b - a) * frac);
    }
    output
}
