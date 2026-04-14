use crate::backend::{AppSnapshot, BackendClient, DocumentListItem, DocumentView, SessionSnapshot};
use crate::logger::AppLogger;
use serde_json::Value;
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
    pub is_terminal: bool,
}

#[derive(Clone)]
struct IngestPathCandidate {
    path: PathBuf,
    is_directory: bool,
}

pub const COMMANDS: [CommandSpec; 13] = [
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
        name: "/ingest_url",
        usage: "/ingest_url <url>",
        summary: "fetch a web article and ingest its readable content",
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
    logger: AppLogger,
    voice_commands: crate::config::VoiceCommandsSection,
    pub input: String,
    pub should_quit: bool,
    pub app_snapshot: Option<AppSnapshot>,
    pub session_snapshot: Option<SessionSnapshot>,
    pub document_view: Option<DocumentView>,
    pub library_documents: Vec<DocumentListItem>,
    pub messages: VecDeque<String>,
    document_scroll: u16,
    last_backend_log_sequence: u64,
    selected_document_id: Option<String>,
    history: VecDeque<String>,
    history_index: Option<usize>,
    history_draft: Option<String>,
    launched_at: Instant,
    last_refresh: Instant,
    quit_armed_at: Option<Instant>,
    suggestion_index: usize,
    pending_play: Option<String>,
}

impl App {
    pub fn new(
        backend: BackendClient,
        logger: AppLogger,
        voice_commands: crate::config::VoiceCommandsSection,
    ) -> Result<Self, String> {
        let mut app = Self {
            backend,
            logger,
            voice_commands,
            input: String::new(),
            should_quit: false,
            app_snapshot: None,
            session_snapshot: None,
            document_view: None,
            library_documents: Vec::new(),
            messages: VecDeque::new(),
            document_scroll: 0,
            last_backend_log_sequence: 0,
            selected_document_id: None,
            history: VecDeque::new(),
            history_index: None,
            history_draft: None,
            launched_at: Instant::now(),
            last_refresh: Instant::now() - Duration::from_secs(1),
            quit_armed_at: None,
            suggestion_index: 0,
            pending_play: None,
        };
        app.refresh()?;
        app.poll_backend_logs();
        app.push_message(format!(
            "Connected to {}. Type /play <path|id>.",
            app.backend.mode_label()
        ));
        Ok(app)
    }

    pub fn run_startup_checks(&mut self) {
        match self.backend.get_doctor_report() {
            Ok(report) => {
                let warnings = startup_warnings(&report);
                if warnings.is_empty() {
                    self.push_message(
                        "Startup checks: configured providers look ready.".to_string(),
                    );
                    return;
                }
                for warning in warnings {
                    self.push_message(format!("startup: {warning}"));
                }
            }
            Err(message) => {
                self.push_message(format!("startup: unable to load doctor report: {message}"));
            }
        }
    }

    pub fn refresh_if_due(&mut self) {
        if self.last_refresh.elapsed() >= Duration::from_millis(750) {
            if let Err(message) = self.refresh() {
                self.logger
                    .warn(format!("Scheduled refresh failed: {message}"));
            }
        }
    }

    pub fn flush_pending_play(&mut self) {
        let Some(document_id) = self.pending_play.take() else {
            return;
        };
        if self.backend.is_busy() {
            // Re-queue — a previous async command is still running.
            self.pending_play = Some(document_id);
            return;
        }
        self.push_message("Starting playback (synthesizing first chunk...)".to_string());
        self.backend.start_session_async(&document_id);
    }

    pub fn poll_async_command(&mut self) {
        if let Some(result) = self.backend.poll_async_result() {
            match result {
                Ok(message) => {
                    self.push_message(message);
                    let _ = self.refresh();
                }
                Err(message) => self.push_message(format!("error: {message}")),
            }
        }
    }

    pub fn check_auto_advance(&mut self) {
        if self.backend.check_auto_advance() {
            let _ = self.refresh();
        }
    }

    pub fn poll_backend_logs(&mut self) {
        let entries = self
            .backend
            .recent_stderr_entries(self.last_backend_log_sequence);
        for entry in entries {
            self.last_backend_log_sequence = entry.sequence;
            self.push_message(entry.line);
        }
    }

