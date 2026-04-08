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
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
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
        app.poll_backend_logs();
        terminal
            .draw(|frame| render(frame, app))
            .map_err(|err| format!("Unable to draw terminal frame: {err}"))?;

        app.flush_pending_play();

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
                    KeyCode::PageUp if app.command_input_is_empty() => app.scroll_document_up(8),
                    KeyCode::PageDown if app.command_input_is_empty() => {
                        app.scroll_document_down(8);
                    }
                    KeyCode::Home if app.command_input_is_empty() => app.scroll_document_to_top(),
                    KeyCode::End if app.command_input_is_empty() => {
                        app.scroll_document_to_bottom();
                    }
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

fn render(frame: &mut Frame, app: &mut App) {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(9),
            Constraint::Min(18),
            Constraint::Length(7),
        ])
        .split(frame.area());
    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(72), Constraint::Percentage(28)])
        .split(vertical[1]);
    let sidebar = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(body[1]);

    let border_style = Style::default().fg(Color::Rgb(140, 160, 180));
    let title_style = Style::default()
        .fg(Color::Rgb(180, 200, 220))
        .add_modifier(Modifier::BOLD);

    let header = Paragraph::new(render_header_lines(app))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(border_style)
                .title(" Overview ")
                .title_style(title_style),
        )
        .wrap(Wrap { trim: false });

    let (document_lines, active_document_line) =
        render_document_lines(app, body[0].width.saturating_sub(2).max(1) as usize);
    sync_document_scroll(app, body[0], active_document_line, document_lines.len());
    let document = Paragraph::new(document_lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(border_style)
                .title(" Document ")
                .title_style(title_style),
        )
        .scroll((app.document_scroll(), 0))
        .wrap(Wrap { trim: false });

    let messages_widget = Paragraph::new(render_messages(app))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(border_style)
                .title(" Log ")
                .title_style(title_style),
        )
        .wrap(Wrap { trim: true });
    let messages_viewport = sidebar[0].height.saturating_sub(2);
    let messages_total = messages_widget.line_count(sidebar[0].width.saturating_sub(2)) as u16;
    let messages = messages_widget.scroll((messages_total.saturating_sub(messages_viewport), 0));

    let status = Paragraph::new(render_status_lines(app))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(border_style)
                .title(" Session ")
                .title_style(title_style),
        )
        .wrap(Wrap { trim: true });

    let input = Paragraph::new(render_input_lines(app))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(border_style)
                .title(" Command ")
                .title_style(title_style),
        )
        .style(Style::default().fg(Color::Yellow));

    frame.render_widget(header, vertical[0]);
    frame.render_widget(document, body[0]);
    frame.render_widget(messages, sidebar[0]);
    frame.render_widget(status, sidebar[1]);
    frame.render_widget(input, vertical[2]);
    frame.set_cursor_position((
        vertical[2].x + 3 + app.input.chars().count() as u16,
        vertical[2].y + 1,
    ));
}

fn sync_document_scroll(
    app: &mut App,
    document_area: Rect,
    active_document_line: Option<usize>,
    total_lines: usize,
) {
    let viewport_height = document_area.height.saturating_sub(2);
    app.sync_document_scroll(
        active_document_line,
        viewport_height.max(1),
        total_lines.max(1),
    );
}

fn render_header_lines(app: &App) -> Vec<Line<'static>> {
    let dinosaur = match app.animation_frame() {
        0 => [
            "            __",
            "           / _)",
            "    _.----./ / ",
            " __/         / ",
            "<__.-'|_|-|_|  ",
        ],
        1 => [
            "            __",
            "           / _)",
            "    _.----./ / ",
            " __/         / ",
            "<__.-'|_|-|-|  ",
        ],
        _ => [
            "            __",
            "           / _)",
            "    _.----./ / ",
            " __/         / ",
            "<__.-'|-|_|_|  ",
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

fn render_document_lines(app: &App, content_width: usize) -> (Vec<Line<'static>>, Option<usize>) {
    let Some(document) = &app.document_view else {
        return (
            vec![
                Line::from("No ingested document selected.".to_string()),
                Line::from("Use /ingest <path> to load a markdown file.".to_string()),
            ],
            None,
        );
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
    let mut active_line_index = None;

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

        for chunk in &section.chunks {
            let marker = if chunk.is_active {
                ">"
            } else if chunk.is_read {
                "-"
            } else {
                " "
            };
            let chunk_style = if chunk.is_active {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else if chunk.is_read {
                Style::default().fg(Color::DarkGray)
            } else {
                Style::default().fg(Color::Gray)
            };
            let chunk_text = chunk.text.replace('\n', " ");
            let first_prefix = format!("{} [{}:{}] ", marker, section.index + 1, chunk.index + 1);
            let continuation_prefix = " ".repeat(first_prefix.chars().count());
            let wrapped_chunk_lines = wrap_prefixed_text(&first_prefix, &chunk_text, content_width);
            if chunk.is_active {
                active_line_index = Some(lines.len());
            }
            for (index, wrapped_line) in wrapped_chunk_lines.into_iter().enumerate() {
                if index == 0 {
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
                        Span::styled(wrapped_line, chunk_style),
                    ]));
                } else {
                    lines.push(Line::from(vec![
                        Span::styled(
                            continuation_prefix.clone(),
                            Style::default().fg(Color::DarkGray),
                        ),
                        Span::styled(wrapped_line, chunk_style),
                    ]));
                }
            }
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

    (lines, active_line_index)
}

fn render_messages(app: &App) -> Vec<Line<'static>> {
    if app.messages.is_empty() {
        return vec![Line::from("No messages yet.")];
    }
    let total = app.messages.len();
    let start = total.saturating_sub(64);
    app.messages
        .iter()
        .skip(start)
        .map(|message| Line::from(message.clone()))
        .collect()
}

fn wrap_prefixed_text(prefix: &str, text: &str, width: usize) -> Vec<String> {
    let available_width = width.saturating_sub(prefix.chars().count()).max(1);
    wrap_text(text, available_width, available_width)
}

fn wrap_text(text: &str, first_width: usize, continuation_width: usize) -> Vec<String> {
    if text.is_empty() {
        return vec![String::new()];
    }

    let mut result = Vec::new();
    let mut current = String::new();
    let mut current_width = 0usize;
    let mut limit = first_width.max(1);

    for word in text.split_whitespace() {
        let word_width = word.chars().count();
        let separator_width = if current.is_empty() { 0 } else { 1 };
        if current_width + separator_width + word_width <= limit {
            if !current.is_empty() {
                current.push(' ');
                current_width += 1;
            }
            current.push_str(word);
            current_width += word_width;
            continue;
        }

        if !current.is_empty() {
            result.push(current);
            current = String::new();
            limit = continuation_width.max(1);
        }

        if word_width <= limit {
            current.push_str(word);
            current_width = word_width;
            continue;
        }

        let mut partial = String::new();
        let mut partial_width = 0usize;
        for ch in word.chars() {
            if partial_width >= limit {
                result.push(partial);
                partial = String::new();
                partial_width = 0;
                limit = continuation_width.max(1);
            }
            partial.push(ch);
            partial_width += 1;
        }
        current = partial;
        current_width = partial_width;
    }

    if !current.is_empty() {
        result.push(current);
    }

    if result.is_empty() {
        vec![String::new()]
    } else {
        result
    }
}
