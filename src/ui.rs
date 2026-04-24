use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Scrollbar, ScrollbarOrientation, ScrollbarState, Table, Wrap};

use crate::app::{App, PanelMode};

const BG: Color = Color::Rgb(25, 26, 34);
const ACCENT: Color = Color::Rgb(156, 146, 201);
const ACCENT_SOFT: Color = Color::Rgb(115, 106, 155);
const TEXT: Color = Color::Rgb(198, 198, 210);
const MUTED: Color = Color::Rgb(120, 122, 138);
const PANEL: Color = Color::Rgb(35, 36, 48);
const BORDER: Color = Color::Rgb(34, 65, 64);

pub fn draw(frame: &mut Frame, app: &App) {
    let area = frame.area();

    frame.render_widget(
        Block::default().style(Style::default().bg(BG)),
        area,
    );

    if app.is_full_editor() {
        render_full_editor(frame, app, area);
        return;
    }

    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(8),
            Constraint::Length(4),
            Constraint::Length(1),
            Constraint::Length(3),
            Constraint::Min(0),
        ])
        .margin(1)
        .split(area);

    let logo = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(24), Constraint::Min(0)])
        .split(root[0]);

    let emblem = Paragraph::new(vec![
        Line::from(vec![Span::styled("⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⣀⢀⢀⢀⡀", Style::default().fg(MUTED))]),
        Line::from(vec![Span::styled("⠀⠀⠀⠀⠀⠀⠀⠀⢀⠀⡴⠰⠞⠿⠛⠁⠓⠖⠲⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀", Style::default().fg(TEXT))]),
        Line::from(vec![Span::styled("⠀⠀⠀⠀⠀⠀⠀⢸⠆⢁⠶⠿⠇⠹⠁⠸⠷⠏⣈⡀⢰⠀⠈⠀⠀⠀⠀⠀⠀⠀", Style::default().fg(TEXT))]),
        Line::from(vec![Span::styled("⠀⠀⠀⠀⠀⠀⡁⠴⠛⢀⡀⠀⠀⢀⠀⠀⠀⠀⡀⠀⠀⠂⠄⠀⠀⠀⠀⠀⠀⠀", Style::default().fg(ACCENT))]),
        Line::from(vec![Span::styled("⠀⠀⠀⠀⠀⠠⠀⢠⣴⣿⠀⠄⠈⠉⠀⠀⢀⠀⢻⡗⠀⠀⠐⠡⣄⡀⠀⠀⠀⠀", Style::default().fg(ACCENT_SOFT))]),
        Line::from(vec![Span::styled("⠀⠀⠀⠀⠀⣤⠒⢺⣿⣿⣆⠙⠄⢤⠠⠔⠘⢢⣞⠋⠀⢀⣰⣧⣬⡇⠀⠀⠀⠀", Style::default().fg(TEXT))]),
        Line::from(vec![Span::styled("⠀⠀⠀⠀⠈⠪⡅⠲⢿⢽⣿⣿⣶⣶⣦⣶⣿⠇⠴⠋⠍⢉⣹⣿⠿⠀⠀⠀⠀⠀", Style::default().fg(TEXT))]),
        Line::from(vec![Span::styled("⠀⠀⠀⠀⠀⠀⠰⠆⠁⠀⢈⠉⠹⣹⠈⠁⠀⠆⢰⢆⢀⣾⣾⠉⠀⠀⠀⠀⠀⠀", Style::default().fg(MUTED))]),
        Line::from(vec![Span::styled("⠀⠀⠀⠀⠀⠀⠀⠀⠃⠷⠀⠄⣤⡀⠀⣠⠠⣤⠄⠼⠟⠉⠀⠀⠀⠀⠀⠀⠀⠀", Style::default().fg(MUTED))]),
        Line::from(vec![Span::styled("⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠉⠉⠁⠈⠀", Style::default().fg(MUTED))]),
    ])
    .alignment(Alignment::Left);
    frame.render_widget(emblem, logo[0]);

    let version = Paragraph::new(Line::from(vec![Span::styled(
        "Aleph 0.1.0",
        Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
    )]));
    frame.render_widget(version, logo[1]);

    let title_block = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1), Constraint::Length(1)])
        .split(root[1]);

    let title = Paragraph::new(Line::from(vec![Span::styled(
        "Aleph",
        Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
    )]));
    frame.render_widget(title, title_block[0]);

    let subtitle = Paragraph::new(Line::from(vec![Span::styled(
        "terminal and agent runtime for Strix",
        Style::default().fg(MUTED),
    )]));
    frame.render_widget(subtitle, title_block[1]);

    let help = Paragraph::new(Line::from(vec![
        Span::styled("Tab", Style::default().fg(TEXT)),
        Span::raw(" to autocomplete, "),
        Span::styled("↑/↓", Style::default().fg(TEXT)),
        Span::raw(" cycle commands, "),
        Span::styled("Enter", Style::default().fg(TEXT)),
        Span::raw(" run selected command, "),
        Span::styled("/note edit", Style::default().fg(MUTED)),
        Span::raw(" opens the editor, "),
        Span::styled("Ctrl+C", Style::default().fg(TEXT)),
        Span::raw(" quit"),
    ]))
    .style(Style::default().fg(MUTED));
    frame.render_widget(help, title_block[2]);

    frame.render_widget(
        Block::default()
            .borders(Borders::TOP)
            .border_style(Style::default().fg(BORDER)),
        root[2],
    );

    let input_row = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(0), Constraint::Length(28)])
        .split(root[3]);

    let prompt_block = Paragraph::new(Line::from(vec![
        Span::styled(
            ">",
            Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(app.prompt_before_cursor(), Style::default().fg(TEXT)),
        Span::styled("█", Style::default().fg(MUTED)),
        Span::styled(app.prompt_after_cursor(), Style::default().fg(TEXT)),
    ]))
    .alignment(Alignment::Left)
    .block(Block::default().borders(Borders::BOTTOM).border_style(Style::default().fg(BORDER)));
    frame.render_widget(prompt_block, input_row[0]);

    let command_hint = if app.is_editing_note() {
        Paragraph::new(Line::from(vec![
            Span::styled("Ctrl+S", Style::default().fg(ACCENT)),
            Span::raw(" save "),
            Span::styled("Esc", Style::default().fg(ACCENT_SOFT)),
            Span::raw(" save & exit"),
        ]))
        .style(Style::default().fg(MUTED))
        .alignment(Alignment::Right)
    } else if app.is_thinking() {
        Paragraph::new(Line::from(vec![
            Span::styled(
                app.thinking_frame(),
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" focusing..."),
        ]))
        .style(Style::default().fg(MUTED))
        .alignment(Alignment::Right)
    } else {
        Paragraph::new(Line::from(vec![
            Span::styled("/note list", Style::default().fg(TEXT)),
            Span::raw(" "),
            Span::styled("/note edit", Style::default().fg(MUTED)),
        ]))
        .style(Style::default().fg(MUTED))
        .alignment(Alignment::Right)
    };
    frame.render_widget(command_hint, input_row[1]);

    match app.panel_mode() {
        PanelMode::Commands => render_commands_panel(frame, app, root[4]),
        PanelMode::NoteEditor => render_note_editor_panel(frame, app, root[4]),
        PanelMode::FullEditor => {},
    }
}

