use crate::domain::{PlaybackState, ReaderState, ReadingSession};
use std::error::Error;
use std::fmt::{Display, Formatter};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidTransitionError {
    pub from: ReaderState,
    pub to: ReaderState,
}

impl Display for InvalidTransitionError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Cannot move from {} to {}.",
            self.from.as_str(),
            self.to.as_str()
        )
    }
}

impl Error for InvalidTransitionError {}

pub fn playback_state_for(reader_state: ReaderState) -> PlaybackState {
    match reader_state {
        ReaderState::Reading | ReaderState::ReadingRewrite => PlaybackState::Playing,
        ReaderState::Idle => PlaybackState::Stopped,
        _ => PlaybackState::Paused,
    }
}

pub struct ReaderStateMachine;

impl ReaderStateMachine {
    pub fn transition(
        &self,
        session: &mut ReadingSession,
        target: ReaderState,
    ) -> Result<(), InvalidTransitionError> {
        if session.state == target {
            return Ok(());
        }

        if !is_transition_allowed(session.state, target) {
            return Err(InvalidTransitionError {
                from: session.state,
                to: target,
            });
        }

        session.state = target;
        session.playback_state = playback_state_for(target);
        session.touch();
        Ok(())
    }
}

fn is_transition_allowed(from: ReaderState, to: ReaderState) -> bool {
    match from {
        ReaderState::Idle => matches!(
            to,
            ReaderState::Reading | ReaderState::ListeningForCommand | ReaderState::Error
        ),
        ReaderState::Reading => matches!(
            to,
            ReaderState::Idle
                | ReaderState::Paused
                | ReaderState::ListeningForCommand
                | ReaderState::RecordingNote
                | ReaderState::ProcessingRewrite
                | ReaderState::Error
        ),
        ReaderState::Paused => matches!(
            to,
            ReaderState::Idle
                | ReaderState::Reading
                | ReaderState::ListeningForCommand
                | ReaderState::RecordingNote
                | ReaderState::ProcessingRewrite
                | ReaderState::Error
        ),
        ReaderState::ListeningForCommand => matches!(
            to,
            ReaderState::Idle
                | ReaderState::Reading
                | ReaderState::Paused
                | ReaderState::RecordingNote
                | ReaderState::Error
        ),
        ReaderState::RecordingNote => {
            matches!(
                to,
                ReaderState::Paused | ReaderState::Reading | ReaderState::Error
            )
        }
        ReaderState::ProcessingRewrite => matches!(
            to,
            ReaderState::ReadingRewrite | ReaderState::Paused | ReaderState::Error
        ),
        ReaderState::ReadingRewrite => {
            matches!(
                to,
                ReaderState::Paused | ReaderState::Reading | ReaderState::Error
            )
        }
        ReaderState::Error => matches!(to, ReaderState::Idle),
    }
}

#[cfg(test)]
mod tests {
    use super::{playback_state_for, InvalidTransitionError, ReaderStateMachine};
    use crate::domain::{PlaybackState, ReaderState, ReadingSession};

    #[test]
    fn playback_state_projection_matches_reader_state() {
        assert_eq!(
            playback_state_for(ReaderState::Idle),
            PlaybackState::Stopped
        );
        assert_eq!(
            playback_state_for(ReaderState::Reading),
            PlaybackState::Playing
        );
        assert_eq!(
            playback_state_for(ReaderState::Paused),
            PlaybackState::Paused
        );
    }

    #[test]
    fn valid_transition_updates_session_state() {
        let machine = ReaderStateMachine;
        let mut session = ReadingSession::new("session-1", "doc-1");

        machine
            .transition(&mut session, ReaderState::Reading)
            .unwrap();

        assert_eq!(session.state, ReaderState::Reading);
        assert_eq!(session.playback_state, PlaybackState::Playing);
    }

    #[test]
    fn invalid_transition_returns_error() {
        let machine = ReaderStateMachine;
        let mut session = ReadingSession::new("session-1", "doc-1");
        session.state = ReaderState::Idle;

        let error = machine
            .transition(&mut session, ReaderState::RecordingNote)
            .unwrap_err();

        assert_eq!(
            error,
            InvalidTransitionError {
                from: ReaderState::Idle,
                to: ReaderState::RecordingNote,
            }
        );
    }
}
