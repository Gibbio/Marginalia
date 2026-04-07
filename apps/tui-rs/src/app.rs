use crate::backend::{AppSnapshot, BackendClient, DocumentListItem, DocumentView, SessionSnapshot};
use serde_json::json;
use std::collections::VecDeque;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[derive(Clone, Copy)]
pub struct CommandSpec {
    pub name: &'static str,
    pub usage: &'static str,
    pub summary: &'static str,
}

#[derive(Clone)]
pub struct SuggestionItem {
    pub value: String,
    pub label: String,
    pub summary: String,
}

#[derive(Clone)]
pub struct LocalMarkdownFile {
    pub name: String,
    pub path: PathBuf,
}

pub const COMMANDS: [CommandSpec; 12] = [
    CommandSpec {
        name: "/play",
        usage: "/play <path|id>",
        summary: "start a reading session",
    },
    CommandSpec {
        name: "/ingest",
        usage: "/ingest <path>",
        summary: "ingest a document into the library",
    },
    CommandSpec {
        name: "/pause",
        usage: "/pause",
        summary: "pause the active session",
    },
    CommandSpec {
        name: "/resume",
        usage: "/resume",
        summary: "resume the active session",
    },
    CommandSpec {
        name: "/stop",
        usage: "/stop",
        summary: "stop the active session",
    },
    CommandSpec {
        name: "/repeat",
        usage: "/repeat",
        summary: "repeat the current chunk",
    },
    CommandSpec {
        name: "/restart",
        usage: "/restart",
        summary: "restart the current chapter",
    },
    CommandSpec {
        name: "/back",
        usage: "/back",
        summary: "go to the previous chunk",
    },
    CommandSpec {
        name: "/next",
        usage: "/next",
        summary: "jump to the next chapter",
    },
    CommandSpec {
        name: "/note",
        usage: "/note <text>",
        summary: "save a note on the current position",
    },
    CommandSpec {
        name: "/help",
        usage: "/help",
        summary: "show command help",
    },
    CommandSpec {
        name: "/refresh",
        usage: "/refresh",
        summary: "refresh backend snapshots",
    },
];

pub struct App {
    backend: BackendClient,
    pub input: String,
    pub should_quit: bool,
    pub app_snapshot: Option<AppSnapshot>,
    pub session_snapshot: Option<SessionSnapshot>,
    pub document_view: Option<DocumentView>,
    pub library_documents: Vec<DocumentListItem>,
    pub local_markdown_files: Vec<LocalMarkdownFile>,
    pub messages: VecDeque<String>,
    selected_document_id: Option<String>,
    history: VecDeque<String>,
    history_index: Option<usize>,
    history_draft: Option<String>,
    launched_at: Instant,
    last_refresh: Instant,
    quit_armed_at: Option<Instant>,
    suggestion_index: usize,
}

