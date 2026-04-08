use crate::domain::{Document, PlaybackState, ReadingPosition};
use crate::ports::{capabilities::ProviderCapabilities, tts::SynthesisResult};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaybackSnapshot {
    pub state: PlaybackState,
    pub last_action: String,
    pub document_id: Option<String>,
    pub anchor: Option<String>,
    pub progress_units: usize,
    pub audio_reference: Option<String>,
    pub provider_name: Option<String>,
    pub process_id: Option<u32>,
}

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
