use chrono::{DateTime, Utc};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeSessionRecord {
    pub process_id: u32,
    pub session_id: String,
    pub document_id: String,
    pub command_language: String,
    pub started_at: DateTime<Utc>,
    pub entrypoint: String,
    pub working_directory: Option<PathBuf>,
    pub process_start_time: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeCleanupReport {
    pub runtime_found: bool,
    pub record_removed: bool,
    pub terminated_process_ids: Vec<u32>,
    pub notes: Vec<String>,
}

impl RuntimeCleanupReport {
    pub fn cleaned_up(&self) -> bool {
        self.record_removed || !self.terminated_process_ids.is_empty()
    }
}

pub trait RuntimeSupervisor {
    fn activate(&mut self, record: RuntimeSessionRecord);
    fn current_runtime(&self) -> Option<RuntimeSessionRecord>;
    fn cleanup_existing_runtime(&mut self, current_process_id: u32) -> RuntimeCleanupReport;
    fn clear(&mut self, process_id: Option<u32>);
}

#[cfg(test)]
mod tests {
    use super::RuntimeCleanupReport;

    #[test]
    fn cleaned_up_is_true_for_removed_record() {
        let report = RuntimeCleanupReport {
            runtime_found: true,
            record_removed: true,
            terminated_process_ids: Vec::new(),
            notes: Vec::new(),
        };

        assert!(report.cleaned_up());
    }
}
