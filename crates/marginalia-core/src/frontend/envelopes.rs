use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

pub const FRONTEND_PROTOCOL_VERSION: u32 = 1;

static REQUEST_COUNTER: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrontendResponseStatus {
    Error,
    Ok,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrontendRequest {
    pub request_type: String,
    pub name: String,
    pub payload: HashMap<String, String>,
    pub request_id: String,
    pub protocol_version: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrontendRequestParseError {
    PayloadMustBeAnObject,
    RequestTypeRequired,
    RequestNameRequired,
    ProtocolVersionMustBeInteger,
}

impl FrontendRequest {
    pub fn new(request_type: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            request_type: request_type.into(),
            name: name.into(),
            payload: HashMap::new(),
            request_id: format!("req-{}", REQUEST_COUNTER.fetch_add(1, Ordering::Relaxed)),
            protocol_version: FRONTEND_PROTOCOL_VERSION,
        }
    }

    pub fn with_payload(mut self, payload: HashMap<String, String>) -> Self {
        self.payload = payload;
        self
    }

    pub fn from_raw_parts(
        request_type: impl Into<String>,
        name: impl Into<String>,
        payload: Option<HashMap<String, String>>,
        request_id: Option<String>,
        protocol_version: Option<u32>,
    ) -> Result<Self, FrontendRequestParseError> {
        let request_type = request_type.into().trim().to_string();
        let name = name.into().trim().to_string();
        if request_type.is_empty() {
            return Err(FrontendRequestParseError::RequestTypeRequired);
        }
        if name.is_empty() {
            return Err(FrontendRequestParseError::RequestNameRequired);
        }
        Ok(Self {
            request_type,
            name,
            payload: payload.unwrap_or_default(),
            request_id: request_id.unwrap_or_else(|| {
                format!("req-{}", REQUEST_COUNTER.fetch_add(1, Ordering::Relaxed))
            }),
            protocol_version: protocol_version.unwrap_or(FRONTEND_PROTOCOL_VERSION),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrontendResponse {
    pub status: FrontendResponseStatus,
    pub name: String,
    pub message: String,
    pub payload: HashMap<String, String>,
    pub request_id: Option<String>,
    pub protocol_version: u32,
}

#[cfg(test)]
mod tests {
    use super::{FrontendRequest, FrontendRequestParseError, FRONTEND_PROTOCOL_VERSION};
    use std::collections::HashMap;

    #[test]
    fn frontend_request_builder_uses_protocol_default() {
        let request = FrontendRequest::new("query", "get_app_snapshot");

        assert_eq!(request.protocol_version, FRONTEND_PROTOCOL_VERSION);
        assert_eq!(request.request_type, "query");
        assert_eq!(request.name, "get_app_snapshot");
    }

    #[test]
    fn frontend_request_from_raw_parts_validates_required_fields() {
        let err = FrontendRequest::from_raw_parts("", "name", None, None, None);
        assert_eq!(err, Err(FrontendRequestParseError::RequestTypeRequired));

        let mut payload = HashMap::new();
        payload.insert("document_id".to_string(), "doc-1".to_string());
        let ok = FrontendRequest::from_raw_parts(
            "command",
            "start_session",
            Some(payload.clone()),
            Some("req-9".to_string()),
            Some(1),
        )
        .unwrap();

        assert_eq!(ok.payload, payload);
        assert_eq!(ok.request_id, "req-9");
    }
}
