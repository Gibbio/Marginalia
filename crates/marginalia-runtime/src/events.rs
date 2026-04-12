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
    Error {
        message: String,
    },
}

pub type EventCallback = Box<dyn Fn(&RuntimeEvent) + Send + Sync>;

pub struct RuntimeEventSink {
    channels: Vec<mpsc::Sender<RuntimeEvent>>,
    callbacks: Vec<EventCallback>,
}

impl RuntimeEventSink {
    pub fn new() -> Self {
        Self {
            channels: Vec::new(),
            callbacks: Vec::new(),
        }
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
