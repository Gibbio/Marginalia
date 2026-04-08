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
use ratatui::layout::Alignment;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph, Wrap};
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
    let version = env!("CARGO_PKG_VERSION");
    let border_style = Style::default().fg(Color::Rgb(140, 160, 180));
    let title_style = Style::default()
        .fg(Color::Rgb(180, 200, 220))
        .add_modifier(Modifier::BOLD);
    let section_style = Style::default().fg(Color::Rgb(102, 118, 136));

    let shell = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(border_style)
        .title_alignment(Alignment::Left)
        .title(shell_title(app, border_style, title_style, section_style));
    let inner = shell.inner(frame.area());
    frame.render_widget(shell, frame.area());

    if inner.width < 12 || inner.height < 12 {
        return;
    }

    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(6),
            Constraint::Length(1),
            Constraint::Min(12),
            Constraint::Length(1),
            Constraint::Length(6),
        ])
        .split(inner);
    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(72),
            Constraint::Length(3),
            Constraint::Percentage(28),
        ])
        .split(vertical[2]);
    let sidebar = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(70),
            Constraint::Length(1),
            Constraint::Percentage(30),
        ])
        .split(body[2]);

    let overview_title = format!("Marginalia tui-rs {version}");
    let header = Paragraph::new(section_lines(
        &overview_title,
        render_header_lines(app, vertical[0].width),
        vertical[0].width,
        title_style,
        section_style,
    ));

    let document_area = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(body[0]);
    let (document_lines, active_document_line) =
        render_document_lines(app, document_area[1].width.saturating_sub(2).max(1) as usize);
    let document_heading = Paragraph::new(section_heading(
        "Document",
        document_area[0].width,
        title_style,
        section_style,
    ));
    let document_lines = indented_lines(document_lines, "  ");
    sync_document_scroll(app, document_area[1], active_document_line, document_lines.len());
    let document = Paragraph::new(document_lines)
        .scroll((app.document_scroll(), 0))
        .wrap(Wrap { trim: false });

    let log_area = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(sidebar[0]);
    let log_heading = Paragraph::new(section_heading(
        "Log",
        log_area[0].width,
        title_style,
        section_style,
    ));
    let messages_widget = Paragraph::new(indented_lines(render_messages(app), "  "))
        .wrap(Wrap { trim: true });
    let messages_viewport = log_area[1].height;
    let messages_total = messages_widget.line_count(log_area[1].width) as u16;
    let messages = messages_widget.scroll((messages_total.saturating_sub(messages_viewport), 0));

    let status = Paragraph::new(section_lines(
        "Session",
        render_status_lines(app),
        sidebar[2].width,
        title_style,
        section_style,
    ))
    .wrap(Wrap { trim: true });

    let input = Paragraph::new(section_lines(
        "Command",
        render_input_lines(app),
        vertical[4].width,
        title_style,
        section_style,
    ))
        .style(Style::default().fg(Color::Yellow));

    frame.render_widget(header, vertical[0]);
    frame.render_widget(document_heading, document_area[0]);
    frame.render_widget(document, document_area[1]);
    render_vertical_separator(frame, body[1], section_style);
    frame.render_widget(log_heading, log_area[0]);
    frame.render_widget(messages, log_area[1]);
    frame.render_widget(status, sidebar[2]);
    frame.render_widget(input, vertical[4]);
    frame.set_cursor_position((
        vertical[4].x + 4 + app.input.chars().count() as u16,
        vertical[4].y + 1,
    ));
}

fn sync_document_scroll(
    app: &mut App,
    document_area: Rect,
    active_document_line: Option<usize>,
    total_lines: usize,
) {
    let viewport_height = document_area.height;
    app.sync_document_scroll(
        active_document_line,
        viewport_height.max(1),
        total_lines.max(1),
    );
}

fn shell_title(
    app: &App,
    border_style: Style,
    active_style: Style,
    idle_style: Style,
) -> Line<'static> {
    let phase = (app.animation_tick() / 5) % 3;
    let mut spans = vec![Span::styled("─── ", border_style)];
    for index in 0..3 {
        let style = if index == phase { active_style } else { idle_style };
        let glyph = if index == phase { "•" } else { "·" };
        spans.push(Span::styled(glyph, style));
        if index < 2 {
            spans.push(Span::raw(" "));
        }
    }
    spans.push(Span::styled(" ───", border_style));
    Line::from(spans)
}

fn section_lines(
    title: &str,
    lines: Vec<Line<'static>>,
    width: u16,
    title_style: Style,
    separator_style: Style,
) -> Vec<Line<'static>> {
    let mut result = Vec::with_capacity(lines.len() + 1);
    result.push(section_heading(
        title,
        width,
        title_style,
        separator_style,
    ));
    result.extend(indented_lines(lines, "  "));
    result
}

fn indented_lines(lines: Vec<Line<'static>>, indent: &str) -> Vec<Line<'static>> {
    let mut result = Vec::with_capacity(lines.len());
    for line in lines {
        let mut spans = Vec::with_capacity(line.spans.len() + 1);
        spans.push(Span::raw(indent.to_string()));
        spans.extend(line.spans);
        result.push(Line::from(spans));
    }
    result
}

fn section_heading(
    title: &str,
    width: u16,
    title_style: Style,
    separator_style: Style,
) -> Line<'static> {
    let separator_width = width
        .saturating_sub(title.chars().count() as u16)
        .saturating_sub(3) as usize;

    Line::from(vec![
        Span::raw("  "),
        Span::styled(title.to_string(), title_style),
        Span::styled(format!(" {}", "─".repeat(separator_width)), separator_style),
    ])
}

fn render_vertical_separator(frame: &mut Frame, area: Rect, style: Style) {
    if area.width == 0 || area.height == 0 {
        return;
    }

    let lines = (0..area.height)
        .map(|_| Line::from(Span::styled(" │ ", style)))
        .collect::<Vec<_>>();
    frame.render_widget(Paragraph::new(lines), area);
}

fn render_header_lines(app: &App, width: u16) -> Vec<Line<'static>> {
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

    let content_width = width.saturating_sub(2) as usize;
    let style = Style::default()
        .fg(Color::Green)
        .add_modifier(Modifier::BOLD);

    dinosaur
        .iter()
        .map(|line| {
            Line::from(Span::styled(
                wrap_sprite_line(line, app.animation_tick(), content_width),
                style,
            ))
        })
        .collect()
}

fn wrap_sprite_line(line: &str, offset: usize, width: usize) -> String {
    if width == 0 {
        return String::new();
    }

    let mut cells = vec![' '; width];
    for (index, ch) in line.chars().enumerate() {
        let position = (offset + index) % width;
        cells[position] = ch;
    }
    cells.into_iter().collect()
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
