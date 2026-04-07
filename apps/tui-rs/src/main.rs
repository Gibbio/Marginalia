mod app;
mod backend;

use app::App;
use backend::BackendClient;
use crossterm::cursor::{DisableBlinking, EnableBlinking};
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
    execute!(stdout, EnterAlternateScreen, EnableBlinking)
        .map_err(|err| format!("Unable to enter alternate screen: {err}"))?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)
        .map_err(|err| format!("Unable to create terminal backend: {err}"))?;
    terminal
        .show_cursor()
        .map_err(|err| format!("Unable to show terminal cursor: {err}"))?;

    let result = run_tui(&mut terminal, &mut app);

    disable_raw_mode().map_err(|err| format!("Unable to disable raw mode: {err}"))?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableBlinking
    )
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
                        app.handle_ctrl_c();
                    }
                    KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        app.select_previous_history();
                    }
                    KeyCode::Char('n') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        app.select_next_history();
                    }
                    KeyCode::Enter => app.confirm_input(),
                    KeyCode::Up if app.command_input_is_empty() => app.navigate_previous_chunk(),
                    KeyCode::Down if app.command_input_is_empty() => app.navigate_next_chunk(),
                    KeyCode::Left if app.command_input_is_empty() => {
                        app.navigate_previous_chapter();
                    }
                    KeyCode::Right if app.command_input_is_empty() => app.navigate_next_chapter(),
                    KeyCode::Up => app.select_previous_suggestion(),
                    KeyCode::Down => app.select_next_suggestion(),
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
            Constraint::Length(9),
            Constraint::Min(12),
            Constraint::Length(8),
            Constraint::Length(7),
        ])
        .split(frame.area());
    let middle = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(68), Constraint::Percentage(32)])
        .split(vertical[1]);
    let lower = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(68), Constraint::Percentage(32)])
        .split(vertical[2]);

    let header = Paragraph::new(render_header_lines(app))
        .block(Block::default().borders(Borders::ALL).title("Overview"))
        .wrap(Wrap { trim: true });

    let document = Paragraph::new(render_document_lines(app))
        .block(Block::default().borders(Borders::ALL).title("Document"))
        .wrap(Wrap { trim: true });

    let documents = List::new(render_documents(app))
        .block(Block::default().borders(Borders::ALL).title("Documents"));

    let messages = Paragraph::new(render_messages(app))
        .block(Block::default().borders(Borders::ALL).title("Log"))
        .wrap(Wrap { trim: true });

    let status = Paragraph::new(render_status_lines(app))
        .block(Block::default().borders(Borders::ALL).title("Session"))
        .wrap(Wrap { trim: true });

    let input = Paragraph::new(render_input_lines(app))
        .block(Block::default().borders(Borders::ALL).title("Command"))
        .style(Style::default().fg(Color::Yellow));

    frame.render_widget(header, vertical[0]);
    frame.render_widget(document, middle[0]);
    frame.render_widget(documents, middle[1]);
    frame.render_widget(messages, lower[0]);
    frame.render_widget(status, lower[1]);
    frame.render_widget(input, vertical[3]);
    frame.set_cursor_position((
        vertical[3].x + 3 + app.input.chars().count() as u16,
        vertical[3].y + 1,
    ));
}

