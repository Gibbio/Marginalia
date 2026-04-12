#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrontendCommandName {
    CreateNote,
    IngestDocument,
    NextChunk,
    NextChapter,
    PauseSession,
    PreviousChapter,
    PreviousChunk,
    RepeatChunk,
    RestartChapter,
    ResumeSession,
    StartSession,
    StopSession,
}
