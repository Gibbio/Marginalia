//! Acoustic echo cancellation pipeline.
//!
//! Opens the default mic via cpal, runs WebRTC AEC3 to subtract the TTS
//! playback signal (render/reference) from the mic input (capture/near-end),
//! and writes the cleaned audio frames to the Swift helper's stdin.
//!
//! The pipeline runs on its own thread. It consumes render frames from a
//! crossbeam-style channel fed by the playback host whenever a TTS chunk
//! is playing.

use crate::AppleHelperShared;
use aec3::voip::VoipAec3;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::mpsc;
use std::sync::Arc;

/// Number of samples in a 10ms frame at the AEC sample rate.
const FRAME_SAMPLES: usize = (crate::AEC_SAMPLE_RATE as usize) / 100; // 240 at 24kHz

/// Handle to the running AEC pipeline. Dropping it stops the mic stream.
pub struct AecPipeline {
    _stream: cpal::Stream,
    render_tx: mpsc::SyncSender<Vec<f32>>,
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

        // Channel: cpal callback → AEC processing thread (mic frames).
        let (mic_tx, mic_rx) = mpsc::sync_channel::<Vec<f32>>(64);
        // Channel: playback host → AEC processing thread (render/reference frames).
        let (render_tx, render_rx) = mpsc::sync_channel::<Vec<f32>>(64);

        // Mic capture stream — records f32 mono at device rate.
        let stream = device
            .build_input_stream(
                &stream_config,
                move |data: &[f32], _| {
                    // Downmix to mono.
                    let mono: Vec<f32> = data
                        .chunks(channels)
                        .map(|frame| frame[0])
                        .collect();
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
        std::thread::spawn(move || {
            let mut aec = match VoipAec3::builder(target_rate, 1, 1)
                .render_sample_rate_hz(target_rate)
                .capture_sample_rate_hz(target_rate)
                .build()
            {
                Ok(a) => a,
                Err(e) => {
                    log::error!("[aec] failed to build AEC3 pipeline: {e}");
                    return;
                }
            };

            // Accumulation buffers for resampled mic data.
            let mut mic_accum: Vec<f32> = Vec::with_capacity(FRAME_SAMPLES * 4);
            let mut out_buf = vec![0.0f32; FRAME_SAMPLES];

            loop {
                // Receive mic samples (may be at device_rate ≠ target_rate).
                let raw_mic = match mic_rx.recv() {
                    Ok(v) => v,
                    Err(_) => break,
                };

                // Resample to target_rate if needed.
                let resampled = if device_rate as usize != target_rate {
                    linear_resample(&raw_mic, device_rate as usize, target_rate)
                } else {
                    raw_mic
                };
                mic_accum.extend_from_slice(&resampled);

                // Process complete 10ms frames.
                while mic_accum.len() >= FRAME_SAMPLES {
                    let frame: Vec<f32> = mic_accum.drain(..FRAME_SAMPLES).collect();

                    // Drain any pending render frames first (AEC3 requirement:
                    // render before capture).
                    while let Ok(render) = render_rx.try_recv() {
                        // Render frames should be at target_rate already
                        // (Kokoro = 24kHz). Feed in FRAME_SAMPLES chunks.
                        for chunk in render.chunks(FRAME_SAMPLES) {
                            if chunk.len() == FRAME_SAMPLES {
                                if let Err(e) = aec.handle_render_frame(chunk) {
                                    log::warn!("[aec] render frame error: {e}");
                                }
                            }
                        }
                    }

                    // Process capture through AEC.
                    match aec.process_capture_frame(&frame, false, &mut out_buf) {
                        Ok(_metrics) => {}
                        Err(e) => {
                            log::warn!("[aec] capture frame error: {e}");
                            out_buf.copy_from_slice(&frame); // pass through on error
                        }
                    }

                    // Send cleaned audio to the Swift helper.
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
        })
    }

    /// Feed render (reference / far-end) audio samples to the AEC. Call this
    /// when the playback engine is about to play a chunk — pass the WAV
    /// samples (f32, mono, at AEC_SAMPLE_RATE). The AEC thread will drain
    /// these before processing each mic frame.
    pub fn feed_render(&self, samples: &[f32]) {
        let _ = self.render_tx.try_send(samples.to_vec());
    }

    /// Access the render channel sender (for passing to the playback host).
    pub fn render_sender(&self) -> mpsc::SyncSender<Vec<f32>> {
        self.render_tx.clone()
    }
}

/// Simple linear interpolation resampling. Good enough for voice at these
/// rates; no external dep needed.
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
        let frac = src_pos - idx as f64;
        let a = input[idx.min(input.len() - 1)];
        let b = input[(idx + 1).min(input.len() - 1)];
        output.push(a + (b - a) * frac as f32);
    }
    output
}
