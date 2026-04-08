#[cfg(feature = "alpha-compat")]
#[path = "backend_alpha.rs"]
mod backend_alpha;

use marginalia_playback_host::HostPlaybackEngine;
use marginalia_runtime::SqliteRuntime;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::VecDeque;
use std::env;
use std::path::{Path, PathBuf};
#[cfg(feature = "alpha-compat")]
use serde::Serialize;
#[cfg(feature = "alpha-compat")]
use backend_alpha::ProcessBackendClient;

#[derive(Debug, Clone)]
pub struct BackendLogEntry {
    pub sequence: u64,
    pub line: String,
}

#[derive(Debug, Deserialize)]
pub struct ResponseEnvelope {
    pub status: String,
    pub message: String,
    #[serde(default)]
    pub payload: Value,
    #[serde(default)]
    #[allow(dead_code)]
    pub request_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct IngestDocumentResult {
    pub message: String,
    pub document_id: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AppSnapshot {
    pub active_session_id: Option<String>,
    pub document_count: u32,
    pub latest_document_id: Option<String>,
    pub playback_state: Option<String>,
    pub runtime_status: Option<String>,
    pub state: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct SessionSnapshot {
    pub anchor: String,
    pub chunk_index: u32,
    pub chunk_text: String,
    pub command_listening_active: bool,
    pub command_stt_provider: Option<String>,
    pub document_id: String,
    pub notes_count: u32,
    pub playback_provider: Option<String>,
    pub playback_state: String,
    pub section_count: u32,
    pub section_index: u32,
    pub section_title: String,
    pub session_id: String,
    pub state: String,
    pub tts_provider: Option<String>,
    pub voice: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct DocumentListItem {
    pub chapter_count: u32,
    pub chunk_count: u32,
    pub document_id: String,
    pub title: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct DocumentChunkView {
    pub anchor: String,
    pub index: u32,
    pub is_active: bool,
    pub is_read: bool,
    pub text: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct DocumentSectionView {
    pub chunks: Vec<DocumentChunkView>,
    pub index: u32,
    pub title: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct DocumentView {
    pub active_chunk_index: Option<u32>,
    pub active_section_index: Option<u32>,
    pub chapter_count: u32,
    pub chunk_count: u32,
    pub document_id: String,
    pub sections: Vec<DocumentSectionView>,
    pub source_path: String,
    pub title: String,
}

#[cfg(feature = "alpha-compat")]
#[derive(Debug, Serialize)]
pub(crate) struct RequestEnvelope<'a> {
    #[serde(rename = "type")]
    request_type: &'a str,
    name: &'a str,
    payload: Value,
    id: String,
    protocol_version: u32,
}

pub(crate) enum BackendClient {
    #[cfg(feature = "alpha-compat")]
    Process(ProcessBackendClient),
    Beta(BetaBackendClient),
}

impl BackendClient {
    pub fn spawn(config_path: Option<&Path>) -> Result<Self, String> {
        #[cfg(not(feature = "alpha-compat"))]
        let _ = config_path;

        let mode = env::var("MARGINALIA_TUI_BACKEND")
            .ok()
            .map(|value| value.to_ascii_lowercase());

        match mode.as_deref() {
            None | Some("beta") | Some("rust") => BetaBackendClient::spawn().map(Self::Beta),
            #[cfg(feature = "alpha-compat")]
            Some("python") | Some("alpha") => {
                ProcessBackendClient::spawn(config_path).map(Self::Process)
            }
            #[cfg(not(feature = "alpha-compat"))]
            Some("python") | Some("alpha") => Err(
                "This TUI build does not include Alpha Python compatibility. Rebuild with --features alpha-compat."
                    .to_string(),
            ),
            Some(other) => Err(format!(
                "Unsupported MARGINALIA_TUI_BACKEND value: {other}. Expected beta, rust, python, or alpha."
            )),
        }
    }

    pub fn mode_label(&self) -> &'static str {
        match self {
            #[cfg(feature = "alpha-compat")]
            Self::Process(_) => "Alpha Python backend",
            Self::Beta(_) => "Beta Rust runtime",
        }
    }

    pub fn get_app_snapshot(&mut self) -> Result<AppSnapshot, String> {
        match self {
            #[cfg(feature = "alpha-compat")]
            Self::Process(client) => client.get_app_snapshot(),
            Self::Beta(client) => client.get_app_snapshot(),
        }
    }

    pub fn get_session_snapshot(&mut self) -> Result<Option<SessionSnapshot>, String> {
        match self {
            #[cfg(feature = "alpha-compat")]
            Self::Process(client) => client.get_session_snapshot(),
            Self::Beta(client) => client.get_session_snapshot(),
        }
    }

    pub fn get_doctor_report(&mut self) -> Result<Value, String> {
        match self {
            #[cfg(feature = "alpha-compat")]
            Self::Process(client) => client.get_doctor_report(),
            Self::Beta(client) => client.get_doctor_report(),
        }
    }

    pub fn list_documents(&mut self) -> Result<Vec<DocumentListItem>, String> {
        match self {
            #[cfg(feature = "alpha-compat")]
            Self::Process(client) => client.list_documents(),
            Self::Beta(client) => client.list_documents(),
        }
    }

    pub fn get_document_view(
        &mut self,
        document_id: Option<&str>,
    ) -> Result<Option<DocumentView>, String> {
        match self {
            #[cfg(feature = "alpha-compat")]
            Self::Process(client) => client.get_document_view(document_id),
            Self::Beta(client) => client.get_document_view(document_id),
        }
    }

    pub fn ingest_document(&mut self, path: &Path) -> Result<IngestDocumentResult, String> {
        let response = self.command_response(
            "ingest_document",
            json!({"path": path.display().to_string()}),
        )?;
        let document_id = response
            .payload
            .get("document")
            .and_then(|document| document.get("document_id"))
            .and_then(|document_id| document_id.as_str())
            .map(ToString::to_string);
        Ok(IngestDocumentResult {
            message: response.message,
            document_id,
        })
    }

    pub fn start_session(&mut self, target: &str) -> Result<String, String> {
        self.command_message("start_session", json!({"target": target}))
    }

    pub fn pause_session(&mut self) -> Result<String, String> {
        self.command_message("pause_session", json!({}))
    }

    pub fn resume_session(&mut self) -> Result<String, String> {
        self.command_message("resume_session", json!({}))
    }

    pub fn stop_session(&mut self) -> Result<String, String> {
        self.command_message("stop_session", json!({}))
    }

    pub fn repeat_chunk(&mut self) -> Result<String, String> {
        self.command_message("repeat_chunk", json!({}))
    }

    pub fn restart_chapter(&mut self) -> Result<String, String> {
        self.command_message("restart_chapter", json!({}))
    }

    pub fn previous_chunk(&mut self) -> Result<String, String> {
        self.command_message("previous_chunk", json!({}))
    }

    pub fn next_chunk(&mut self) -> Result<String, String> {
        self.command_message("next_chunk", json!({}))
    }

    pub fn previous_chapter(&mut self) -> Result<String, String> {
        self.command_message("previous_chapter", json!({}))
    }

    pub fn next_chapter(&mut self) -> Result<String, String> {
        self.command_message("next_chapter", json!({}))
    }

    pub fn create_note(&mut self, text: &str) -> Result<String, String> {
        self.command_message("create_note", json!({"text": text}))
    }

    fn command_message(&mut self, name: &str, payload: Value) -> Result<String, String> {
        let response = self.command_response(name, payload)?;
        if response.status == "ok" {
            Ok(response.message)
        } else {
            Err(response.message)
        }
    }

    fn command_response(
        &mut self,
        name: &str,
        payload: Value,
    ) -> Result<ResponseEnvelope, String> {
        match self {
            #[cfg(feature = "alpha-compat")]
            Self::Process(client) => client.execute_command_response(name, payload),
            Self::Beta(client) => client.execute_command_response(name, payload),
        }
    }

    pub fn recent_stderr_entries(&self, after_sequence: u64) -> Vec<BackendLogEntry> {
        match self {
            #[cfg(feature = "alpha-compat")]
            Self::Process(client) => client.recent_stderr_entries(after_sequence),
            Self::Beta(client) => client.recent_stderr_entries(after_sequence),
        }
    }
}

pub(crate) struct BetaBackendClient {
    runtime: SqliteRuntime,
    logs: VecDeque<BackendLogEntry>,
    sequence: u64,
}

impl BetaBackendClient {
    fn spawn() -> Result<Self, String> {
        let repo_root = env::var("MARGINALIA_REPO_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|_| env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
        let db_path = env::var("MARGINALIA_TUI_BETA_DB")
            .map(PathBuf::from)
            .unwrap_or_else(|_| repo_root.join(".marginalia-beta.sqlite3"));
        let mut runtime = SqliteRuntime::open(&db_path)
            .map_err(|err| format!("Unable to open beta runtime database: {err}"))?;
        if env::var("MARGINALIA_TUI_PLAYBACK")
            .ok()
            .is_some_and(|value| value.eq_ignore_ascii_case("host"))
        {
            runtime.set_playback_engine(HostPlaybackEngine::default());
        }

        let mut client = Self {
            runtime,
            logs: VecDeque::with_capacity(256),
            sequence: 0,
        };
        client.push_log(format!(
            "beta-runtime ready db={} playback={}",
            db_path.display(),
            env::var("MARGINALIA_TUI_PLAYBACK").unwrap_or_else(|_| "fake".to_string())
        ));
        Ok(client)
    }

    fn get_app_snapshot(&mut self) -> Result<AppSnapshot, String> {
        let response = self.send_request("query", "get_app_snapshot", json!({}));
        decode_payload(response.payload, "app")
    }

    fn get_session_snapshot(&mut self) -> Result<Option<SessionSnapshot>, String> {
        let response = self.send_request("query", "get_session_snapshot", json!({}));
        match response.payload.get("session") {
            Some(Value::Null) | None => Ok(None),
            Some(_) => decode_payload(response.payload, "session").map(Some),
        }
    }

    fn get_doctor_report(&mut self) -> Result<Value, String> {
        Ok(self.send_request("query", "get_doctor_report", json!({})).payload)
    }

    fn list_documents(&mut self) -> Result<Vec<DocumentListItem>, String> {
        let response = self.send_request("query", "list_documents", json!({}));
        let documents = response
            .payload
            .get("documents")
            .cloned()
            .ok_or_else(|| "Beta runtime omitted documents list.".to_string())?;
        serde_json::from_value(documents)
            .map_err(|err| format!("Unable to decode documents payload: {err}"))
    }

    fn get_document_view(
        &mut self,
        document_id: Option<&str>,
    ) -> Result<Option<DocumentView>, String> {
        let payload = match document_id {
            Some(document_id) => json!({ "document_id": document_id }),
            None => json!({}),
        };
        let response = self.send_request("query", "get_document_view", payload);
        match response.payload.get("document") {
            Some(Value::Null) | None => Ok(None),
            Some(_) => decode_payload(response.payload, "document").map(Some),
        }
    }

    fn execute_command_response(
        &mut self,
        name: &str,
        payload: Value,
    ) -> Result<ResponseEnvelope, String> {
        Ok(self.send_request("command", name, payload))
    }

    fn recent_stderr_entries(&self, after_sequence: u64) -> Vec<BackendLogEntry> {
        self.logs
            .iter()
            .filter(|entry| entry.sequence > after_sequence)
            .cloned()
            .collect()
    }

    fn push_log(&mut self, line: String) {
        self.sequence += 1;
        if self.logs.len() >= 200 {
            self.logs.pop_front();
        }
        self.logs.push_back(BackendLogEntry {
            sequence: self.sequence,
            line,
        });
    }
    fn send_request(&mut self, request_type: &str, name: &str, payload: Value) -> ResponseEnvelope {
        let response = match request_type {
            "query" => self.runtime.execute_frontend_query(name, payload),
            "command" => self.runtime.execute_frontend_command(name, payload),
            other => marginalia_runtime::RuntimeFrontendResponse {
                status: "error".to_string(),
                message: format!("Unsupported beta request type: {other}"),
                payload: json!({}),
            },
        };

        self.push_log(format!("beta {request_type} {name} => {}", response.status));
        ResponseEnvelope {
            status: response.status,
            message: response.message,
            payload: response.payload,
            request_id: None,
        }
    }
}

fn decode_payload<T>(payload: Value, field: &str) -> Result<T, String>
where
    T: for<'de> Deserialize<'de>,
{
    let value = payload
        .get(field)
        .cloned()
        .ok_or_else(|| format!("Backend omitted '{field}' payload."))?;
    serde_json::from_value(value)
        .map_err(|err| format!("Unable to decode '{field}' payload: {err}"))
}
