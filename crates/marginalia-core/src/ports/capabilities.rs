#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderExecutionMode {
    Local,
    Hybrid,
    Remote,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderCapabilities {
    pub provider_name: String,
    pub interface_kind: String,
    pub supported_languages: Vec<String>,
    pub supports_streaming: bool,
    pub supports_partial_results: bool,
    pub supports_timestamps: bool,
    pub low_latency_suitable: bool,
    pub offline_capable: bool,
    pub execution_mode: ProviderExecutionMode,
}

impl Default for ProviderCapabilities {
    fn default() -> Self {
        Self {
            provider_name: String::new(),
            interface_kind: String::new(),
            supported_languages: vec!["en".to_string()],
            supports_streaming: false,
            supports_partial_results: false,
            supports_timestamps: false,
            low_latency_suitable: false,
            offline_capable: true,
            execution_mode: ProviderExecutionMode::Local,
        }
    }
}
