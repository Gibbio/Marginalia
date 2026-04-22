// Re-export shared config types so the rest of the TUI can use them
// via `crate::config::TuiConfig` etc., unchanged.
pub use marginalia_config::{AppConfig as TuiConfig, VoiceCommandsSection};
use std::path::PathBuf;

/// TUI default config path.
pub fn default_path() -> PathBuf {
    PathBuf::from("apps/tui-rs/marginalia.toml")
}

/// Convenience: TUI's `load()` — mirrors the previous behavior (forgiving,
/// falls back to `Default` on missing / malformed files).
pub fn load() -> TuiConfig {
    TuiConfig::load_or_default(&default_path())
}