    pub fn refresh(&mut self) -> Result<(), String> {
        // Each query may be skipped (runtime busy with prefetch) — keep previous data.
        if let Ok(snapshot) = self.backend.get_app_snapshot() {
            self.app_snapshot = Some(snapshot);
        }
        if let Ok(snapshot) = self.backend.get_session_snapshot() {
            self.session_snapshot = snapshot;
        }
        if let Ok(docs) = self.backend.list_documents() {
            self.library_documents = docs;
        }
        if let Ok(view) = self
            .backend
            .get_document_view(self.selected_document_id.as_deref())
        {
            self.document_view = view;
        }
        if self.document_view.is_none() {
            self.selected_document_id = None;
            if let Ok(view) = self.backend.get_document_view(None) {
                self.document_view = view;
            }
        }
        self.last_refresh = Instant::now();
        self.poll_backend_logs();
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
        self.logger.info(format!("Submitting command: {command}"));
        match self.execute_command(&command) {
            Ok(message) => self.push_message(message),
            Err(message) => self.push_message(format!("error: {message}")),
        }
    }

    pub fn confirm_input(&mut self) {
        if let Some(selected) = self.selected_suggestion() {
            if selected.value != self.input.trim() || !selected.is_terminal {
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
            self.graceful_shutdown();
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
            return "Type / to explore commands. Empty input: arrows navigate, PageUp/PageDown scroll document, Home/End jump. Ctrl+P/Ctrl+N browse history.".to_string();
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
            "play" => {
                if self.backend.is_busy() {
                    return Err("A command is already running, please wait.".to_string());
                }
                self.backend.start_session_async(argument);
                "Starting playback (synthesizing first chunk...)".to_string()
            }
            "ingest" => {
                if argument.is_empty() {
                    return Err("Usage: /ingest <path>".to_string());
                }
                let resolved_path = expand_shell_like_path(argument);
                let response = self.backend.ingest_document(&resolved_path)?;
                let document_id = response.document_id;
                self.selected_document_id = document_id.clone();
                self.pending_play = document_id;
                response.message
            }
            "ingest_url" => {
                if argument.is_empty() {
                    return Err("Usage: /ingest_url <url>".to_string());
                }
                // Do NOT expand_shell_like_path the argument — URLs must stay
                // verbatim (no `~` expansion, no path trimming).
                let response = self.backend.ingest_url(argument)?;
                let document_id = response.document_id;
                self.selected_document_id = document_id.clone();
                self.pending_play = document_id;
                response.message
            }
            "pause" => self.backend.pause_session()?,
            "resume" => {
                self.backend.resume_session();
                "Resuming...".to_string()
            }
            "stop" => self.backend.stop_session()?,
            "repeat" => {
                self.backend.repeat_chunk();
                "Repeating chunk...".to_string()
            }
            "restart" => {
                self.backend.restart_chapter();
                "Restarting chapter...".to_string()
            }
            "back" => {
                self.backend.previous_chunk();
                "Previous chunk...".to_string()
            }
            "next" => {
                self.backend.next_chapter();
                "Next chapter...".to_string()
            }
            "note" => {
                if argument.is_empty() {
                    return Err("Usage: /note <text>".to_string());
                }
                self.backend.create_note(argument)?
            }
            "quit" | "exit" => {
                self.graceful_shutdown();
                "Closing TUI.".to_string()
            }
            _ => return Err(format!("Unknown command: /{name}")),
        };

        if !self.should_quit && name != "refresh" && name != "help" {
            let _ = self.refresh();
        }
        Ok(message)
    }

    pub fn push_message(&mut self, message: String) {
        self.logger.info(format!("ui-log {message}"));
        self.messages.push_back(message);
        while self.messages.len() > 200 {
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

    fn run_shortcut_command(
        &mut self,
        command: impl FnOnce(&mut BackendClient) -> Result<String, String>,
    ) {
        if self.backend.is_busy() {
            self.push_message("Busy — please wait for the current command to finish.".to_string());
            return;
        }
        match command(&mut self.backend) {
            Ok(message) => {
                self.push_message(message);
                let _ = self.refresh();
            }
            Err(message) => self.push_message(format!("error: {message}")),
        }
    }

    /// Fire-and-forget shortcut for commands that run async (TTS synthesis).
    fn run_async_shortcut(&mut self, label: &str, command: impl FnOnce(&mut BackendClient)) {
        if self.backend.is_busy() {
            self.push_message("Busy — please wait for the current command to finish.".to_string());
            return;
        }
        command(&mut self.backend);
        self.push_message(format!("{label}..."));
    }

    pub fn animation_tick(&self) -> usize {
        (self.launched_at.elapsed().as_millis() / 90) as usize
    }

    pub fn waveform_levels(&self) -> (Vec<f32>, Vec<f32>) {
        self.backend.waveform_levels()
    }

    fn graceful_shutdown(&mut self) {
        let _ = self.backend.stop_session();
        self.should_quit = true;
    }

    pub fn navigate_previous_chunk(&mut self) {
        self.run_async_shortcut("Previous chunk", BackendClient::previous_chunk);
    }

    pub fn navigate_next_chunk(&mut self) {
        self.run_async_shortcut("Next chunk", BackendClient::next_chunk);
    }

    pub fn navigate_previous_chapter(&mut self) {
        self.run_async_shortcut("Previous chapter", BackendClient::previous_chapter);
    }

    pub fn navigate_next_chapter(&mut self) {
        self.run_async_shortcut("Next chapter", BackendClient::next_chapter);
    }

    fn voice_bookmark(&mut self) {
        if let Some(session) = &self.session_snapshot {
            let label = format!(
                "[BOOKMARK] {}, ch.{} chunk{}",
                session.section_title,
                session.section_index + 1,
                session.chunk_index + 1,
            );
            match self.backend.create_note(&label) {
                Ok(_) => self.push_message(format!(
                    "Bookmark saved: ch.{} chunk{}",
                    session.section_index + 1,
                    session.chunk_index + 1,
                )),
                Err(e) => self.push_message(format!("Bookmark error: {e}")),
            }
        } else {
            self.push_message("No active session.".to_string());
        }
    }

    fn voice_where(&mut self) {
        if let Some(session) = &self.session_snapshot {
            let chapter_info = format!(
                "chapter {}/{} ({})",
                session.section_index + 1,
                session.section_count,
                session.section_title,
            );
            let chunk_info = if let Some(view) = &self.document_view {
                format!("chunk {}/{}", session.chunk_index + 1, view.chunk_count)
            } else {
                format!("chunk {}", session.chunk_index + 1)
            };
            self.push_message(format!("Position: {chapter_info}, {chunk_info}"));
        } else {
            self.push_message("No active session.".to_string());
        }
    }

    pub fn poll_voice_event(&mut self) -> Option<(Option<String>, Option<String>)> {
        self.backend.poll_voice_event()
    }

    pub fn handle_voice_command(&mut self, raw: &str) {
        let listening = self
            .session_snapshot
            .as_ref()
            .map(|s| s.command_listening_active)
            .unwrap_or(false);
        if !listening {
            return;
        }

        match self.voice_commands.resolve_action(raw) {
            Some("pause") => self.run_shortcut_command(BackendClient::pause_session),
            Some("next") => self.navigate_next_chunk(),
            Some("back") => self.navigate_previous_chunk(),
            Some("stop") => self.run_shortcut_command(BackendClient::stop_session),
            Some("repeat") => self.run_async_shortcut("Repeat", BackendClient::repeat_chunk),
            Some("resume") => self.run_async_shortcut("Resume", BackendClient::resume_session),
            Some("next_chapter") => self.navigate_next_chapter(),
            Some("prev_chapter") => self.navigate_previous_chapter(),
            Some("note") => self.push_message("Note: use /note <text> for now.".to_string()),
            Some("bookmark") => self.voice_bookmark(),
            Some("where") => self.voice_where(),
            _ => {}
        }
    }

    pub fn scroll_document_up(&mut self, amount: u16) {
        self.document_scroll = self.document_scroll.saturating_sub(amount);
    }

    pub fn scroll_document_down(&mut self, amount: u16) {
        self.document_scroll = self.document_scroll.saturating_add(amount);
    }

    pub fn scroll_document_to_top(&mut self) {
        self.document_scroll = 0;
    }

    pub fn scroll_document_to_bottom(&mut self) {
        self.document_scroll = u16::MAX;
    }

    pub fn document_scroll(&self) -> u16 {
        self.document_scroll
    }

    pub fn sync_document_scroll(
        &mut self,
        active_line_index: Option<usize>,
        viewport_height: u16,
        total_lines: usize,
    ) {
        let max_scroll = total_lines.saturating_sub(viewport_height as usize) as u16;
        self.document_scroll = self.document_scroll.min(max_scroll);
        let Some(active_line_index) = active_line_index else {
            return;
        };
        let active_line = active_line_index as u16;
        let follow_margin = (viewport_height / 4).clamp(1, 4);
        let viewport_top = self.document_scroll;
        let viewport_bottom = self.document_scroll.saturating_add(viewport_height);
        let top_trigger = viewport_top.saturating_add(follow_margin);
        let bottom_trigger = viewport_bottom.saturating_sub(follow_margin);

        if active_line < top_trigger {
            self.document_scroll = active_line.saturating_sub(follow_margin).min(max_scroll);
        } else if active_line >= bottom_trigger {
            let preferred_offset = viewport_height / 3;
            self.document_scroll = active_line.saturating_sub(preferred_offset).min(max_scroll);
        }
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
                is_terminal: true,
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
                is_terminal: true,
            })
            .collect()
    }

    fn ingest_suggestions(&self, argument_prefix: &str) -> Vec<SuggestionItem> {
        discover_ingestable_files(argument_prefix)
            .into_iter()
            .map(|candidate| SuggestionItem {
                value: if candidate.is_directory {
                    format!("/ingest {}/", candidate.path.display())
                } else {
                    format!("/ingest {}", candidate.path.display())
                },
                label: if candidate.is_directory {
                    format!(
                        "{}/",
                        candidate
                            .path
                            .file_name()
                            .and_then(|name| name.to_str())
                            .unwrap_or_default()
                    )
                } else {
                    candidate
                        .path
                        .file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or_default()
                        .to_string()
                },
                summary: candidate.path.display().to_string(),
                is_terminal: !candidate.is_directory,
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

fn discover_ingestable_files(argument_prefix: &str) -> Vec<IngestPathCandidate> {
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

    let mut candidates: Vec<IngestPathCandidate> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_dir() || is_ingestable_extension(path))
        .filter(|path| {
            partial_name.is_empty()
                || path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.to_lowercase().contains(&partial_name))
        })
        .map(|path| IngestPathCandidate {
            is_directory: path.is_dir(),
            path,
        })
        .collect();

    candidates.sort_by(|left, right| {
        right
            .is_directory
            .cmp(&left.is_directory)
            .then_with(|| left.path.cmp(&right.path))
    });
    candidates
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
                "md" | "markdown" | "txt" | "pdf" | "epub"
            )
        })
}

