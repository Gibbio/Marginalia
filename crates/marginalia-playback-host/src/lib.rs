use marginalia_core::domain::{Document, PlaybackState, ReadingPosition};
use marginalia_core::ports::{
    PlaybackEngine, PlaybackSnapshot, ProviderCapabilities, ProviderExecutionMode, SynthesisResult,
};
use std::process::{Child, Command, Stdio};

const AUDIO_PLACEHOLDER: &str = "{audio}";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HostPlaybackConfig {
    pub command_template: Option<Vec<String>>,
}

#[derive(Debug)]
pub struct HostPlaybackEngine {
    command_template: Option<Vec<String>>,
    snapshot: PlaybackSnapshot,
    child: Option<Child>,
}

impl Default for HostPlaybackEngine {
    fn default() -> Self {
        Self::new(HostPlaybackConfig::default())
    }
}

impl HostPlaybackEngine {
    pub fn new(config: HostPlaybackConfig) -> Self {
        Self {
            command_template: config.command_template.or_else(detect_command_template),
            snapshot: PlaybackSnapshot {
                state: PlaybackState::Stopped,
                last_action: "initialized".to_string(),
                document_id: None,
                anchor: None,
                progress_units: 0,
                audio_reference: None,
                provider_name: Some("host-playback".to_string()),
                process_id: None,
            },
            child: None,
        }
    }

    pub fn with_command_template(command_template: Vec<String>) -> Self {
        Self::new(HostPlaybackConfig {
            command_template: Some(command_template),
        })
    }

    pub fn command_template(&self) -> Option<&[String]> {
        self.command_template.as_deref()
    }

    fn prepare_command(&self, audio_reference: &str) -> Option<Command> {
        let template = self.command_template.as_ref()?;
        let (program, args) = template.split_first()?;
        let mut command = Command::new(program);
        for arg in args {
            if arg == AUDIO_PLACEHOLDER {
                command.arg(audio_reference);
            } else {
                command.arg(arg);
            }
        }
        command.stdin(Stdio::null());
        command.stdout(Stdio::null());
        command.stderr(Stdio::null());
        Some(command)
    }

    fn refresh_state(&mut self) {
        if let Some(child) = self.child.as_mut() {
            match child.try_wait() {
                Ok(Some(_)) => {
                    if self.snapshot.state == PlaybackState::Playing {
                        self.snapshot.last_action = "completed".to_string();
                    }
                    self.snapshot.state = PlaybackState::Stopped;
                    self.snapshot.process_id = None;
                    self.child = None;
                }
                Ok(None) => {
                    self.snapshot.process_id = Some(child.id());
                }
                Err(_) => {
                    self.snapshot.last_action = "monitor-failed".to_string();
                    self.snapshot.state = PlaybackState::Stopped;
                    self.snapshot.process_id = None;
                    self.child = None;
                }
            }
        }
    }
}

impl PlaybackEngine for HostPlaybackEngine {
    fn describe_capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            provider_name: "host-playback".to_string(),
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
            self.child = None;
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
        self.snapshot.audio_reference = synthesis.as_ref().map(|item| item.audio_reference.clone());

        let Some(synthesis) = synthesis else {
            self.snapshot.state = PlaybackState::Stopped;
            self.snapshot.last_action = "start-missing-audio".to_string();
            return self.snapshot();
        };

        let Some(mut command) = self.prepare_command(&synthesis.audio_reference) else {
            self.snapshot.state = PlaybackState::Stopped;
            self.snapshot.last_action = "start-no-command".to_string();
            return self.snapshot();
        };

        match command.spawn() {
            Ok(child) => {
                self.snapshot.state = PlaybackState::Playing;
                self.snapshot.last_action = "start".to_string();
                self.snapshot.process_id = Some(child.id());
                self.snapshot.audio_reference = Some(synthesis.audio_reference);
                self.child = Some(child);
            }
            Err(_) => {
                self.snapshot.state = PlaybackState::Stopped;
                self.snapshot.last_action = "start-spawn-failed".to_string();
                self.snapshot.process_id = None;
                self.child = None;
            }
        }

