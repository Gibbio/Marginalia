use std::env;
use std::fs::{create_dir_all, File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

impl log::Log for AppLogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= log::Level::Info
    }

    fn log(&self, record: &log::Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let level = match record.level() {
            log::Level::Error => "ERROR",
            log::Level::Warn => "WARN",
            log::Level::Info => "INFO",
            log::Level::Debug => "DEBUG",
            log::Level::Trace => "TRACE",
        };
        self.write(level, &record.args().to_string());
    }

    fn flush(&self) {}
}

#[derive(Clone)]
pub struct AppLogger {
    file: Arc<Mutex<File>>,
    path: PathBuf,
}

impl AppLogger {
    pub fn from_env() -> Result<Self, String> {
        let path = env::var("MARGINALIA_TUI_LOG_FILE")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("marginalia-tui.log"));

        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            create_dir_all(parent).map_err(|err| {
                format!(
                    "Unable to create TUI log directory '{}': {err}",
                    parent.display()
                )
            })?;
        }

        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|err| format!("Unable to open TUI log file '{}': {err}", path.display()))?;

        Ok(Self {
            file: Arc::new(Mutex::new(file)),
            path,
        })
    }

    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    pub fn info(&self, message: impl AsRef<str>) {
        self.write("INFO", message.as_ref());
    }

    pub fn warn(&self, message: impl AsRef<str>) {
        self.write("WARN", message.as_ref());
    }

    pub fn error(&self, message: impl AsRef<str>) {
        self.write("ERROR", message.as_ref());
    }

    fn write(&self, level: &str, message: &str) {
        let Ok(mut file) = self.file.lock() else {
            return;
        };
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_secs())
            .unwrap_or(0);
        let _ = writeln!(file, "[{timestamp}] {level} {message}");
        let _ = file.flush();
    }
}