fn parse_markdown_spans(text: &str) -> Vec<Span<'_>> {
    let mut spans = Vec::new();
    let mut remaining = text;

    while !remaining.is_empty() {
        if let Some(pos) = remaining.find("**") {
            if pos > 0 {
                spans.push(Span::raw(&remaining[..pos]));
            }
            remaining = &remaining[pos + 2..];
            if let Some(end_pos) = remaining.find("**") {
                spans.push(Span::styled(&remaining[..end_pos], Style::default().add_modifier(Modifier::BOLD)));
                remaining = &remaining[end_pos + 2..];
            } else {
                spans.push(Span::raw("**"));
                spans.push(Span::raw(remaining));
                break;
            }
        } else if let Some(pos) = remaining.find('*') {
            if pos > 0 {
                spans.push(Span::raw(&remaining[..pos]));
            }
            remaining = &remaining[pos + 1..];
            if let Some(end_pos) = remaining.find('*') {
                spans.push(Span::styled(&remaining[..end_pos], Style::default().add_modifier(Modifier::ITALIC)));
                remaining = &remaining[end_pos + 1..];
            } else {
                spans.push(Span::raw("*"));
                spans.push(Span::raw(remaining));
                break;
            }
        } else if let Some(pos) = remaining.find('`') {
            if pos > 0 {
                spans.push(Span::raw(&remaining[..pos]));
            }
            remaining = &remaining[pos + 1..];
            if let Some(end_pos) = remaining.find('`') {
                spans.push(Span::styled(&remaining[..end_pos], Style::default().fg(ACCENT_SOFT)));
                remaining = &remaining[end_pos + 1..];
            } else {
                spans.push(Span::raw("`"));
                spans.push(Span::raw(remaining));
                break;
            }
        } else {
            spans.push(Span::raw(remaining));
            break;
        }
    }

    if spans.is_empty() {
        spans.push(Span::raw(text));
    }

    spans
}

