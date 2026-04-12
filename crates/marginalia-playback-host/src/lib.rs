use marginalia_core::domain::{Document, PlaybackState, ReadingPosition};
use marginalia_core::ports::{
    PlaybackEngine, PlaybackSnapshot, ProviderCapabilities, ProviderExecutionMode, SynthesisResult,
};
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink};
use std::fs::File;
use std::io::BufReader;

/// Wrapper to make rodio's OutputStream Send.
/// The stream is created and dropped on the same thread context;
/// we only hold it as a drop guard to keep the audio device open.
struct SendOutputStream(#[allow(dead_code)] OutputStream);
unsafe impl Send for SendOutputStream {}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HostPlaybackConfig {
    pub command_template: Option<Vec<String>>,
}

pub struct HostPlaybackEngine {
    _stream: Option<SendOutputStream>,
    stream_handle: Option<OutputStreamHandle>,
    sink: Option<Sink>,
    snapshot: PlaybackSnapshot,
}

impl Default for HostPlaybackEngine {
    fn default() -> Self {
        let (stream, handle) = match OutputStream::try_default() {
            Ok((s, h)) => (Some(SendOutputStream(s)), Some(h)),
            Err(e) => {
                log::warn!("[playback] audio output not available: {e}");
                (None, None)
            }
        };
        Self {
            _stream: stream,
            stream_handle: handle,
            sink: None,
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

    /// Check if current playback has finished (for auto-advance).
    pub fn is_finished(&self) -> bool {
        self.sink.as_ref().is_some_and(|s| s.empty())
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

        let Some(handle) = &self.stream_handle else {
            self.snapshot.state = PlaybackState::Stopped;
            self.snapshot.last_action = "start-no-audio-device".to_string();
            return self.snapshot();
        };

        let file = match File::open(&synthesis.audio_reference) {
            Ok(f) => f,
            Err(_) => {
                self.snapshot.state = PlaybackState::Stopped;
                self.snapshot.last_action = "start-file-not-found".to_string();
                return self.snapshot();
            }
        };

        let source = match Decoder::new(BufReader::new(file)) {
            Ok(s) => s,
            Err(_) => {
                self.snapshot.state = PlaybackState::Stopped;
                self.snapshot.last_action = "start-decode-failed".to_string();
                return self.snapshot();
            }
        };

        let sink = match Sink::try_new(handle) {
            Ok(s) => s,
            Err(_) => {
                self.snapshot.state = PlaybackState::Stopped;
                self.snapshot.last_action = "start-sink-failed".to_string();
                return self.snapshot();
            }
        };

        sink.append(source);
        self.sink = Some(sink);
        self.snapshot.state = PlaybackState::Playing;
        self.snapshot.last_action = "start".to_string();
        self.snapshot.audio_reference = Some(synthesis.audio_reference);
        self.snapshot()
    }

    fn pause(&mut self) -> PlaybackSnapshot {
        if let Some(sink) = &self.sink {
            if self.snapshot.state == PlaybackState::Playing {
                sink.pause();
                self.snapshot.state = PlaybackState::Paused;
            }
        }
        self.snapshot.last_action = "pause".to_string();
        self.snapshot()
    }

    fn resume(&mut self) -> PlaybackSnapshot {
        if let Some(sink) = &self.sink {
            if self.snapshot.state == PlaybackState::Paused {
                sink.play();
                self.snapshot.state = PlaybackState::Playing;
            }
        }
        self.snapshot.last_action = "resume".to_string();
        self.snapshot()
    }

    fn stop(&mut self) -> PlaybackSnapshot {
        if let Some(sink) = self.sink.take() {
            sink.stop();
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
        if let Some(sink) = &self.sink {
            if sink.empty() && snapshot.state == PlaybackState::Playing {
                snapshot.state = PlaybackState::Stopped;
                snapshot.last_action = "completed".to_string();
            }
        }
        snapshot
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

    #[test]
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