fn render_header_lines(app: &App) -> Vec<Line<'static>> {
    let dinosaur = match app.animation_frame() {
        0 => [
            "           __",
            "          / _)",
            "   .-^^^-/ /  ",
            "__/       /   ",
            "<__.|_|-|_|   ",
        ],
        1 => [
            "           __",
            "          / _)",
            "   .-^^^-/ /  ",
            "__/       /   ",
            "<__.|_|-|-|   ",
        ],
        _ => [
            "           __",
            "          / _)",
            "   .-^^^-/ /  ",
            "__/       /   ",
            "<__.|-|_|_|   ",
        ],
    };

    vec![
        Line::from(vec![
            Span::styled(
                dinosaur[0].to_string(),
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("      "),
            Span::styled(
                "M",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "A",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "R",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "G",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "I",
                Style::default()
                    .fg(Color::Blue)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "N",
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "A",
                Style::default()
                    .fg(Color::LightRed)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "L",
                Style::default()
                    .fg(Color::LightYellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "I",
                Style::default()
                    .fg(Color::LightGreen)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "A",
                Style::default()
                    .fg(Color::LightCyan)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(Span::styled(
            dinosaur[1].to_string(),
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(vec![
            Span::styled(
                dinosaur[2].to_string(),
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("   "),
            Span::styled(
                "Rust TUI over the headless backend",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(Span::styled(
            dinosaur[3].to_string(),
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(vec![
            Span::styled(
                dinosaur[4].to_string(),
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("   "),
            Span::styled(
                "Type /ingest to wake the reader",
                Style::default().fg(Color::Gray),
            ),
        ]),
    ]
}

fn render_input_lines(app: &App) -> Vec<Line<'static>> {
    let mut first_line = vec![
        Span::styled("> ", Style::default().fg(Color::Yellow)),
        Span::styled(
            app.input.clone(),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
    ];
    if let Some(suffix) = app.completion_suffix() {
        first_line.push(Span::styled(
            suffix.to_string(),
            Style::default().fg(Color::DarkGray),
        ));
    }

    let mut lines = vec![
        Line::from(first_line),
        Line::from(Span::styled(
            app.command_hint(),
            Style::default().fg(Color::Gray),
        )),
    ];

    let selected_slot = app.selected_suggestion_slot();
    for (index, suggestion) in app.visible_suggestions().into_iter().enumerate() {
        let prefix = if Some(index) == selected_slot {
            "› "
        } else {
            "  "
        };
        let style = if Some(index) == selected_slot {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        lines.push(Line::from(vec![
            Span::styled(prefix, style),
            Span::styled(format!("{:<18}", suggestion.label), style),
            Span::styled(suggestion.summary, Style::default().fg(Color::Gray)),
        ]));
    }

    lines
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

fn render_document_lines(app: &App) -> Vec<Line<'static>> {
    let Some(document) = &app.document_view else {
        return vec![
            Line::from("No ingested document selected.".to_string()),
            Line::from("Use /ingest <path> to load a markdown file.".to_string()),
        ];
    };

    let mut lines = vec![
        Line::from(Span::styled(
            document.title.clone(),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(format!(
            "{}  id={}  {} chapters  {} chunks",
            document.source_path,
            document.document_id,
            document.chapter_count,
            document.chunk_count
        )),
        Line::from("".to_string()),
    ];

    for section in &document.sections {
        let section_style = if Some(section.index) == document.active_section_index {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };
        lines.push(Line::from(Span::styled(
            format!("{}. {}", section.index + 1, section.title),
            section_style,
        )));

        for chunk in section.chunks.iter().take(2) {
            let marker = if chunk.is_active {
                ">"
            } else if chunk.is_read {
                "-"
            } else {
                " "
            };
            lines.push(Line::from(vec![
                Span::styled(
                    format!("{} ", marker),
                    if chunk.is_active {
                        Style::default().fg(Color::Yellow)
                    } else {
                        Style::default().fg(Color::DarkGray)
                    },
                ),
                Span::styled(
                    format!("[{}:{}] ", section.index + 1, chunk.index + 1),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(
                    chunk.text.clone(),
                    if chunk.is_active {
                        Style::default().fg(Color::Yellow)
                    } else {
                        Style::default().fg(Color::Gray)
                    },
                ),
            ]));
            if chunk.is_active {
                lines.push(Line::from(Span::styled(
                    format!(
                        "  anchor={} current_chunk={}",
                        chunk.anchor,
                        chunk.index + 1
                    ),
                    Style::default().fg(Color::DarkGray),
                )));
            }
        }
        if section.chunk_count > 2 {
            lines.push(Line::from(Span::styled(
                format!("  ... {} more chunks", section.chunk_count - 2),
                Style::default().fg(Color::DarkGray),
            )));
        }
        if Some(section.index) == document.active_section_index {
            if let Some(active_chunk_index) = document.active_chunk_index {
                lines.push(Line::from(Span::styled(
                    format!("  active chunk in section: {}", active_chunk_index + 1),
                    Style::default().fg(Color::DarkGray),
                )));
            }
        }
        lines.push(Line::from("".to_string()));
    }

    lines
}

fn render_documents(app: &App) -> Vec<ListItem<'static>> {
    if app.local_markdown_files.is_empty() {
        return vec![ListItem::new(
            "No .md files in the current launch directory.",
        )];
    }

    app.local_markdown_files
        .iter()
        .map(|file| {
            ListItem::new(vec![Line::from(vec![
                Span::styled(
                    file.name.clone(),
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(format!("  {}", file.path.display())),
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
