use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::VecDeque;
use std::env;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

const PROTOCOL_VERSION: u32 = 1;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

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
    pub request_id: Option<String>,
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
    stdin: Option<BufWriter<ChildStdin>>,
    response_rx: mpsc::Receiver<String>,
    reader_thread: Option<JoinHandle<()>>,
    request_counter: u64,
    stderr_lines: Arc<Mutex<VecDeque<BackendLogEntry>>>,
    stderr_thread: Option<JoinHandle<()>>,
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
            .stderr(Stdio::piped());

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
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| "Backend stderr pipe unavailable.".to_string())?;
        let stderr_lines = Arc::new(Mutex::new(VecDeque::with_capacity(256)));
        let stderr_thread = Some(spawn_stderr_collector(stderr, Arc::clone(&stderr_lines)));
        let (response_tx, response_rx) = mpsc::channel();
        let reader_thread = Some(spawn_stdout_reader(stdout, response_tx));

        Ok(Self {
            child,
            stdin: Some(BufWriter::new(stdin)),
            response_rx,
            reader_thread,
            request_counter: 0,
            stderr_lines,
            stderr_thread,
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

    pub fn get_doctor_report(&mut self) -> Result<Value, String> {
        let response = self.send_request("query", "get_doctor_report", json!({}))?;
        Ok(response.payload)
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

    pub fn get_document_view(
        &mut self,
        document_id: Option<&str>,
    ) -> Result<Option<DocumentView>, String> {
        let payload = match document_id {
            Some(document_id) => json!({"document_id": document_id}),
            None => json!({}),
        };
        let response = self.send_request("query", "get_document_view", payload)?;
        if response.status != "ok" {
            return Ok(None);
        }
        match response.payload.get("document") {
            Some(Value::Null) | None => Ok(None),
            Some(_) => decode_payload(response.payload, "document").map(Some),
        }
    }

    pub fn execute_command(&mut self, name: &str, payload: Value) -> Result<String, String> {
        let response = self.execute_command_response(name, payload)?;
        if response.status == "ok" {
            Ok(response.message)
        } else {
            Err(response.message)
        }
    }

    pub fn execute_command_response(
        &mut self,
        name: &str,
        payload: Value,
    ) -> Result<ResponseEnvelope, String> {
        let response = self.send_request("command", name, payload)?;
        Ok(response)
    }

    pub fn recent_stderr_entries(&self, after_sequence: u64) -> Vec<BackendLogEntry> {
        let Ok(lines) = self.stderr_lines.lock() else {
            return Vec::new();
        };
        lines
            .iter()
            .filter(|entry| entry.sequence > after_sequence)
            .cloned()
            .collect()
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
        let write_err = {
            let stdin = self
                .stdin
                .as_mut()
                .ok_or_else(|| "Backend stdin already closed.".to_string())?;
            stdin
                .write_all(encoded.as_bytes())
                .and_then(|_| stdin.write_all(b"\n"))
                .and_then(|_| stdin.flush())
                .err()
        };
        if let Some(err) = write_err {
            return Err(format!(
                "Unable to send backend request: {err}{}",
                self.backend_exit_hint()
            ));
        }

        let line = self
            .response_rx
            .recv_timeout(REQUEST_TIMEOUT)
            .map_err(|err| match err {
                mpsc::RecvTimeoutError::Timeout => {
                    format!(
                        "Backend did not respond within {} seconds.{}",
                        REQUEST_TIMEOUT.as_secs(),
                        self.backend_exit_hint()
                    )
                }
                mpsc::RecvTimeoutError::Disconnected => {
                    format!(
                        "Backend closed the stdio channel unexpectedly.{}",
                        self.backend_exit_hint()
                    )
                }
            })?;
        let response: ResponseEnvelope = serde_json::from_str(&line)
            .map_err(|err| format!("Unable to decode backend response: {err}"))?;
        if let Some(ref resp_id) = response.request_id {
            let expected_id = format!("req-{}", self.request_counter);
            if resp_id != &expected_id {
                return Err(format!(
                    "Response id mismatch: expected {expected_id}, got {resp_id}"
                ));
            }
        }
        Ok(response)
    }

    fn backend_exit_hint(&mut self) -> String {
        let exit_status = self
            .child
            .try_wait()
            .ok()
            .flatten()
            .map(|status| status.to_string());
        let stderr = self.recent_stderr();
        match (exit_status, stderr.is_empty()) {
            (Some(status), false) => format!(". Backend exited with {status}. stderr: {stderr}"),
            (Some(status), true) => format!(". Backend exited with {status}."),
            (None, false) => format!(". Recent backend stderr: {stderr}"),
            (None, true) => String::new(),
        }
    }

    fn recent_stderr(&self) -> String {
        let Ok(lines) = self.stderr_lines.lock() else {
            return String::new();
        };
        lines
            .iter()
            .map(|entry| entry.line.clone())
            .collect::<Vec<_>>()
            .join(" | ")
    }
}

impl Drop for BackendClient {
    fn drop(&mut self) {
        // Close stdin so the backend's serve_forever() loop exits and the
        // finally block calls gateway.shutdown() for a clean provider stop.
        self.stdin.take();

        // Give the backend a moment to shut down gracefully before killing.
        std::thread::sleep(Duration::from_millis(500));
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
        }
        let _ = self.child.wait();
        if let Some(handle) = self.reader_thread.take() {
            let _ = handle.join();
        }
        if let Some(handle) = self.stderr_thread.take() {
            let _ = handle.join();
        }
    }
}

fn spawn_stdout_reader(stdout: ChildStdout, tx: mpsc::Sender<String>) -> JoinHandle<()> {
    thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.lines().map_while(Result::ok) {
            let trimmed = line.trim().to_string();
            if trimmed.is_empty() {
                continue;
            }
            if tx.send(trimmed).is_err() {
                break;
            }
        }
    })
}

fn spawn_stderr_collector(
    stderr: ChildStderr,
    stderr_lines: Arc<Mutex<VecDeque<BackendLogEntry>>>,
) -> JoinHandle<()> {
    thread::spawn(move || {
        let reader = BufReader::new(stderr);
        let mut sequence = 0_u64;
        for line in reader.lines().map_while(Result::ok) {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            sequence += 1;
            if let Ok(mut lines) = stderr_lines.lock() {
                if lines.len() >= 200 {
                    lines.pop_front();
                }
                lines.push_back(BackendLogEntry {
                    sequence,
                    line: trimmed.to_string(),
                });
            }
        }
    })
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
