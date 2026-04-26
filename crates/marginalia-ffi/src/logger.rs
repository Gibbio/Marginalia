//! File-backed `log` backend for the FFI consumers (mac-gui, future Kotlin).
//!
//! `marginalia-ffi` depends on `log = "0.4"` and the runtime / playback /
//! AEC crates emit `log::info!` / `log::warn!` / `log::error!` throughout —
//! but without a registered backend every macro call is a no-op. The TUI
//! installs `tui-rs/src/logger.rs::AppLogger` from `main`; the `.app` has
//! no equivalent entry point (`FfiRuntime::new` is the closest thing), so
//! the diagnostic lines added in `playback-host` for the silent-cross-thread
//! investigation never reached disk on macOS. This module fills that gap.
//!
//! Output goes to `<config_dir>/marginalia-ffi.log` next to `marginalia.toml`,
//! which inside a sandboxed `.app` resolves to
//! `~/Library/Containers/<bundle-id>/Data/Library/Application Support/Marginalia/`.
//! Override with `MARGINALIA_FFI_LOG_FILE`. Level defaults to `info`;
//! override with `MARGINALIA_FFI_LOG_LEVEL` (off|error|warn|info|debug|trace).

use std::fs::{create_dir_all, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

struct FileLogger {
    file: Mutex<File>,
    level: log::LevelFilter,
}

impl log::Log for FileLogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= self.level
    }

    fn log(&self, record: &log::Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let Ok(mut file) = self.file.lock() else {
            return;
        };
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let thread = std::thread::current();
        let thread_name = thread.name().unwrap_or("?");
        let _ = writeln!(
            file,
            "[{ts}] {:<5} {} ({}) {}",
            record.level(),
            record.target(),
            thread_name,
            record.args(),
        );
        let _ = file.flush();
    }

    fn flush(&self) {
        if let Ok(mut f) = self.file.lock() {
            let _ = f.flush();
        }
    }
}

static INIT: OnceLock<PathBuf> = OnceLock::new();

/// Install a file-backed `log` backend at `<dir>/marginalia-ffi.log` (or the
/// path in `MARGINALIA_FFI_LOG_FILE`). Idempotent: subsequent calls return
/// the path that won the first race; the file from later calls is dropped.
/// Returns the resolved log path on success.
pub fn init_in_dir(dir: &Path) -> Result<PathBuf, String> {
    if let Some(p) = INIT.get() {
        return Ok(p.clone());
    }

    let path = std::env::var("MARGINALIA_FFI_LOG_FILE")
        .map(PathBuf::from)
        .unwrap_or_else(|_| dir.join("marginalia-ffi.log"));

    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            create_dir_all(parent)
                .map_err(|e| format!("create log dir {}: {e}", parent.display()))?;
        }
    }

    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|e| format!("open log file {}: {e}", path.display()))?;

    let level = std::env::var("MARGINALIA_FFI_LOG_LEVEL")
        .ok()
        .as_deref()
        .and_then(|s| s.parse().ok())
        .unwrap_or(log::LevelFilter::Info);

    let logger = FileLogger {
        file: Mutex::new(file),
        level,
    };
    if log::set_boxed_logger(Box::new(logger)).is_ok() {
        log::set_max_level(level);
    }
    let _ = INIT.set(path.clone());
    Ok(path)
}
