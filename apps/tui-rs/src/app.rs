use crate::backend::{AppSnapshot, BackendClient, DocumentListItem, SessionSnapshot};
use serde_json::json;
use std::collections::VecDeque;
use std::time::{Duration, Instant};

const COMMANDS: [&str; 12] = [
    "/play", "/ingest", "/pause", "/resume", "/stop", "/repeat", "/restart", "/back", "/next",
    "/note", "/help", "/refresh",
];

pub struct App {
    backend: BackendClient,
    pub input: String,
    pub should_quit: bool,
    pub app_snapshot: Option<AppSnapshot>,
    pub session_snapshot: Option<SessionSnapshot>,
    pub documents: Vec<DocumentListItem>,
    pub messages: VecDeque<String>,
    last_refresh: Instant,
}

impl App {
    pub fn new(backend: BackendClient) -> Result<Self, String> {
        let mut app = Self {
            backend,
            input: String::new(),
            should_quit: false,
            app_snapshot: None,
            session_snapshot: None,
            documents: Vec::new(),
            messages: VecDeque::new(),
            last_refresh: Instant::now() - Duration::from_secs(1),
        };
        app.refresh()?;
        app.push_message("Connected to Marginalia backend. Type /play <path|id>.".to_string());
        Ok(app)
    }

    pub fn refresh_if_due(&mut self) {
        if self.last_refresh.elapsed() >= Duration::from_millis(750) {
            let _ = self.refresh();
        }
    }

    pub fn refresh(&mut self) -> Result<(), String> {
        self.app_snapshot = Some(self.backend.get_app_snapshot()?);
        self.session_snapshot = self.backend.get_session_snapshot()?;
        self.documents = self.backend.list_documents()?;
        self.last_refresh = Instant::now();
        Ok(())
    }

    pub fn submit_input(&mut self) {
        let command = self.input.trim().to_string();
        self.input.clear();
        if command.is_empty() {
            return;
        }
        match self.execute_command(&command) {
            Ok(message) => self.push_message(message),
            Err(message) => self.push_message(format!("error: {message}")),
        }
    }

    pub fn autocomplete(&mut self) {
        let input = self.input.trim();
        if !input.starts_with('/') || input.contains(' ') {
            return;
        }
        let matches: Vec<&str> = COMMANDS
            .iter()
            .copied()
            .filter(|command| command.starts_with(input))
            .collect();
        match matches.as_slice() {
            [single] => self.input = (*single).to_string(),
            [] => self.push_message("No matching slash command.".to_string()),
            _ => self.push_message(format!("Matches: {}", matches.join(", "))),
        }
    }

    pub fn push_char(&mut self, ch: char) {
        self.input.push(ch);
    }

    pub fn pop_char(&mut self) {
        self.input.pop();
    }

    pub fn clear_input(&mut self) {
        self.input.clear();
    }

    fn execute_command(&mut self, raw: &str) -> Result<String, String> {
        if !raw.starts_with('/') {
            return Err("Commands must start with /.".to_string());
        }

        let mut parts = raw[1..].splitn(2, ' ');
        let name = parts.next().unwrap_or_default();
        let argument = parts.next().unwrap_or("").trim();

        let message = match name {
            "help" => help_text().to_string(),
            "refresh" => {
                self.refresh()?;
                "Refreshed snapshots.".to_string()
            }
            "play" => self
                .backend
                .execute_command("start_session", json!({"target": argument}))?,
            "ingest" => {
                if argument.is_empty() {
                    return Err("Usage: /ingest <path>".to_string());
                }
                self.backend
                    .execute_command("ingest_document", json!({"path": argument}))?
            }
            "pause" => self.backend.execute_command("pause_session", json!({}))?,
            "resume" => self.backend.execute_command("resume_session", json!({}))?,
            "stop" => self.backend.execute_command("stop_session", json!({}))?,
            "repeat" => self.backend.execute_command("repeat_chunk", json!({}))?,
            "restart" => self.backend.execute_command("restart_chapter", json!({}))?,
            "back" => self.backend.execute_command("previous_chunk", json!({}))?,
            "next" => self.backend.execute_command("next_chapter", json!({}))?,
            "note" => {
                if argument.is_empty() {
                    return Err("Usage: /note <text>".to_string());
                }
                self.backend
                    .execute_command("create_note", json!({"text": argument}))?
            }
            "quit" | "exit" => {
                self.should_quit = true;
                "Closing TUI.".to_string()
            }
            _ => return Err(format!("Unknown command: /{name}")),
        };

        if !self.should_quit && name != "refresh" && name != "help" {
            let _ = self.refresh();
        }
        Ok(message)
    }

    fn push_message(&mut self, message: String) {
        self.messages.push_back(message);
        while self.messages.len() > 8 {
            self.messages.pop_front();
        }
    }
}

pub fn help_text() -> &'static str {
    "/play <path|id>  /ingest <path>  /pause  /resume  /stop  /repeat  /restart  /back  /next  /note <text>"
}