fn render_markdown_line(line: &str) -> Line<'_> {
    let mut spans = Vec::new();
    let mut remaining = line;

    // Handle headers - H1 is underlined+bold, H2 is bold, H3 is italic+bold
    if line.starts_with("# ") {
        spans.push(Span::styled("# ", Style::default().fg(ACCENT_SOFT)));
        spans.push(Span::styled(&line[2..], Style::default().fg(TEXT).add_modifier(Modifier::BOLD | Modifier::UNDERLINED)));
        return Line::from(spans);
    } else if line.starts_with("## ") {
        spans.push(Span::styled("## ", Style::default().fg(MUTED)));
        spans.push(Span::styled(&line[3..], Style::default().fg(TEXT).add_modifier(Modifier::BOLD)));
        return Line::from(spans);
    } else if line.starts_with("### ") {
        spans.push(Span::styled("### ", Style::default().fg(MUTED)));
        spans.push(Span::styled(&line[4..], Style::default().fg(TEXT).add_modifier(Modifier::BOLD | Modifier::ITALIC)));
        return Line::from(spans);
    }

    // Handle inline formatting
    while !remaining.is_empty() {
        if let Some(pos) = remaining.find("**") {
            if pos > 0 {
                spans.push(Span::raw(&remaining[..pos]));
            }
            remaining = &remaining[pos + 2..];
            if let Some(end_pos) = remaining.find("**") {
                spans.push(Span::styled(&remaining[..end_pos], Style::default().add_modifier(Modifier::BOLD)));
                remaining = &remaining[end_pos + 2..];
            } else {
                spans.push(Span::raw("**"));
                spans.push(Span::raw(remaining));
                break;
            }
        } else if let Some(pos) = remaining.find('*') {
            if pos > 0 {
                spans.push(Span::raw(&remaining[..pos]));
            }
            remaining = &remaining[pos + 1..];
            if let Some(end_pos) = remaining.find('*') {
                spans.push(Span::styled(&remaining[..end_pos], Style::default().add_modifier(Modifier::ITALIC)));
                remaining = &remaining[end_pos + 1..];
            } else {
                spans.push(Span::raw("*"));
                spans.push(Span::raw(remaining));
                break;
            }
        } else if let Some(pos) = remaining.find('`') {
            if pos > 0 {
                spans.push(Span::raw(&remaining[..pos]));
            }
            remaining = &remaining[pos + 1..];
            if let Some(end_pos) = remaining.find('`') {
                spans.push(Span::styled(&remaining[..end_pos], Style::default().fg(ACCENT_SOFT)));
                remaining = &remaining[end_pos + 1..];
            } else {
                spans.push(Span::raw("`"));
                spans.push(Span::raw(remaining));
                break;
            }
        } else {
            spans.push(Span::raw(remaining));
            break;
        }
    }

    if spans.is_empty() {
        spans.push(Span::raw(line));
    }

    Line::from(spans)
}

