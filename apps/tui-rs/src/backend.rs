use marginalia_core::frontend as core_frontend;
use marginalia_runtime::SqliteRuntime;
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

pub(crate) enum BackendClient {
    Process(ProcessBackendClient),
    Beta(BetaBackendClient),
}

impl BackendClient {
    pub fn spawn(config_path: Option<&Path>) -> Result<Self, String> {
        let mode = env::var("MARGINALIA_TUI_BACKEND")
            .unwrap_or_else(|_| "python".to_string())
            .to_ascii_lowercase();

        match mode.as_str() {
            "beta" => BetaBackendClient::spawn().map(Self::Beta),
            _ => ProcessBackendClient::spawn(config_path).map(Self::Process),
        }
    }

    pub fn get_app_snapshot(&mut self) -> Result<AppSnapshot, String> {
        match self {
            Self::Process(client) => client.get_app_snapshot(),
            Self::Beta(client) => client.get_app_snapshot(),
        }
    }

    pub fn get_session_snapshot(&mut self) -> Result<Option<SessionSnapshot>, String> {
        match self {
            Self::Process(client) => client.get_session_snapshot(),
            Self::Beta(client) => client.get_session_snapshot(),
        }
    }

    pub fn get_doctor_report(&mut self) -> Result<Value, String> {
        match self {
            Self::Process(client) => client.get_doctor_report(),
            Self::Beta(client) => client.get_doctor_report(),
        }
    }

    pub fn list_documents(&mut self) -> Result<Vec<DocumentListItem>, String> {
        match self {
            Self::Process(client) => client.list_documents(),
            Self::Beta(client) => client.list_documents(),
        }
    }

    pub fn get_document_view(
        &mut self,
        document_id: Option<&str>,
    ) -> Result<Option<DocumentView>, String> {
        match self {
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
            Self::Process(client) => client.execute_command_response(name, payload),
            Self::Beta(client) => client.execute_command_response(name, payload),
        }
    }

    pub fn recent_stderr_entries(&self, after_sequence: u64) -> Vec<BackendLogEntry> {
        match self {
            Self::Process(client) => client.recent_stderr_entries(after_sequence),
            Self::Beta(client) => client.recent_stderr_entries(after_sequence),
        }
    }
}

pub(crate) struct ProcessBackendClient {
    child: Child,
    stdin: Option<BufWriter<ChildStdin>>,
    response_rx: mpsc::Receiver<String>,
    reader_thread: Option<JoinHandle<()>>,
    request_counter: u64,
    stderr_lines: Arc<Mutex<VecDeque<BackendLogEntry>>>,
    stderr_thread: Option<JoinHandle<()>>,
}

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
        Ok(self.runtime.app_snapshot().into())
    }

    fn get_session_snapshot(&mut self) -> Result<Option<SessionSnapshot>, String> {
        self.runtime
            .session_snapshot()
            .map(|snapshot| snapshot.map(Into::into))
            .map_err(|err| err.to_string())
    }

    fn get_doctor_report(&mut self) -> Result<Value, String> {
        Ok(self.runtime.doctor_report())
    }

    fn list_documents(&mut self) -> Result<Vec<DocumentListItem>, String> {
        Ok(self
            .runtime
            .list_documents()
            .into_iter()
            .map(Into::into)
            .collect())
    }

    fn get_document_view(
        &mut self,
        document_id: Option<&str>,
    ) -> Result<Option<DocumentView>, String> {
        Ok(self.runtime.document_view(document_id).map(Into::into))
    }

    fn execute_command_response(
        &mut self,
        name: &str,
        payload: Value,
    ) -> Result<ResponseEnvelope, String> {
        match name {
            "ingest_document" => {
                let path = payload
                    .get("path")
                    .and_then(Value::as_str)
                    .ok_or_else(|| "ingest_document requires a path.".to_string())?;
                let outcome = self
                    .runtime
                    .ingest_path(Path::new(path))
                    .map_err(|err| err.to_string())?;
                self.push_log(format!("beta ingest path={path} document_id={}", outcome.document.document_id));
                Ok(ResponseEnvelope {
                    status: "ok".to_string(),
                    message: "Document ingested into local SQLite storage.".to_string(),
                    payload: json!({
                        "document": {
                            "document_id": outcome.document.document_id,
                            "title": outcome.document.title,
                        }
                    }),
                    request_id: None,
                })
            }
            "start_session" => {
                let target = payload
                    .get("target")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .trim()
                    .to_string();
                if target.is_empty() {
                    return Err("start_session requires a target.".to_string());
                }

                let document_id = if Path::new(&target).exists() {
                    self.runtime
                        .ingest_path(Path::new(&target))
                        .map_err(|err| err.to_string())?
                        .document
                        .document_id
                } else {
                    target
                };

                let session = self
                    .runtime
                    .start_session(&document_id)
                    .map_err(|err| err.to_string())?;
                self.push_log(format!(
                    "beta start session_id={} document_id={}",
                    session.session_id, session.document_id
                ));
                Ok(ok_response("Reading session started."))
            }
            "pause_session" => {
                self.runtime.pause_session().map_err(|err| err.to_string())?;
                self.push_log("beta pause".to_string());
                Ok(ok_response("Reading session paused."))
            }
            "resume_session" => {
                self.runtime.resume_session().map_err(|err| err.to_string())?;
                self.push_log("beta resume".to_string());
                Ok(ok_response("Reading session resumed."))
            }
            "stop_session" => {
                self.runtime.stop_session().map_err(|err| err.to_string())?;
                self.push_log("beta stop".to_string());
                Ok(ok_response("Reading session stopped."))
            }
            "repeat_chunk" => {
                self.runtime.repeat_chunk().map_err(|err| err.to_string())?;
                self.push_log("beta repeat_chunk".to_string());
                Ok(ok_response("Current chunk repeated."))
            }
            "restart_chapter" => {
                self.runtime
                    .restart_chapter()
                    .map_err(|err| err.to_string())?;
                self.push_log("beta restart_chapter".to_string());
                Ok(ok_response("Current chapter restarted."))
            }
            "previous_chunk" => {
                self.runtime.previous_chunk().map_err(|err| err.to_string())?;
                self.push_log("beta previous_chunk".to_string());
                Ok(ok_response("Moved to previous chunk."))
            }
            "next_chunk" => {
                self.runtime.next_chunk().map_err(|err| err.to_string())?;
                self.push_log("beta next_chunk".to_string());
                Ok(ok_response("Moved to next chunk."))
            }
            "previous_chapter" => {
                self.runtime
                    .previous_chapter()
                    .map_err(|err| err.to_string())?;
                self.push_log("beta previous_chapter".to_string());
                Ok(ok_response("Moved to previous chapter."))
            }
            "next_chapter" => {
                self.runtime.next_chapter().map_err(|err| err.to_string())?;
                self.push_log("beta next_chapter".to_string());
                Ok(ok_response("Moved to next chapter."))
            }
            "create_note" => {
                let text = payload
                    .get("text")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let note = self.runtime.create_note(text).map_err(|err| err.to_string())?;
                self.push_log(format!("beta create_note note_id={}", note.note_id));
                Ok(ok_response("Note saved."))
            }
            other => Err(format!("Unsupported beta backend command: {other}")),
        }
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
}

