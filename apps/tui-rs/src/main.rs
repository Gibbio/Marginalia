mod app;
mod backend;

use app::{help_text, App};
use backend::BackendClient;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Wrap};
use ratatui::{Frame, Terminal};
use std::env;
use std::io::stdout;
use std::path::PathBuf;
use std::time::Duration;

fn main() -> Result<(), String> {
    let config_path = env::var("MARGINALIA_CONFIG").ok().map(PathBuf::from);
    let backend = BackendClient::spawn(config_path.as_deref())?;
    let mut app = App::new(backend)?;

    enable_raw_mode().map_err(|err| format!("Unable to enable raw mode: {err}"))?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen)
        .map_err(|err| format!("Unable to enter alternate screen: {err}"))?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)
        .map_err(|err| format!("Unable to create terminal backend: {err}"))?;

    let result = run_tui(&mut terminal, &mut app);

    disable_raw_mode().map_err(|err| format!("Unable to disable raw mode: {err}"))?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)
        .map_err(|err| format!("Unable to leave alternate screen: {err}"))?;
    terminal
        .show_cursor()
        .map_err(|err| format!("Unable to restore terminal cursor: {err}"))?;

    result
}

fn run_tui(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    app: &mut App,
) -> Result<(), String> {
    while !app.should_quit {
        app.refresh_if_due();
        terminal
            .draw(|frame| render(frame, app))
            .map_err(|err| format!("Unable to draw terminal frame: {err}"))?;

        if event::poll(Duration::from_millis(100))
            .map_err(|err| format!("Unable to poll terminal events: {err}"))?
        {
            if let Event::Key(key) =
                event::read().map_err(|err| format!("Unable to read terminal event: {err}"))?
            {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                match key.code {
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        app.should_quit = true;
                    }
                    KeyCode::Enter => app.submit_input(),
                    KeyCode::Backspace => app.pop_char(),
                    KeyCode::Esc => app.clear_input(),
                    KeyCode::Tab => app.autocomplete(),
                    KeyCode::Char(ch) => app.push_char(ch),
                    _ => {}
                }
            }
        }
    }

    Ok(())
}

fn render(frame: &mut Frame, app: &App) {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(7),
            Constraint::Min(10),
            Constraint::Length(8),
            Constraint::Length(3),
        ])
        .split(frame.area());
    let top = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(vertical[1]);

    let header = Paragraph::new(vec![
        Line::from(Span::styled(
            "Marginalia Rust TUI",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from("Frontend ratatui + crossterm over backend stdio"),
        Line::from(help_text()),
    ])
    .block(Block::default().borders(Borders::ALL).title("Overview"))
    .wrap(Wrap { trim: true });

    let status = Paragraph::new(render_status_lines(app))
        .block(Block::default().borders(Borders::ALL).title("Session"))
        .wrap(Wrap { trim: true });

    let documents = List::new(render_documents(app))
        .block(Block::default().borders(Borders::ALL).title("Documents"));

    let messages = Paragraph::new(render_messages(app))
        .block(Block::default().borders(Borders::ALL).title("Log"))
        .wrap(Wrap { trim: true });

    let input = Paragraph::new(app.input.as_str())
        .block(Block::default().borders(Borders::ALL).title("Command"))
        .style(Style::default().fg(Color::Yellow));

    frame.render_widget(header, vertical[0]);
    frame.render_widget(status, top[0]);
    frame.render_widget(documents, top[1]);
    frame.render_widget(messages, vertical[2]);
    frame.render_widget(input, vertical[3]);
}

fn render_status_lines(app: &App) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    if let Some(snapshot) = &app.app_snapshot {
        lines.push(Line::from(format!(
            "backend state={} documents={} runtime={}",
            snapshot.state,
            snapshot.document_count,
            snapshot.runtime_status.as_deref().unwrap_or("-")
        )));
        lines.push(Line::from(format!(
            "active_session={} playback={} latest_document={}",
            snapshot.active_session_id.as_deref().unwrap_or("-"),
            snapshot.playback_state.as_deref().unwrap_or("-"),
            snapshot.latest_document_id.as_deref().unwrap_or("-")
        )));
    }

    if let Some(session) = &app.session_snapshot {
        lines.push(Line::from("".to_string()));
        lines.push(Line::from(format!(
            "session={} state={} chapter {}/{} chunk {}",
            session.session_id,
            session.state,
            session.section_index + 1,
            session.section_count,
            session.chunk_index + 1
        )));
        lines.push(Line::from(format!(
            "document={} section={}",
            session.document_id, session.section_title
        )));
        lines.push(Line::from(format!(
            "tts={} playback={} command_stt={} voice={} notes={}",
            session.tts_provider.as_deref().unwrap_or("-"),
            session.playback_provider.as_deref().unwrap_or("-"),
            session.command_stt_provider.as_deref().unwrap_or("-"),
            session.voice.as_deref().unwrap_or("-"),
            session.notes_count
        )));
        lines.push(Line::from(format!(
            "anchor={} listening={} playback_state={}",
            session.anchor, session.command_listening_active, session.playback_state
        )));
        lines.push(Line::from(format!("chunk: {}", session.chunk_text)));
    } else {
        lines.push(Line::from("".to_string()));
        lines.push(Line::from("No active session.".to_string()));
    }
    lines
}

fn render_documents(app: &App) -> Vec<ListItem<'static>> {
    if app.documents.is_empty() {
        return vec![ListItem::new("No ingested documents yet.")];
    }

    app.documents
        .iter()
        .map(|document| {
            ListItem::new(vec![Line::from(vec![
                Span::styled(
                    document.document_id.clone(),
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(format!(
                    "  {} ({} ch, {} chunks)",
                    document.title, document.chapter_count, document.chunk_count
                )),
            ])])
        })
        .collect()
}

fn render_messages(app: &App) -> Vec<Line<'static>> {
    if app.messages.is_empty() {
        return vec![Line::from("No messages yet.")];
    }
    app.messages
        .iter()
        .map(|message| Line::from(message.clone()))
        .collect()
}