        self.snapshot()
    }

    fn pause(&mut self) -> PlaybackSnapshot {
        self.refresh_state();
        #[cfg(unix)]
        if let Some(pid) = self.snapshot.process_id {
            if self.snapshot.state == PlaybackState::Playing {
                let _ = Command::new("kill")
                    .arg("-STOP")
                    .arg(pid.to_string())
                    .status();
                self.snapshot.state = PlaybackState::Paused;
            }
        }
        self.snapshot.last_action = "pause".to_string();
        self.snapshot()
    }

    fn resume(&mut self) -> PlaybackSnapshot {
        self.refresh_state();
        #[cfg(unix)]
        if let Some(pid) = self.snapshot.process_id {
            if self.snapshot.state == PlaybackState::Paused {
                let _ = Command::new("kill")
                    .arg("-CONT")
                    .arg(pid.to_string())
                    .status();
                self.snapshot.state = PlaybackState::Playing;
                self.snapshot.progress_units += 1;
            }
        }
        self.snapshot.last_action = "resume".to_string();
        self.snapshot()
    }

    fn stop(&mut self) -> PlaybackSnapshot {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        } else if let Some(pid) = self.snapshot.process_id {
            #[cfg(unix)]
            {
                let _ = Command::new("kill")
                    .arg("-TERM")
                    .arg(pid.to_string())
                    .status();
            }
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
        if let Some(child) = self.child.as_ref() {
            snapshot.process_id = Some(child.id());
        }
        snapshot
    }
}

fn detect_command_template() -> Option<Vec<String>> {
    if cfg!(target_os = "macos") && command_exists("afplay") {
        return Some(vec!["afplay".to_string(), AUDIO_PLACEHOLDER.to_string()]);
    }
    if command_exists("aplay") {
        return Some(vec!["aplay".to_string(), AUDIO_PLACEHOLDER.to_string()]);
    }
    if command_exists("ffplay") {
        return Some(vec![
            "ffplay".to_string(),
            "-nodisp".to_string(),
            "-autoexit".to_string(),
            "-loglevel".to_string(),
            "quiet".to_string(),
            AUDIO_PLACEHOLDER.to_string(),
        ]);
    }
    None
}

fn command_exists(command: &str) -> bool {
    Command::new("sh")
        .arg("-lc")
        .arg(format!("command -v {command} >/dev/null 2>&1"))
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::{HostPlaybackEngine, AUDIO_PLACEHOLDER};
    use marginalia_core::domain::{
        Document, DocumentChunk, DocumentSection, PlaybackState, ReadingPosition,
    };
    use marginalia_core::ports::{PlaybackEngine, SynthesisResult};
    use std::collections::HashMap;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(1);

    #[test]
    fn host_playback_engine_starts_with_custom_command_template() {
        let audio_path = temp_wav_path();
        write_silence_wav(&audio_path, 1_000);

        let mut engine = HostPlaybackEngine::with_command_template(vec![
            "sh".to_string(),
            "-lc".to_string(),
            "cat \"$1\" >/dev/null".to_string(),
            "sh".to_string(),
            AUDIO_PLACEHOLDER.to_string(),
        ]);

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
        assert!(started.process_id.is_some());

        let _ = engine.stop();
        let _ = fs::remove_file(audio_path);
    }

    #[test]
    fn host_playback_engine_reports_missing_command() {
        let mut engine = HostPlaybackEngine::with_command_template(vec![]);
        let started = engine.start(
            &test_document(),
            &ReadingPosition::default(),
            Some(SynthesisResult {
                provider_name: "fake-tts".to_string(),
                voice: "narrator".to_string(),
                content_type: "audio/wav".to_string(),
                audio_reference: "/tmp/does-not-matter.wav".to_string(),
                byte_length: 1,
                text_excerpt: "Alpha".to_string(),
                metadata: HashMap::new(),
            }),
        );

        assert_eq!(started.state, PlaybackState::Stopped);
        assert_eq!(started.last_action, "start-no-command");
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
        std::env::temp_dir().join(format!("marginalia-playback-host-{id}.wav"))
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
