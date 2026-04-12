use crate::domain::{Document, PlaybackState, ReadingPosition};
use crate::ports::{capabilities::ProviderCapabilities, tts::SynthesisResult};

/// Point-in-time snapshot of the playback engine state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaybackSnapshot {
    /// Current playback state (Playing, Paused, Stopped).
    pub state: PlaybackState,
    /// Label of the last action performed (e.g. "start", "completed").
    pub last_action: String,
    /// ID of the document currently being played, if any.
    pub document_id: Option<String>,
    /// Position anchor string for the current chunk (e.g. "section:0/chunk:3").
    pub anchor: Option<String>,
    /// Playback progress in engine-specific units.
    pub progress_units: usize,
    /// File path or URI of the audio being played.
    pub audio_reference: Option<String>,
    /// Name of the playback provider (e.g. "host", "fake").
    pub provider_name: Option<String>,
    /// OS process ID of the playback process, if applicable.
    pub process_id: Option<u32>,
}

/// Port for audio playback of synthesized speech.
pub trait PlaybackEngine {
    fn describe_capabilities(&self) -> ProviderCapabilities;
    fn hydrate(&mut self, snapshot: Option<PlaybackSnapshot>);
    fn start(
        &mut self,
        document: &Document,
        position: &ReadingPosition,
        synthesis: Option<SynthesisResult>,
    ) -> PlaybackSnapshot;
    fn pause(&mut self) -> PlaybackSnapshot;
    fn resume(&mut self) -> PlaybackSnapshot;
    fn stop(&mut self) -> PlaybackSnapshot;
    fn seek(&mut self, position: &ReadingPosition) -> PlaybackSnapshot;
    fn snapshot(&self) -> PlaybackSnapshot;
}

impl<T> PlaybackEngine for &mut T
where
    T: PlaybackEngine + ?Sized,
{
    fn describe_capabilities(&self) -> ProviderCapabilities {
        (**self).describe_capabilities()
    }

    fn hydrate(&mut self, snapshot: Option<PlaybackSnapshot>) {
        (**self).hydrate(snapshot);
    }

    fn start(
        &mut self,
        document: &Document,
        position: &ReadingPosition,
        synthesis: Option<SynthesisResult>,
    ) -> PlaybackSnapshot {
        (**self).start(document, position, synthesis)
    }

    fn pause(&mut self) -> PlaybackSnapshot {
        (**self).pause()
    }

    fn resume(&mut self) -> PlaybackSnapshot {
        (**self).resume()
    }

    fn stop(&mut self) -> PlaybackSnapshot {
        (**self).stop()
    }

    fn seek(&mut self, position: &ReadingPosition) -> PlaybackSnapshot {
        (**self).seek(position)
    }

    fn snapshot(&self) -> PlaybackSnapshot {
        (**self).snapshot()
    }
}
