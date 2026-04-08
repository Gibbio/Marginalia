#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackendCapabilities {
    pub protocol_version: u32,
    pub commands: Vec<String>,
    pub queries: Vec<String>,
    pub transports: Vec<String>,
    pub frontend_event_stream_supported: bool,
    pub dictation_enabled: bool,
    pub rewrite_enabled: bool,
    pub summary_enabled: bool,
}
