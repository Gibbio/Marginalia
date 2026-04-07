use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::env;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

const PROTOCOL_VERSION: u32 = 1;

#[derive(Debug, Deserialize)]
pub struct ResponseEnvelope {
    pub status: String,
    pub message: String,
    #[serde(default)]
    pub payload: Value,
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
    pub chunk_count: u32,
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

#[derive(Debug, Serialize)]
struct RequestEnvelope<'a> {
    #[serde(rename = "type")]
    request_type: &'a str,
    name: &'a str,
    payload: Value,
    id: String,
    protocol_version: u32,
}

pub struct BackendClient {
    child: Child,
    stdin: BufWriter<ChildStdin>,
    stdout: BufReader<ChildStdout>,
    request_counter: u64,
}

impl BackendClient {
    pub fn spawn(config_path: Option<&Path>) -> Result<Self, String> {
        let repo_root = env::var("MARGINALIA_REPO_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|_| env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
        let python = env::var("MARGINALIA_BACKEND_PYTHON")
            .unwrap_or_else(|_| repo_root.join(".venv/bin/python").display().to_string());
        let python_path = build_python_path(&repo_root);

        let mut command = Command::new(python);
        command
            .arg("-m")
            .arg("marginalia_backend")
            .arg("serve-stdio");
        if let Some(config) = config_path {
            command.arg("--config").arg(config);
        }
        command
            .env("PYTHONPATH", python_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());

        let mut child = command
            .spawn()
            .map_err(|err| format!("Unable to spawn backend process: {err}"))?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| "Backend stdin pipe unavailable.".to_string())?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "Backend stdout pipe unavailable.".to_string())?;

        Ok(Self {
            child,
            stdin: BufWriter::new(stdin),
            stdout: BufReader::new(stdout),
            request_counter: 0,
        })
    }

    pub fn get_app_snapshot(&mut self) -> Result<AppSnapshot, String> {
        let response = self.send_request("query", "get_app_snapshot", json!({}))?;
        decode_payload(response.payload, "app")
    }

    pub fn get_session_snapshot(&mut self) -> Result<Option<SessionSnapshot>, String> {
        let response = self.send_request("query", "get_session_snapshot", json!({}))?;
        match response.payload.get("session") {
            Some(Value::Null) | None => Ok(None),
            Some(_) => decode_payload(response.payload, "session").map(Some),
        }
    }

    pub fn list_documents(&mut self) -> Result<Vec<DocumentListItem>, String> {
        let response = self.send_request("query", "list_documents", json!({}))?;
        let documents = response
            .payload
            .get("documents")
            .cloned()
            .ok_or_else(|| "Backend omitted documents list.".to_string())?;
        serde_json::from_value(documents)
            .map_err(|err| format!("Unable to decode documents payload: {err}"))
    }

    pub fn get_document_view(&mut self) -> Result<Option<DocumentView>, String> {
        let response = self.send_request("query", "get_document_view", json!({}))?;
        if response.status != "ok" {
            return Ok(None);
        }
        match response.payload.get("document") {
            Some(Value::Null) | None => Ok(None),
            Some(_) => decode_payload(response.payload, "document").map(Some),
        }
    }

    pub fn execute_command(&mut self, name: &str, payload: Value) -> Result<String, String> {
        let response = self.send_request("command", name, payload)?;
        if response.status == "ok" {
            Ok(response.message)
        } else {
            Err(response.message)
        }
    }

    fn send_request(
        &mut self,
        request_type: &str,
        name: &str,
        payload: Value,
    ) -> Result<ResponseEnvelope, String> {
        self.request_counter += 1;
        let request = RequestEnvelope {
            request_type,
            name,
            payload,
            id: format!("req-{}", self.request_counter),
            protocol_version: PROTOCOL_VERSION,
        };
        let encoded = serde_json::to_string(&request)
            .map_err(|err| format!("Unable to encode backend request: {err}"))?;
        self.stdin
            .write_all(encoded.as_bytes())
            .map_err(|err| format!("Unable to write backend request: {err}"))?;
        self.stdin
            .write_all(b"\n")
            .map_err(|err| format!("Unable to terminate backend request: {err}"))?;
        self.stdin
            .flush()
            .map_err(|err| format!("Unable to flush backend request: {err}"))?;

        let mut line = String::new();
        self.stdout
            .read_line(&mut line)
            .map_err(|err| format!("Unable to read backend response: {err}"))?;
        if line.trim().is_empty() {
            return Err("Backend closed the stdio channel unexpectedly.".to_string());
        }
        serde_json::from_str(&line)
            .map_err(|err| format!("Unable to decode backend response: {err}"))
    }
}

impl Drop for BackendClient {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn build_python_path(repo_root: &Path) -> String {
    let entries = [
        repo_root.join("apps/backend/src"),
        repo_root.join("apps/cli/src"),
        repo_root.join("packages/core/src"),
        repo_root.join("packages/adapters/src"),
        repo_root.join("packages/infra/src"),
    ];

    let mut rendered: Vec<String> = entries
        .iter()
        .map(|path| path.display().to_string())
        .collect();
    if let Ok(existing) = env::var("PYTHONPATH") {
        if !existing.trim().is_empty() {
            rendered.push(existing);
        }
    }
    rendered.join(":")
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