impl App {
    pub fn new(backend: BackendClient) -> Result<Self, String> {
        let mut app = Self {
            backend,
            input: String::new(),
            should_quit: false,
            app_snapshot: None,
            session_snapshot: None,
            document_view: None,
            library_documents: Vec::new(),
            local_markdown_files: Vec::new(),
            messages: VecDeque::new(),
            selected_document_id: None,
            history: VecDeque::new(),
            history_index: None,
            history_draft: None,
            launched_at: Instant::now(),
            last_refresh: Instant::now() - Duration::from_secs(1),
            quit_armed_at: None,
            suggestion_index: 0,
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
        self.library_documents = self.backend.list_documents()?;
        self.document_view = self
            .backend
            .get_document_view(self.selected_document_id.as_deref())?;
        if self.document_view.is_none() {
            self.selected_document_id = None;
            self.document_view = self.backend.get_document_view(None)?;
        }
        self.local_markdown_files = discover_markdown_files();
        self.last_refresh = Instant::now();
        Ok(())
    }

    pub fn submit_input(&mut self) {
        let command = self.input.trim().to_string();
        self.input.clear();
        self.remember_history(&command);
        self.history_index = None;
        self.history_draft = None;
        self.suggestion_index = 0;
        if command.is_empty() {
            return;
        }
        match self.execute_command(&command) {
            Ok(message) => self.push_message(message),
            Err(message) => self.push_message(format!("error: {message}")),
        }
    }

    pub fn confirm_input(&mut self) {
        if let Some(selected) = self.selected_suggestion() {
            if selected.value != self.input.trim() {
                self.input = selected.value;
                self.history_index = None;
                self.history_draft = None;
                self.suggestion_index = 0;
                return;
            }
        }
        self.submit_input();
    }

    pub fn handle_ctrl_c(&mut self) {
        let now = Instant::now();
        if self
            .quit_armed_at
            .is_some_and(|armed_at| now.duration_since(armed_at) <= Duration::from_secs(2))
        {
            self.should_quit = true;
            return;
        }
        self.quit_armed_at = Some(now);
        self.push_message("Press Ctrl-C again within 2s to quit.".to_string());
    }

    pub fn command_input_is_empty(&self) -> bool {
        self.input.trim().is_empty()
    }

    pub fn autocomplete(&mut self) {
        let suggestions = self.suggestions();
        match suggestions.as_slice() {
            [single] => self.input = single.value.clone(),
            [] => self.push_message("No matching slash command.".to_string()),
            _ => {
                if let Some(selected) = self.selected_suggestion() {
                    self.input = selected.value;
                } else if let Some(prefix) = longest_common_prefix(self.input.trim(), &suggestions)
                {
                    self.input = prefix;
                } else {
                    self.push_message(format!(
                        "Matches: {}",
                        suggestions
                            .iter()
                            .map(|spec| spec.label.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    ));
                }
            }
        }
        self.history_index = None;
        self.history_draft = None;
        self.suggestion_index = 0;
    }

    pub fn push_char(&mut self, ch: char) {
        self.input.push(ch);
        self.history_index = None;
        self.history_draft = None;
        self.suggestion_index = 0;
    }

    pub fn pop_char(&mut self) {
        self.input.pop();
        self.history_index = None;
        self.history_draft = None;
        self.suggestion_index = 0;
    }

    pub fn clear_input(&mut self) {
        self.input.clear();
        self.history_index = None;
        self.history_draft = None;
        self.suggestion_index = 0;
    }

    pub fn select_next_history(&mut self) {
        if self.history.is_empty() {
            return;
        }
        if self.history_index.is_none() {
            self.history_draft = Some(self.input.clone());
            self.history_index = Some(self.history.len() - 1);
        } else if let Some(index) = self.history_index {
            if index + 1 < self.history.len() {
                self.history_index = Some(index + 1);
            } else {
                self.input = self.history_draft.clone().unwrap_or_default();
                self.history_index = None;
                self.history_draft = None;
                self.suggestion_index = 0;
                return;
            }
        }
        if let Some(index) = self.history_index {
            self.input = self.history[index].clone();
            self.suggestion_index = 0;
        }
    }

    pub fn select_previous_history(&mut self) {
        if self.history.is_empty() {
            return;
        }
        if self.history_index.is_none() {
            self.history_draft = Some(self.input.clone());
            self.history_index = Some(self.history.len() - 1);
        } else if let Some(index) = self.history_index {
            self.history_index = Some(index.saturating_sub(1));
        }
        if let Some(index) = self.history_index {
            self.input = self.history[index].clone();
            self.suggestion_index = 0;
        }
    }

    pub fn completion_suffix(&self) -> Option<String> {
        let selected = self.selected_suggestion()?;
        if selected.value == self.input.trim() {
            return None;
        }
        selected
            .value
            .strip_prefix(self.input.trim())
            .map(ToString::to_string)
    }

    pub fn command_hint(&self) -> String {
        let input = self.input.trim();
        if input.is_empty() {
            return "Type / to explore commands. Empty input: arrows navigate. Ctrl+P/Ctrl+N browse history."
                .to_string();
        }
        if !input.starts_with('/') {
            return "Commands must start with /.".to_string();
        }
        if let Some((command_name, _)) = self.command_context() {
            if let Some(command) = COMMANDS.iter().find(|command| command.name == command_name) {
                let suggestions = self.suggestions();
                if let Some(selected) = self.selected_suggestion() {
                    return format!(
                        "{}  {}  selected: {}",
                        command.usage, command.summary, selected.label
                    );
                }
                if !suggestions.is_empty() {
                    return format!("{} suggestions for {}", suggestions.len(), command.name);
                }
                return format!("{}  {}", command.usage, command.summary);
            }
            return "Unknown command.".to_string();
        }

        let suggestions = self.suggestions();
        match suggestions.as_slice() {
            [] => "No matching command.".to_string(),
            [single] => format!("{}  {}", single.label, single.summary),
            _ => {
                if let Some(selected) = self.selected_suggestion() {
                    format!(
                        "{} matches  selected: {}  {}",
                        suggestions.len(),
                        selected.label,
                        selected.summary
                    )
                } else {
                    format!("{} matches", suggestions.len())
                }
            }
        }
    }

    pub fn visible_suggestions(&self) -> Vec<SuggestionItem> {
        let suggestions = self.suggestions();
        if suggestions.len() <= 3 {
            return suggestions;
        }
        let selected_index = self.suggestion_index % suggestions.len();
        let start = selected_index.saturating_sub(2).min(suggestions.len() - 3);
        suggestions.into_iter().skip(start).take(3).collect()
    }

    pub fn selected_suggestion_slot(&self) -> Option<usize> {
        let visible = self.visible_suggestions();
        let selected = self.selected_suggestion()?;
        visible
            .iter()
            .position(|suggestion| suggestion.value == selected.value)
    }

    pub fn select_next_suggestion(&mut self) {
        let suggestions = self.suggestions();
        if suggestions.is_empty() {
            return;
        }
        self.suggestion_index = (self.suggestion_index + 1) % suggestions.len();
    }

    pub fn select_previous_suggestion(&mut self) {
        let suggestions = self.suggestions();
        if suggestions.is_empty() {
            return;
        }
        self.suggestion_index = if self.suggestion_index == 0 {
            suggestions.len() - 1
        } else {
            self.suggestion_index - 1
        };
    }

    fn selected_suggestion(&self) -> Option<SuggestionItem> {
        let suggestions = self.suggestions();
        if suggestions.is_empty() {
            return None;
        }
        Some(suggestions[self.suggestion_index % suggestions.len()].clone())
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
                let resolved_path = expand_shell_like_path(argument);
                let response = self.backend.execute_command_response(
                    "ingest_document",
                    json!({"path": resolved_path.display().to_string()}),
                )?;
                if response.status != "ok" {
                    return Err(response.message);
                }
                self.selected_document_id = response
                    .payload
                    .get("document")
                    .and_then(|document| document.get("document_id"))
                    .and_then(|document_id| document_id.as_str())
                    .map(ToString::to_string);
                response.message
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

    fn remember_history(&mut self, command: &str) {
        if command.is_empty() {
            return;
        }
        if self
            .history
            .back()
            .is_some_and(|previous| previous == command)
        {
            return;
        }
        self.history.push_back(command.to_string());
        while self.history.len() > 50 {
            self.history.pop_front();
        }
    }

    fn run_shortcut_command(&mut self, command_name: &str) {
        match self.backend.execute_command(command_name, json!({})) {
            Ok(message) => {
                self.push_message(message);
                let _ = self.refresh();
            }
            Err(message) => self.push_message(format!("error: {message}")),
        }
    }

    pub fn animation_frame(&self) -> usize {
        ((self.launched_at.elapsed().as_millis() / 140) % 3) as usize
    }

    pub fn navigate_previous_chunk(&mut self) {
        self.run_shortcut_command("previous_chunk");
    }

    pub fn navigate_next_chunk(&mut self) {
        self.run_shortcut_command("next_chunk");
    }

    pub fn navigate_previous_chapter(&mut self) {
        self.run_shortcut_command("previous_chapter");
    }

    pub fn navigate_next_chapter(&mut self) {
        self.run_shortcut_command("next_chapter");
    }

    fn suggestions(&self) -> Vec<SuggestionItem> {
        if !self.input.trim_start().starts_with('/') {
            return Vec::new();
        }
        if let Some((command_name, argument_prefix)) = self.command_context() {
            return self.argument_suggestions(command_name, argument_prefix);
        }
        let input = self.input.trim();
        COMMANDS
            .iter()
            .filter(|command| command.name.starts_with(input))
            .map(|command| SuggestionItem {
                value: command.name.to_string(),
                label: command.name.to_string(),
                summary: command.summary.to_string(),
            })
            .collect()
    }

    fn command_context(&self) -> Option<(&str, &str)> {
        let input = self.input.as_str();
        let (command_name, argument_prefix) = input.split_once(' ')?;
        if !command_name.starts_with('/') {
            return None;
        }
        Some((command_name.trim(), argument_prefix.trim_start()))
    }

    fn argument_suggestions(
        &self,
        command_name: &str,
        argument_prefix: &str,
    ) -> Vec<SuggestionItem> {
        match command_name {
            "/play" => self.play_suggestions(argument_prefix),
            "/ingest" => self.ingest_suggestions(argument_prefix),
            _ => Vec::new(),
        }
    }

    fn play_suggestions(&self, argument_prefix: &str) -> Vec<SuggestionItem> {
        let query = argument_prefix.trim().to_lowercase();
        self.library_documents
            .iter()
            .filter(|document| {
                query.is_empty()
                    || document.document_id.to_lowercase().starts_with(&query)
                    || document.title.to_lowercase().contains(&query)
            })
            .map(|document| SuggestionItem {
                value: format!("/play {}", document.document_id),
                label: document.document_id.clone(),
                summary: format!(
                    "{} ({} ch, {} chunks)",
                    document.title, document.chapter_count, document.chunk_count
                ),
            })
            .collect()
    }

    fn ingest_suggestions(&self, argument_prefix: &str) -> Vec<SuggestionItem> {
        discover_ingestable_files(argument_prefix)
            .into_iter()
            .map(|file| SuggestionItem {
                value: format!("/ingest {}", file.path.display()),
                label: file
                    .path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or_default()
                    .to_string(),
                summary: file.path.display().to_string(),
            })
            .collect()
    }
}

pub fn help_text() -> &'static str {
    "/play <path|id>  /ingest <path>  /pause  /resume  /stop  /repeat  /restart  /back  /next  /note <text>"
}

fn longest_common_prefix(input: &str, matches: &[SuggestionItem]) -> Option<String> {
    let first = matches.first()?.value.as_str();
    let mut prefix = String::new();

    for (index, ch) in first.chars().enumerate() {
        if matches
            .iter()
            .all(|suggestion| suggestion.value.chars().nth(index) == Some(ch))
        {
            prefix.push(ch);
        } else {
            break;
        }
    }

    if prefix.len() > input.len() {
        Some(prefix)
    } else {
        None
    }
}

fn discover_markdown_files() -> Vec<LocalMarkdownFile> {
    let Ok(current_dir) = std::env::current_dir() else {
        return Vec::new();
    };
    let Ok(entries) = fs::read_dir(current_dir) else {
        return Vec::new();
    };

    let mut files: Vec<LocalMarkdownFile> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .filter(|path| {
            path.extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("md"))
        })
        .filter_map(|path| {
            let name = path.file_name()?.to_str()?.to_string();
            Some(LocalMarkdownFile { name, path })
        })
        .collect();

    files.sort_by(|left, right| left.name.cmp(&right.name));
    files
}

fn discover_ingestable_files(argument_prefix: &str) -> Vec<LocalMarkdownFile> {
    let trimmed = argument_prefix.trim();
    let expanded = expand_shell_like_path(trimmed);

    let (directory, partial_name) = if trimmed.is_empty() {
        (
            env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            String::new(),
        )
    } else if trimmed.ends_with('/') || expanded.is_dir() {
        (expanded, String::new())
    } else {
        let parent = expanded
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
        let partial = expanded
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_lowercase();
        (parent, partial)
    };

    let Ok(entries) = fs::read_dir(directory) else {
        return Vec::new();
    };

    let mut files: Vec<LocalMarkdownFile> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .filter(|path| is_ingestable_extension(path))
        .filter(|path| {
            partial_name.is_empty()
                || path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.to_lowercase().contains(&partial_name))
        })
        .filter_map(|path| {
            let name = path.file_name()?.to_str()?.to_string();
            Some(LocalMarkdownFile { name, path })
        })
        .collect();

    files.sort_by(|left, right| left.name.cmp(&right.name));
    files
}

fn expand_shell_like_path(input: &str) -> PathBuf {
    let mut expanded = input.trim().to_string();
    if let Ok(home) = env::var("HOME") {
        if expanded == "~" {
            expanded = home.clone();
        } else if let Some(rest) = expanded.strip_prefix("~/") {
            expanded = format!("{home}/{rest}");
        }
        expanded = expanded.replace("${HOME}", &home);
        expanded = expanded.replace("$HOME", &home);
    }

    let path = PathBuf::from(expanded);
    if path.is_absolute() {
        path
    } else {
        env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(path)
    }
}

fn is_ingestable_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "md" | "markdown" | "txt"
            )
        })
}