fn render_full_editor(frame: &mut Frame, app: &App, area: Rect) {
    let main_chunks = if app.ai_sidepanel_visible() {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
            .margin(1)
            .split(area)
    } else {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(100)])
            .margin(1)
            .split(area)
    };

    let editor_area = main_chunks[0];

    let title = app.editor_note_title().unwrap_or("Untitled");

    let mut title_spans = vec![
        Span::styled(format!("Editing: {}", title), Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)),
    ];

    if app.search_state().active {
        title_spans.push(Span::raw("  "));
        title_spans.push(Span::styled(
            format!("Find: {}", app.search_state().query),
            Style::default().fg(ACCENT_SOFT).add_modifier(Modifier::BOLD),
        ));
        if let Some(current) = app.search_state().current_match {
            let total = app.search_state().matches.len();
            title_spans.push(Span::styled(
                format!(" ({}/{})", current + 1, total),
                Style::default().fg(MUTED),
            ));
        }
    }

    let block = Block::default()
        .title(Line::from(title_spans))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(BORDER));
    let inner = block.inner(editor_area);
    frame.render_widget(block, editor_area);

    let h_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(inner);

    let v_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(h_chunks[0]);

    let editor_content_area = v_chunks[0];

    let cursor = app.editor_cursor().min(app.editor_buffer().len());

    let mut lines: Vec<Line> = Vec::new();
    let mut char_pos = 0;

    for line_text in app.editor_buffer().lines() {
        let line_len = line_text.len();
        let line_start = char_pos;
        let line_end = char_pos + line_len;

        // Add visual spacing before H1 headers (except at start of document)
        if line_text.starts_with("# ") && line_start > 0 {
            lines.push(Line::from(""));
        }

        if cursor >= line_start && cursor <= line_end {
            let cursor_in_line = cursor - line_start;
            let before_cursor = &line_text[..cursor_in_line];
            let after_cursor = &line_text[cursor_in_line..];

            let mut spans = parse_markdown_spans(before_cursor);
            spans.push(Span::styled("█", Style::default().fg(TEXT)));
            spans.extend(parse_markdown_spans(after_cursor));

            lines.push(Line::from(spans));
        } else {
            lines.push(render_markdown_line(line_text));
        }

        // Add visual spacing after H1 headers
        if line_text.starts_with("# ") {
            lines.push(Line::from(""));
        }

        char_pos += line_len + 1;
    }

    // Calculate which line the cursor is on (for auto-scrolling)
    let cursor_line = app.editor_buffer()[..cursor.min(app.editor_buffer().len())]
        .chars()
        .filter(|&c| c == '\n')
        .count();

    let total_lines = app.editor_buffer().lines().count().max(1);
    let visible_lines = editor_content_area.height as usize;

    // Auto-scroll: ensure cursor is within visible viewport
    let mut effective_scroll_offset = app.editor_scroll_offset();
    if cursor_line < effective_scroll_offset {
        // Cursor above viewport - scroll up to show it
        effective_scroll_offset = cursor_line;
    } else if cursor_line >= effective_scroll_offset + visible_lines {
        // Cursor below viewport - scroll down to show it
        effective_scroll_offset = cursor_line.saturating_sub(visible_lines - 1);
    }
    // Clamp to valid range
    effective_scroll_offset = effective_scroll_offset.min(total_lines.saturating_sub(visible_lines));

    let paragraph = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .scroll((effective_scroll_offset as u16, 0));
    frame.render_widget(paragraph, editor_content_area);

    let scrollbar_needed = total_lines > visible_lines;

    if scrollbar_needed {
        let scrollbar = Scrollbar::default()
            .orientation(ScrollbarOrientation::VerticalRight)
            .begin_symbol(None)
            .end_symbol(None)
            .track_symbol(Some("│"))
            .thumb_symbol("█");
        let mut scroll_state = ScrollbarState::new(total_lines.saturating_sub(visible_lines))
            .position(effective_scroll_offset);
        frame.render_stateful_widget(scrollbar, h_chunks[1], &mut scroll_state);
    }

    let status = Paragraph::new(Line::from(vec![
        Span::styled("Ctrl+S", Style::default().fg(ACCENT)),
        Span::raw(" save  "),
        Span::styled("Esc", Style::default().fg(ACCENT_SOFT)),
        Span::raw(" exit  "),
        Span::styled("Ctrl+L", Style::default().fg(ACCENT)),
        Span::raw(" AI  "),
        Span::styled("Ctrl+Z", Style::default().fg(ACCENT)),
        Span::raw(" undo  "),
        Span::styled("Ctrl+F", Style::default().fg(ACCENT)),
        Span::raw(" find  "),
    ]))
    .style(Style::default().fg(MUTED));
    frame.render_widget(status, v_chunks[1]);

    if app.ai_sidepanel_visible() && main_chunks.len() > 1 {
        let ai_area = main_chunks[1];
        let ai_block = Block::default()
            .title(Span::styled(
                if app.ai_panel_focused() { "AI [FOCUSED]" } else { "AI" },
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
            ))
            .borders(Borders::ALL)
            .border_style(if app.ai_panel_focused() {
                Style::default().fg(ACCENT)
            } else {
                Style::default().fg(BORDER)
            });
        let ai_inner = ai_block.inner(ai_area);
        frame.render_widget(ai_block, ai_area);

        let ai_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(3), Constraint::Length(3)])
            .split(ai_inner);

        let chat_content = Paragraph::new(vec![
            Line::from(Span::styled("Ask anything", Style::default().fg(TEXT))),
            Line::from(""),
            Line::from(Span::styled(
                "AI assistant is ready.",
                Style::default().fg(MUTED),
            )),
        ])
        .wrap(Wrap { trim: false });
        frame.render_widget(chat_content, ai_layout[0]);

        let input_block = Block::default()
            .title(Span::styled(
                ">",
                Style::default().fg(if app.ai_panel_focused() { ACCENT } else { MUTED }),
            ))
            .borders(Borders::TOP)
            .border_style(Style::default().fg(BORDER));
        let input_inner = input_block.inner(ai_layout[1]);
        frame.render_widget(input_block, ai_layout[1]);

        let mut input_text = String::with_capacity(app.ai_input_buffer().len() + 1);
        let cursor = app.ai_input_cursor().min(app.ai_input_buffer().len());
        input_text.push_str(&app.ai_input_buffer()[..cursor]);
        input_text.push('█');
        input_text.push_str(&app.ai_input_buffer()[cursor..]);

        let input_para = Paragraph::new(input_text)
            .style(Style::default().fg(TEXT));
        frame.render_widget(input_para, input_inner);
    }
}