fn startup_warnings(report: &Value) -> Vec<String> {
    let mut warnings = Vec::new();

    let providers = report.get("providers").and_then(Value::as_object);
    let resolved = report.get("resolved_providers").and_then(Value::as_object);
    let checks = report.get("provider_checks").and_then(Value::as_object);

    let playback_provider = providers
        .and_then(|providers| providers.get("playback"))
        .and_then(Value::as_str)
        .unwrap_or("-");
    if playback_provider == "subprocess" {
        let playback_checks = checks.and_then(|checks| checks.get("playback"));
        let command = playback_checks
            .and_then(|value| value.get("command"))
            .and_then(Value::as_str)
            .unwrap_or("-");
        let ready = playback_checks
            .and_then(|value| value.get("ready"))
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let resolved_provider = resolved
            .and_then(|providers| providers.get("playback"))
            .and_then(Value::as_str)
            .unwrap_or("-");
        if !ready && resolved_provider == "subprocess-playback" {
            warnings.push(format!(
                "playback command '{command}' is missing; /play will fail until the playback config is fixed."
            ));
        } else if !ready && resolved_provider == "fake-playback" {
            warnings.push(format!(
                "playback command '{command}' is missing; backend fell back to fake playback."
            ));
        }
    }

    push_provider_warning(
        &mut warnings,
        providers,
        resolved,
        checks,
        "dictation_stt",
        "whisper-dictation-stt",
        "Whisper dictation STT is not ready",
    );
    push_provider_warning(
        &mut warnings,
        providers,
        resolved,
        checks,
        "tts",
        "kokoro",
        "Kokoro TTS is not ready",
    );
    push_provider_warning(
        &mut warnings,
        providers,
        resolved,
        checks,
        "tts",
        "piper",
        "Piper TTS is not ready",
    );

    warnings
}

fn push_provider_warning(
    warnings: &mut Vec<String>,
    providers: Option<&serde_json::Map<String, Value>>,
    resolved: Option<&serde_json::Map<String, Value>>,
    checks: Option<&serde_json::Map<String, Value>>,
    provider_slot: &str,
    configured_name: &str,
    message: &str,
) {
    let Some(configured) = providers
        .and_then(|providers| providers.get(provider_slot))
        .and_then(Value::as_str)
    else {
        return;
    };
    if configured != configured_name {
        return;
    }

    let check_key = configured_name.replace('-', "_");
    let ready = checks
        .and_then(|checks| checks.get(&check_key))
        .and_then(|value| value.get("ready"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if ready {
        return;
    }

    let resolved_name = resolved
        .and_then(|resolved| resolved.get(provider_slot))
        .and_then(Value::as_str)
        .unwrap_or("-");
    if resolved_name.starts_with("fake-") {
        warnings.push(format!("{message}; backend fell back to {resolved_name}."));
    } else {
        warnings.push(format!(
            "{message}; the configured provider may fail at runtime."
        ));
    }
}
