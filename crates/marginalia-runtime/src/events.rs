use std::sync::mpsc;

#[derive(Debug, Clone)]
pub enum RuntimeEvent {
    PlaybackFinished {
        document_id: String,
        section_index: usize,
        chunk_index: usize,
    },
    CommandRecognized {
        raw_text: String,
        command: Option<String>,
    },
    /// A synthesis job has been handed to the TTS backend but no audio is
    /// ready yet. Consumers use this to light up a "sintetizzando…" status
    /// indicator — the gap between `SynthesisStarted` and `SynthesisReady`
    /// is the user-visible latency they need feedback about.
    SynthesisStarted {
        document_id: String,
        section_index: usize,
        chunk_index: usize,
    },
    SynthesisReady {
        document_id: String,
        section_index: usize,
        chunk_index: usize,
        cache_hit: bool,
    },
    SessionRestored {
        session_id: String,
        document_id: String,
        section_index: usize,
        chunk_index: usize,
    },
    ChunkAdvanced {
        document_id: String,
        section_index: usize,
        chunk_index: usize,
    },
    SessionStopped {
        document_id: String,
    },
    /// Import job accepted — the UI should light up a blocking overlay so
    /// the user understands the app is working, not frozen. Fired at the
    /// start of `ingest_path` / `ingest_url`; the paired `IngestFinished`
    /// fires once chunking + DB save complete (success or error).
    IngestStarted {
        source: String,
    },
    IngestFinished {
        source: String,
        document_id: Option<String>,
        error: Option<String>,
    },
    /// Dictation job accepted — helper has switched to DICTATION mode and
    /// is recording. UI should show the live-note card as "listening".
    DictationStarted,
    /// Dictation finished: either a transcript was captured or an error
    /// occurred. Paired with the `DictationStarted` that preceded it.
    VoiceNoteTranscribed {
        text: String,
        duration_secs: f64,
        note_id: Option<String>,
        error: Option<String>,
    },
    /// The document being started uses a language that doesn't match the
    /// current TTS voice. Fired at `start_session` time when language
    /// detection finds a clear mismatch (high-confidence). The UI should
    /// prompt the user to switch voices; ignoring it still lets the
    /// playback proceed with the mismatched voice (espeak-ng/Kokoro will
    /// phonemize anyway, just with wrong-language pronunciation).
    VoiceMismatch {
        document_id: String,
        /// BCP-47 prefix of the detected language ("en", "it", "fr", …).
        detected_language: String,
        /// Currently-selected voice's language ("it-IT" → compared on
        /// prefix).
        current_language: String,
    },
    Error {
        message: String,
    },
}

pub type EventCallback = Box<dyn Fn(&RuntimeEvent) + Send + Sync>;

#[derive(Default)]
pub struct RuntimeEventSink {
    channels: Vec<mpsc::Sender<RuntimeEvent>>,
    callbacks: Vec<EventCallback>,
}

impl RuntimeEventSink {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn subscribe_channel(&mut self) -> mpsc::Receiver<RuntimeEvent> {
        let (tx, rx) = mpsc::channel();
        self.channels.push(tx);
        rx
    }

    pub fn subscribe_callback(&mut self, callback: EventCallback) {
        self.callbacks.push(callback);
    }

    pub fn emit(&mut self, event: RuntimeEvent) {
        for cb in &self.callbacks {
            cb(&event);
        }
        self.channels.retain(|tx| tx.send(event.clone()).is_ok());
    }
}