fn render_commands_panel(frame: &mut Frame, app: &App, area: Rect) {
    let has_status = !app.panel_lines().is_empty();
    let inner = if has_status {
        let block = Block::default()
            .title(Span::styled(
                "Commands",
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
            ))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(BORDER));
        let inner = block.inner(area);
        frame.render_widget(block, area);
        let parts = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(4), Constraint::Min(0)])
            .split(inner);

        let status_block = Paragraph::new(vec![
            Line::from(vec![Span::styled(
                app.panel_title(),
                Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
            )]),
            Line::from(vec![Span::styled(
                app.panel_lines().join(" "),
                Style::default().fg(MUTED),
            )]),
        ])
        .wrap(Wrap { trim: false });
        frame.render_widget(status_block, parts[0]);
        parts[1]
    } else {
        let block = Block::default()
            .title(Span::styled(
                "Commands",
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
            ))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(BORDER));
        let inner = block.inner(area);
        frame.render_widget(block, area);
        inner
    };

    let (suggestions, offset) = app.visible_commands_window(8);
    let total = app.total_command_matches();
    let remaining = total.saturating_sub(offset + suggestions.len());
    let selected_global = app.selected_suggestion();

    let rows = suggestions
        .iter()
        .enumerate()
        .map(|(index, command)| {
            let global_index = offset + index;
            let selected = global_index == selected_global;
            let row_style = if selected {
                Style::default()
                    .fg(TEXT)
                    .bg(PANEL)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Rgb(122, 122, 128))
            };

            Row::new(vec![
                Cell::from(Span::styled(App::command_label(command), row_style)),
                Cell::from(Span::styled((*command).description, row_style)),
            ])
        })
        .chain((remaining > 0).then(|| {
            Row::new(vec![
                Cell::from(Span::styled(
                    format!("+ {} more", remaining),
                    Style::default().fg(MUTED),
                )),
                Cell::from(Span::styled("", Style::default())),
            ])
        }))
        .collect::<Vec<_>>();

    let suggestions_table = Table::new(rows, [Constraint::Length(26), Constraint::Min(10)])
        .column_spacing(3)
        .style(Style::default().fg(Color::Rgb(122, 122, 128)));
    frame.render_widget(suggestions_table, inner);
}

