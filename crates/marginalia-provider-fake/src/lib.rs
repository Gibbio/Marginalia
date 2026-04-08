pub mod events;
pub mod llm;
pub mod playback;
pub mod storage;
pub mod stt;
pub mod tts;

pub use events::RecordingEventPublisher;
pub use llm::{FakeRewriteGenerator, FakeTopicSummarizer};
pub use playback::FakePlaybackEngine;
pub use storage::{
    InMemoryDocumentRepository, InMemoryNoteRepository, InMemoryRewriteDraftRepository,
    InMemorySessionRepository,
};
pub use stt::{FakeCommandRecognizer, FakeDictationTranscriber, FakeInterruptMonitor};
pub use tts::FakeSpeechSynthesizer;
