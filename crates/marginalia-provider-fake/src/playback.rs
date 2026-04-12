use marginalia_core::domain::{Document, PlaybackState, ReadingPosition};
use marginalia_core::ports::{
    PlaybackEngine, PlaybackSnapshot, ProviderCapabilities, ProviderExecutionMode, SynthesisResult,
};

#[derive(Debug, Clone)]
pub struct FakePlaybackEngine {
    snapshot: PlaybackSnapshot,
}

impl Default for FakePlaybackEngine {
    fn default() -> Self {
        Self {
            snapshot: PlaybackSnapshot {
                state: PlaybackState::Stopped,
                last_action: "initialized".to_string(),
                document_id: None,
                anchor: None,
                progress_units: 0,
                audio_reference: None,
                provider_name: Some("fake-playback".to_string()),
                process_id: None,
            },
        }
    }
}

impl FakePlaybackEngine {
    pub fn new() -> Self {
        Self::default()
    }
}

impl PlaybackEngine for FakePlaybackEngine {
    fn describe_capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            provider_name: "fake-playback".to_string(),
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
        self.snapshot.state = PlaybackState::Playing;
        self.snapshot.last_action = "start".to_string();
        self.snapshot.document_id = Some(document.document_id.clone());
        self.snapshot.anchor = Some(position.anchor());
        self.snapshot.progress_units = position.chunk_index;
        self.snapshot.audio_reference = synthesis.map(|synthesis| synthesis.audio_reference);
        self.snapshot.clone()
    }

    fn pause(&mut self) -> PlaybackSnapshot {
        self.snapshot.state = PlaybackState::Paused;
        self.snapshot.last_action = "pause".to_string();
        self.snapshot.clone()
    }

    fn resume(&mut self) -> PlaybackSnapshot {
        self.snapshot.state = PlaybackState::Playing;
        self.snapshot.last_action = "resume".to_string();
        self.snapshot.clone()
    }

    fn stop(&mut self) -> PlaybackSnapshot {
        self.snapshot.state = PlaybackState::Stopped;
        self.snapshot.last_action = "stop".to_string();
        self.snapshot.clone()
    }

    fn seek(&mut self, position: &ReadingPosition) -> PlaybackSnapshot {
        self.snapshot.anchor = Some(position.anchor());
        self.snapshot.progress_units = position.chunk_index;
        self.snapshot.last_action = "seek".to_string();
        self.snapshot.clone()
    }

    fn snapshot(&self) -> PlaybackSnapshot {
        self.snapshot.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::FakePlaybackEngine;
    use marginalia_core::domain::{
        Document, DocumentChunk, DocumentSection, PlaybackState, ReadingPosition,
    };
    use marginalia_core::ports::{PlaybackEngine, SynthesisResult};
    use std::collections::HashMap;
    use std::path::PathBuf;

    #[test]
    fn fake_playback_engine_updates_snapshot_during_controls() {
        let mut engine = FakePlaybackEngine::new();
        let document = Document {
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
        };
        let position = ReadingPosition::default();

        let started = engine.start(
            &document,
            &position,
            Some(SynthesisResult {
                provider_name: "fake-tts".to_string(),
                voice: "narrator".to_string(),
                content_type: "audio/wav".to_string(),
                audio_reference: "/tmp/audio.wav".to_string(),
                byte_length: 42,
                text_excerpt: "Alpha".to_string(),
                metadata: HashMap::new(),
            }),
        );
        assert_eq!(started.state, PlaybackState::Playing);

        let paused = engine.pause();
        assert_eq!(paused.state, PlaybackState::Paused);

        let stopped = engine.stop();
        assert_eq!(stopped.state, PlaybackState::Stopped);
    }
}