fn render_note_editor_panel(frame: &mut Frame, app: &App, area: Rect) {
    let base_title = app
        .editor_note_title()
        .map(|note| format!("Editing: {}", note))
        .unwrap_or_else(|| String::from("Editing note"));

    let mut title_spans = vec![
        Span::styled(base_title, Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)),
    ];

    if app.search_state().active {
        title_spans.push(Span::raw("  "));
        title_spans.push(Span::styled(
            format!("Find: {}", app.search_state().query),
            Style::default().fg(ACCENT_SOFT).add_modifier(Modifier::BOLD),
        ));
        if let Some(current) = app.search_state().current_match {
            let total = app.search_state().matches.len();
            title_spans.push(Span::styled(
                format!(" ({}/{})", current + 1, total),
                Style::default().fg(MUTED),
            ));
        }
    }

    let block = Block::default()
        .title(Line::from(title_spans))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(BORDER));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let h_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(inner);

    let v_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0), Constraint::Length(1)])
        .split(h_chunks[0]);

    let helper = Paragraph::new(Line::from(vec![
        Span::styled("Ctrl+S", Style::default().fg(ACCENT)),
        Span::raw(" save, "),
        Span::styled("Esc", Style::default().fg(ACCENT_SOFT)),
        Span::raw(" exit, "),
        Span::styled("Ctrl+Z", Style::default().fg(ACCENT)),
        Span::raw(" undo, "),
        Span::styled("Ctrl+F", Style::default().fg(ACCENT)),
        Span::raw(" find"),
    ]))
    .style(Style::default().fg(MUTED));
    frame.render_widget(helper, v_chunks[0]);

    let editor_content_area = v_chunks[1];
    let cursor = app.editor_cursor().min(app.editor_buffer().len());

    let wrap_setting = if app.editor_word_wrap() {
        Wrap { trim: false }
    } else {
        Wrap { trim: true }
    };

    // Build lines with cursor inserted at correct position
    let mut lines: Vec<Line> = Vec::new();
    let mut char_pos = 0;

    for line_text in app.editor_buffer().lines() {
        let line_len = line_text.len();
        let line_start = char_pos;
        let line_end = char_pos + line_len;

        // Add visual spacing before H1 headers (except at start of document)
        if line_text.starts_with("# ") && line_start > 0 {
            lines.push(Line::from(""));
        }

        if cursor >= line_start && cursor <= line_end {
            // Cursor is on this line
            let cursor_in_line = cursor - line_start;
            let before_cursor = &line_text[..cursor_in_line];
            let after_cursor = &line_text[cursor_in_line..];

            let mut spans = parse_markdown_spans(before_cursor);
            spans.push(Span::styled("█", Style::default().fg(TEXT)));
            spans.extend(parse_markdown_spans(after_cursor));

            lines.push(Line::from(spans));
        } else {
            lines.push(render_markdown_line(line_text));
        }

        // Add visual spacing after H1 headers
        if line_text.starts_with("# ") {
            lines.push(Line::from(""));
        }

        char_pos += line_len + 1;
    }

    // Calculate which line the cursor is on (for auto-scrolling)
    let cursor_line = app.editor_buffer()[..cursor.min(app.editor_buffer().len())]
        .chars()
        .filter(|&c| c == '\n')
        .count();

    let total_lines = app.editor_buffer().lines().count().max(1);
    let visible_lines = editor_content_area.height as usize;

    // Auto-scroll: ensure cursor is within visible viewport
    let mut effective_scroll_offset = app.editor_scroll_offset();
    if cursor_line < effective_scroll_offset {
        // Cursor above viewport - scroll up to show it
        effective_scroll_offset = cursor_line;
    } else if cursor_line >= effective_scroll_offset + visible_lines {
        // Cursor below viewport - scroll down to show it
        effective_scroll_offset = cursor_line.saturating_sub(visible_lines - 1);
    }
    // Clamp to valid range
    effective_scroll_offset = effective_scroll_offset.min(total_lines.saturating_sub(visible_lines));

    let paragraph = Paragraph::new(lines)
        .wrap(wrap_setting)
        .scroll((effective_scroll_offset as u16, 0));
    frame.render_widget(paragraph, editor_content_area);

    let scrollbar_needed = total_lines > visible_lines;

    if scrollbar_needed {
        let scrollbar = Scrollbar::default()
            .orientation(ScrollbarOrientation::VerticalRight)
            .begin_symbol(None)
            .end_symbol(None)
            .track_symbol(Some("│"))
            .thumb_symbol("█");
        let mut scroll_state = ScrollbarState::new(total_lines.saturating_sub(visible_lines))
            .position(effective_scroll_offset);
        frame.render_stateful_widget(scrollbar, h_chunks[1], &mut scroll_state);
    }

    let status = Paragraph::new(Line::from(vec![
        Span::styled("PgUp/Dn", Style::default().fg(ACCENT)),
        Span::raw(" scroll, "),
        Span::styled("Ctrl+W", Style::default().fg(ACCENT)),
        Span::raw(" wrap, "),
        Span::styled("Ctrl+B", Style::default().fg(ACCENT)),
        Span::raw(" cursor"),
    ]))
    .style(Style::default().fg(MUTED))
    .alignment(Alignment::Right);
    frame.render_widget(status, v_chunks[2]);
}