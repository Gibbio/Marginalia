use marginalia_runtime::SqliteRuntime;
use serde::Deserialize;
#[cfg(feature = "alpha-compat")]
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::VecDeque;
use std::env;
#[cfg(feature = "alpha-compat")]
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
#[cfg(feature = "alpha-compat")]
use std::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command, Stdio};
#[cfg(feature = "alpha-compat")]
use std::sync::mpsc;
#[cfg(feature = "alpha-compat")]
use std::sync::{Arc, Mutex};
#[cfg(feature = "alpha-compat")]
use std::thread::{self, JoinHandle};
#[cfg(feature = "alpha-compat")]
use std::time::Duration;

#[cfg(feature = "alpha-compat")]
const PROTOCOL_VERSION: u32 = 1;
#[cfg(feature = "alpha-compat")]
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
    #[allow(dead_code)]
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

#[cfg(feature = "alpha-compat")]
#[derive(Debug, Serialize)]
struct RequestEnvelope<'a> {
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

#[cfg(feature = "alpha-compat")]
pub(crate) struct ProcessBackendClient {
    child: Child,
    stdin: Option<BufWriter<ChildStdin>>,
    response_rx: mpsc::Receiver<String>,
    reader_thread: Option<JoinHandle<()>>,
    request_counter: u64,
    stderr_lines: Arc<Mutex<VecDeque<BackendLogEntry>>>,
    stderr_thread: Option<JoinHandle<()>>,
}

#[cfg(feature = "alpha-compat")]
impl ProcessBackendClient {
    fn spawn(config_path: Option<&Path>) -> Result<Self, String> {
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

    fn get_app_snapshot(&mut self) -> Result<AppSnapshot, String> {
        let response = self.send_request("query", "get_app_snapshot", json!({}))?;
        decode_payload(response.payload, "app")
    }

    fn get_session_snapshot(&mut self) -> Result<Option<SessionSnapshot>, String> {
        let response = self.send_request("query", "get_session_snapshot", json!({}))?;
        match response.payload.get("session") {
            Some(Value::Null) | None => Ok(None),
            Some(_) => decode_payload(response.payload, "session").map(Some),
        }
    }

    fn get_doctor_report(&mut self) -> Result<Value, String> {
        let response = self.send_request("query", "get_doctor_report", json!({}))?;
        Ok(response.payload)
    }

    fn list_documents(&mut self) -> Result<Vec<DocumentListItem>, String> {
        let response = self.send_request("query", "list_documents", json!({}))?;
        let documents = response
            .payload
            .get("documents")
            .cloned()
            .ok_or_else(|| "Backend omitted documents list.".to_string())?;
        serde_json::from_value(documents)
            .map_err(|err| format!("Unable to decode documents payload: {err}"))
    }

    fn get_document_view(
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

    fn execute_command_response(
        &mut self,
        name: &str,
        payload: Value,
    ) -> Result<ResponseEnvelope, String> {
        self.send_request("command", name, payload)
    }

    fn recent_stderr_entries(&self, after_sequence: u64) -> Vec<BackendLogEntry> {
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

#[cfg(feature = "alpha-compat")]
impl Drop for ProcessBackendClient {
    fn drop(&mut self) {
        self.stdin.take();
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
        let runtime = SqliteRuntime::open(&db_path)
            .map_err(|err| format!("Unable to open beta runtime database: {err}"))?;

        let mut client = Self {
            runtime,
            logs: VecDeque::with_capacity(256),
            sequence: 0,
        };
        client.push_log(format!("beta-runtime ready db={}", db_path.display()));
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

#[cfg(feature = "alpha-compat")]
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

#[cfg(feature = "alpha-compat")]
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

#[cfg(feature = "alpha-compat")]
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
