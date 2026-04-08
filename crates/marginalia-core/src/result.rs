use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationStatus {
    Ok,
    Planned,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperationResult {
    pub status: OperationStatus,
    pub message: String,
    pub data: HashMap<String, String>,
}

impl OperationResult {
    pub fn ok(message: impl Into<String>, data: Option<HashMap<String, String>>) -> Self {
        Self {
            status: OperationStatus::Ok,
            message: message.into(),
            data: data.unwrap_or_default(),
        }
    }

    pub fn planned(message: impl Into<String>, data: Option<HashMap<String, String>>) -> Self {
        Self {
            status: OperationStatus::Planned,
            message: message.into(),
            data: data.unwrap_or_default(),
        }
    }

    pub fn error(message: impl Into<String>, data: Option<HashMap<String, String>>) -> Self {
        Self {
            status: OperationStatus::Error,
            message: message.into(),
            data: data.unwrap_or_default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{OperationResult, OperationStatus};

    #[test]
    fn operation_result_ok_uses_expected_status() {
        let result = OperationResult::ok("fine", None);

        assert_eq!(result.status, OperationStatus::Ok);
        assert_eq!(result.message, "fine");
    }
}