fn ok_response(message: impl Into<String>) -> ResponseEnvelope {
    ResponseEnvelope {
        status: "ok".to_string(),
        message: message.into(),
        payload: json!({}),
        request_id: None,
    }
}

impl From<core_frontend::AppSnapshot> for AppSnapshot {
    fn from(value: core_frontend::AppSnapshot) -> Self {
        Self {
            active_session_id: value.active_session_id,
            document_count: value.document_count as u32,
            latest_document_id: value.latest_document_id,
            playback_state: value.playback_state,
            runtime_status: value.runtime_status,
            state: value.state,
        }
    }
}

impl From<core_frontend::SessionSnapshot> for SessionSnapshot {
    fn from(value: core_frontend::SessionSnapshot) -> Self {
        Self {
            anchor: value.anchor,
            chunk_index: value.chunk_index as u32,
            chunk_text: value.chunk_text,
            command_listening_active: value.command_listening_active,
            command_stt_provider: value.command_stt_provider,
            document_id: value.document_id,
            notes_count: value.notes_count as u32,
            playback_provider: value.playback_provider,
            playback_state: value.playback_state,
            section_count: value.section_count as u32,
            section_index: value.section_index as u32,
            section_title: value.section_title,
            session_id: value.session_id,
            state: value.state,
            tts_provider: value.tts_provider,
            voice: value.voice,
        }
    }
}

impl From<core_frontend::DocumentListItem> for DocumentListItem {
    fn from(value: core_frontend::DocumentListItem) -> Self {
        Self {
            chapter_count: value.chapter_count as u32,
            chunk_count: value.chunk_count as u32,
            document_id: value.document_id,
            title: value.title,
        }
    }
}

impl From<core_frontend::DocumentView> for DocumentView {
    fn from(value: core_frontend::DocumentView) -> Self {
        Self {
            active_chunk_index: value.active_chunk_index.map(|value| value as u32),
            active_section_index: value.active_section_index.map(|value| value as u32),
            chapter_count: value.chapter_count as u32,
            chunk_count: value.chunk_count as u32,
            document_id: value.document_id,
            sections: value.sections.into_iter().map(Into::into).collect(),
            source_path: value.source_path,
            title: value.title,
        }
    }
}

impl From<core_frontend::DocumentSectionView> for DocumentSectionView {
    fn from(value: core_frontend::DocumentSectionView) -> Self {
        Self {
            chunks: value.chunks.into_iter().map(Into::into).collect(),
            index: value.index as u32,
            title: value.title,
        }
    }
}

impl From<core_frontend::DocumentChunkView> for DocumentChunkView {
    fn from(value: core_frontend::DocumentChunkView) -> Self {
        Self {
            anchor: value.anchor,
            index: value.index as u32,
            is_active: value.is_active,
            is_read: value.is_read,
            text: value.text,
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
