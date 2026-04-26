use marginalia_core::domain::{Document, PlaybackState, ReadingPosition};
use marginalia_core::ports::{
    PlaybackEngine, PlaybackSnapshot, ProviderCapabilities, ProviderExecutionMode, SynthesisResult,
};
use rodio::{Decoder, DeviceSinkBuilder, MixerDeviceSink, Player, Source};
use std::fs::File;
use std::io::BufReader;

/// Wrapper to make rodio's MixerDeviceSink Send.
/// The sink is created and dropped on the same thread; we only hold it
/// as a drop guard to keep the audio device open.
struct SendDeviceSink(#[allow(dead_code)] MixerDeviceSink);
unsafe impl Send for SendDeviceSink {}

/// Same Send override for the persistent `Player`. rodio 0.22's audio
/// thread only pulls samples reliably when the Player was created on
/// the same thread as the underlying cpal stream. We honour that by
/// constructing it once at engine init (alongside the DeviceSink) and
/// keep reusing it from any thread that holds the runtime mutex —
/// `append`/`pause`/`clear` all just push to internal channels which
/// are MPSC-safe.
struct SendPlayer(Player);
unsafe impl Send for SendPlayer {}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HostPlaybackConfig {
    pub command_template: Option<Vec<String>>,
}

pub struct HostPlaybackEngine {
    /// Holds the device sink open for the lifetime of the engine. The
    /// `Player` borrows the mixer from this; the player is created on
    /// the same thread to keep cpal's audio callback alive.
    device_sink: Option<SendDeviceSink>,
    /// Persistent player. Created once next to `device_sink` so the
    /// rodio audio thread is wired correctly. Each `start()` calls
    /// `clear()` then `play()` + `append(source)`. Recreating per
    /// chunk caused silent playback in cross-thread scenarios because
    /// rodio 0.22's audio thread doesn't pick up sources from players
    /// created on a different thread than the cpal stream.
    player: Option<SendPlayer>,
    snapshot: PlaybackSnapshot,
    /// Optional callback invoked with the f32 mono samples of each WAV chunk
    /// right before playback starts. Used by the AEC pipeline as the render
    /// reference signal.
    on_play_samples: Option<Box<dyn Fn(Vec<f32>) + Send>>,
    /// Fired on `pause()` so the AEC pipeline can freeze its render
    /// reference (don't advance render_pos, set tts_peak = 0). Separate
    /// from "stopped" so resume() can pick up where pause left off
    /// without a fresh `SetReference` (which would reset render_pos
    /// to 0 and desync from the actual sink position).
    on_playback_paused: Option<Box<dyn Fn() + Send>>,
    /// Fired on `resume()` — flips the AEC pipeline back to advancing
    /// render_pos through the existing reference buffer.
    on_playback_resumed: Option<Box<dyn Fn() + Send>>,
    /// Fired on `stop()` so AEC drops the render reference entirely.
    /// The next chunk's `start()` will install a fresh one.
    on_playback_cleared: Option<Box<dyn Fn() + Send>>,
    /// Linear volume 0.0 – 1.0+. Persisted across sink recreations so
    /// new chunks inherit the current level.
    volume: f32,
}

impl Default for HostPlaybackEngine {
    fn default() -> Self {
        let (device_sink, player) = match DeviceSinkBuilder::open_default_sink() {
            Ok(sink) => {
                // Construct the player on the same thread as the device
                // sink so rodio's audio callback recognises it. Shipping
                // it across threads later (via SendPlayer) is fine —
                // append/clear/pause only touch MPSC channels.
                let p = Player::connect_new(sink.mixer());
                // Idle until the first chunk arrives.
                p.pause();
                (Some(SendDeviceSink(sink)), Some(SendPlayer(p)))
            }
            Err(e) => {
                log::warn!("[playback] audio output not available: {e}");
                (None, None)
            }
        };
        Self {
            device_sink,
            player,
            on_play_samples: None,
            on_playback_paused: None,
            on_playback_resumed: None,
            on_playback_cleared: None,
            volume: 1.0,
            snapshot: PlaybackSnapshot {
                state: PlaybackState::Stopped,
                last_action: "initialized".to_string(),
                document_id: None,
                anchor: None,
                progress_units: 0,
                audio_reference: None,
                provider_name: Some("rodio".to_string()),
                process_id: None,
            },
        }
    }
}

impl HostPlaybackEngine {
    pub fn new(_config: HostPlaybackConfig) -> Self {
        Self::default()
    }

    /// Register a callback to receive the f32 mono samples of each WAV chunk
    /// right before playback starts. The AEC pipeline uses this as its render
    /// reference. Pass `None` to clear.
    pub fn set_play_samples_callback(&mut self, cb: Box<dyn Fn(Vec<f32>) + Send>) {
        self.on_play_samples = Some(cb);
    }

    /// Register a callback fired when playback pauses — AEC uses this
    /// to freeze its render reference so the TTS-level meter goes flat
    /// while the sink is silent, without losing the buffer entirely.
    pub fn set_playback_paused_callback(&mut self, cb: Box<dyn Fn() + Send>) {
        self.on_playback_paused = Some(cb);
    }

    /// Register a callback fired when playback resumes after a pause.
    /// The AEC pipeline starts advancing through the cached reference
    /// again so echo cancellation kicks back in for the rest of the chunk.
    pub fn set_playback_resumed_callback(&mut self, cb: Box<dyn Fn() + Send>) {
        self.on_playback_resumed = Some(cb);
    }

    /// Register a callback fired when playback stops outright (chunk
    /// transitions, session stop). AEC drops the reference; the next
    /// `start()` will install a fresh one.
    pub fn set_playback_cleared_callback(&mut self, cb: Box<dyn Fn() + Send>) {
        self.on_playback_cleared = Some(cb);
    }

    /// Check if current playback has finished (for auto-advance).
    pub fn is_finished(&self) -> bool {
        self.player.as_ref().is_some_and(|p| p.0.empty())
            && self.snapshot.state == PlaybackState::Playing
    }
}

impl PlaybackEngine for HostPlaybackEngine {
    fn describe_capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            provider_name: "rodio".to_string(),
            interface_kind: "playback".to_string(),
            supported_languages: vec!["it".to_string(), "en".to_string()],
            supports_streaming: false,
            supports_partial_results: false,
            supports_timestamps: false,
            low_latency_suitable: true,
            offline_capable: true,
            execution_mode: ProviderExecutionMode::Local,
        }
    }

    fn hydrate(&mut self, snapshot: Option<PlaybackSnapshot>) {
        if let Some(snapshot) = snapshot {
            self.snapshot = snapshot;
        } else {
            self.snapshot.state = PlaybackState::Stopped;
            self.snapshot.last_action = "hydrated-empty".to_string();
            self.snapshot.document_id = None;
            self.snapshot.anchor = None;
            self.snapshot.audio_reference = None;
            self.snapshot.process_id = None;
        }
    }

    fn start(
        &mut self,
        document: &Document,
        position: &ReadingPosition,
        synthesis: Option<SynthesisResult>,
    ) -> PlaybackSnapshot {
        self.stop();
        self.snapshot.document_id = Some(document.document_id.clone());
        self.snapshot.anchor = Some(position.anchor());
        self.snapshot.progress_units = position.chunk_index;
        self.snapshot.audio_reference = synthesis.as_ref().map(|s| s.audio_reference.clone());

        let Some(synthesis) = synthesis else {
            self.snapshot.state = PlaybackState::Stopped;
            self.snapshot.last_action = "start-missing-audio".to_string();
            return self.snapshot();
        };

        let Some(player) = &self.player else {
            self.snapshot.state = PlaybackState::Stopped;
            self.snapshot.last_action = "start-no-audio-device".to_string();
            return self.snapshot();
        };
        // Drop unused binding silently — we touch the device sink only
        // implicitly via the player. Kept around to keep the cpal
        // stream alive.
        let _ = &self.device_sink;

        let file = match File::open(&synthesis.audio_reference) {
            Ok(f) => f,
            Err(_) => {
                self.snapshot.state = PlaybackState::Stopped;
                self.snapshot.last_action = "start-file-not-found".to_string();
                return self.snapshot();
            }
        };

        let source = match Decoder::try_from(BufReader::new(file)) {
            Ok(s) => s,
            Err(e) => {
                // Corrupted cached FLAC (truncated write, disk full
                // mid-save, …). Remove it so the next synthesize_cached
                // call for the same key re-generates instead of hitting
                // the same broken file forever.
                log::warn!(
                    "[playback] decode failed for {}: {e} — purging cache entry",
                    synthesis.audio_reference
                );
                let _ = std::fs::remove_file(&synthesis.audio_reference);
                self.snapshot.state = PlaybackState::Stopped;
                self.snapshot.last_action = "start-decode-failed".to_string();
                return self.snapshot();
            }
        };

        // Feed the f32 mono samples to the AEC render callback (if set)
        // BEFORE starting playback, so the AEC thread has the reference
        // ready. rodio 0.22's Decoder yields `f32` samples (Sample =
        // Float), so no i16 conversion is needed; just downmix to mono
        // by taking the first channel of each frame.
        if let Some(ref cb) = self.on_play_samples {
            if let Ok(f2) = File::open(&synthesis.audio_reference) {
                if let Ok(decoder) = Decoder::try_from(BufReader::new(f2)) {
                    let channels = decoder.channels().get() as usize;
                    let samples: Vec<f32> = decoder.collect();
                    let samples_f32: Vec<f32> = if channels <= 1 {
                        samples
                    } else {
                        samples.chunks(channels).map(|frame| frame[0]).collect()
                    };
                    cb(samples_f32);
                }
            }
        }

        // Drain anything the previous chunk left queued (clear() also
        // pauses the player), then resume + append + set volume. The
        // persistent player must stay alive — recreating it here would
        // break audio output when start() runs from a thread other
        // than the one that created the cpal stream.
        player.0.clear();
        player.0.play();
        player.0.set_volume(self.volume);
        player.0.append(source);
        self.snapshot.state = PlaybackState::Playing;
        self.snapshot.last_action = "start".to_string();
        self.snapshot.audio_reference = Some(synthesis.audio_reference);
        self.snapshot()
    }

    fn pause(&mut self) -> PlaybackSnapshot {
        if let Some(player) = &self.player {
            if self.snapshot.state == PlaybackState::Playing {
                player.0.pause();
                self.snapshot.state = PlaybackState::Paused;
            }
        }
        // Tell the AEC pipeline to freeze its render reference —
        // render_pos stops advancing and the TTS meter goes flat
        // while the sink is silent. The reference stays loaded so
        // resume() picks up exactly where we left off.
        if let Some(cb) = &self.on_playback_paused {
            cb();
        }
        self.snapshot.last_action = "pause".to_string();
        self.snapshot()
    }

    fn resume(&mut self) -> PlaybackSnapshot {
        if let Some(player) = &self.player {
            if self.snapshot.state == PlaybackState::Paused {
                player.0.play();
                self.snapshot.state = PlaybackState::Playing;
            }
        }
        if let Some(cb) = &self.on_playback_resumed {
            cb();
        }
        self.snapshot.last_action = "resume".to_string();
        self.snapshot()
    }

    fn stop(&mut self) -> PlaybackSnapshot {
        // Drain the queue but keep the persistent player alive — tearing
        // it down would force a fresh `Player::connect_new` on the next
        // start(), which only works on the cpal-stream thread.
        if let Some(player) = &self.player {
            player.0.clear();
        }
        if let Some(cb) = &self.on_playback_cleared {
            cb();
        }
        self.snapshot.state = PlaybackState::Stopped;
        self.snapshot.last_action = "stop".to_string();
        self.snapshot.process_id = None;
        self.snapshot()
    }

    fn seek(&mut self, position: &ReadingPosition) -> PlaybackSnapshot {
        self.stop();
        self.snapshot.anchor = Some(position.anchor());
        self.snapshot.progress_units = position.chunk_index;
        self.snapshot.state = PlaybackState::Paused;
        self.snapshot.last_action = "seek".to_string();
        self.snapshot()
    }

    fn snapshot(&self) -> PlaybackSnapshot {
        let mut snapshot = self.snapshot.clone();
        // Update state if playback finished naturally
        if let Some(player) = &self.player {
            if player.0.empty() && snapshot.state == PlaybackState::Playing {
                snapshot.state = PlaybackState::Stopped;
                snapshot.last_action = "completed".to_string();
            }
        }
        snapshot
    }

    fn set_volume(&mut self, volume: f32) {
        // Clamp negative to 0.0 but leave the upper end open — rodio
        // accepts >1.0 as software amplification, which power users can
        // request via a future "più forte" voice command extension.
        let clamped = volume.max(0.0);
        self.volume = clamped;
        if let Some(player) = &self.player {
            player.0.set_volume(clamped);
        }
    }

    fn volume(&self) -> f32 {
        self.volume
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use marginalia_core::domain::{DocumentChunk, DocumentSection};
    use std::collections::HashMap;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(1);

    // HostPlaybackEngine opens a real OS audio device via rodio, which
    // requires an available sound card. Headless Linux CI runners don't
    // have one, so these tests are `#[ignore]`d by default and run locally
    // (or on a workstation runner) with `cargo test -- --ignored`.
    #[test]
    #[ignore = "requires a real audio device"]
    fn playback_engine_starts_and_stops() {
        let audio_path = temp_wav_path();
        write_silence_wav(&audio_path, 1_000);

        let mut engine = HostPlaybackEngine::default();
        let started = engine.start(
            &test_document(),
            &ReadingPosition::default(),
            Some(SynthesisResult {
                provider_name: "fake-tts".to_string(),
                voice: "narrator".to_string(),
                content_type: "audio/wav".to_string(),
                audio_reference: audio_path.display().to_string(),
                byte_length: 1000,
                text_excerpt: "Alpha".to_string(),
                metadata: HashMap::new(),
            }),
        );

        assert_eq!(started.state, PlaybackState::Playing);

        let stopped = engine.stop();
        assert_eq!(stopped.state, PlaybackState::Stopped);

        let _ = fs::remove_file(audio_path);
    }

    #[test]
    #[ignore = "requires a real audio device"]
    fn playback_engine_pause_resume() {
        let audio_path = temp_wav_path();
        write_silence_wav(&audio_path, 48_000); // 3 seconds of silence

        let mut engine = HostPlaybackEngine::default();
        engine.start(
            &test_document(),
            &ReadingPosition::default(),
            Some(SynthesisResult {
                provider_name: "fake-tts".to_string(),
                voice: "narrator".to_string(),
                content_type: "audio/wav".to_string(),
                audio_reference: audio_path.display().to_string(),
                byte_length: 1000,
                text_excerpt: "Alpha".to_string(),
                metadata: HashMap::new(),
            }),
        );

        let paused = engine.pause();
        assert_eq!(paused.state, PlaybackState::Paused);

        let resumed = engine.resume();
        assert_eq!(resumed.state, PlaybackState::Playing);

        let _ = engine.stop();
        let _ = fs::remove_file(audio_path);
    }

    fn test_document() -> Document {
        Document {
            document_id: "doc-1".to_string(),
            title: "Doc".to_string(),
            source_path: PathBuf::from("/tmp/doc.md"),
            sections: vec![DocumentSection {
                index: 0,
                title: "Intro".to_string(),
                chunks: vec![DocumentChunk {
                    index: 0,
                    text: "Alpha".to_string(),
                    char_start: 0,
                    char_end: 5,
                }],
                source_anchor: Some("section:0".to_string()),
            }],
            imported_at: chrono::Utc::now(),
        }
    }

    fn temp_wav_path() -> PathBuf {
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("marginalia-playback-test-{id}.wav"))
    }

    fn write_silence_wav(path: &PathBuf, sample_count: usize) {
        let sample_rate = 16_000u32;
        let channels = 1u16;
        let bits_per_sample = 16u16;
        let bytes_per_sample = (bits_per_sample / 8) as usize;
        let data_size = sample_count * bytes_per_sample;
        let byte_rate = sample_rate * channels as u32 * bytes_per_sample as u32;
        let block_align = channels * bits_per_sample / 8;
        let riff_size = 36 + data_size as u32;

        let mut bytes = Vec::with_capacity(44 + data_size);
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&riff_size.to_le_bytes());
        bytes.extend_from_slice(b"WAVE");
        bytes.extend_from_slice(b"fmt ");
        bytes.extend_from_slice(&16u32.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&channels.to_le_bytes());
        bytes.extend_from_slice(&sample_rate.to_le_bytes());
        bytes.extend_from_slice(&byte_rate.to_le_bytes());
        bytes.extend_from_slice(&block_align.to_le_bytes());
        bytes.extend_from_slice(&bits_per_sample.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&(data_size as u32).to_le_bytes());
        bytes.resize(44 + data_size, 0);
        fs::write(path, bytes).unwrap();
    }
}
