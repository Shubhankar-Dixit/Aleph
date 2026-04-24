use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Scrollbar, ScrollbarOrientation, ScrollbarState, Table, Wrap};

use crate::app::{App, CursorStyle, PanelMode};

const BG: Color = Color::Rgb(25, 26, 34);
const ACCENT: Color = Color::Rgb(156, 146, 201);
const ACCENT_SOFT: Color = Color::Rgb(115, 106, 155);
const TEXT: Color = Color::Rgb(198, 198, 210);
const MUTED: Color = Color::Rgb(120, 122, 138);
const PANEL: Color = Color::Rgb(35, 36, 48);
const BORDER: Color = Color::Rgb(34, 65, 64);
const GHOST_FRAMES: [&str; 4] = ["◌", "◎", "◍", "◉"];
const CURSOR: &str = "│";

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
    
    if app.is_ai_chat() {
        render_full_chat(frame, app, area);
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
        Span::styled(CURSOR, Style::default().fg(MUTED)),
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
            Span::styled("/login", Style::default().fg(TEXT)),
            Span::raw(" "),
            Span::styled("/obsidian pair", Style::default().fg(MUTED)),
        ]))
        .style(Style::default().fg(MUTED))
        .alignment(Alignment::Right)
    };
    frame.render_widget(command_hint, input_row[1]);

    match app.panel_mode() {
        PanelMode::Commands => render_commands_panel(frame, app, root[4]),
        PanelMode::NoteEditor => render_note_editor_panel(frame, app, root[4]),
        PanelMode::FullEditor | PanelMode::AiChat => {},
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
    let trimmed = line.trim_start();
    let indent_len = line.len() - trimmed.len();

    if trimmed.starts_with("# ") {
        spans.push(Span::styled(&line[..indent_len + 2], Style::default().fg(ACCENT_SOFT)));
        spans.push(Span::styled(&trimmed[2..], Style::default().fg(TEXT).add_modifier(Modifier::BOLD | Modifier::UNDERLINED)));
        return Line::from(spans);
    } else if trimmed.starts_with("## ") {
        spans.push(Span::styled(&line[..indent_len + 3], Style::default().fg(MUTED)));
        spans.push(Span::styled(&trimmed[3..], Style::default().fg(TEXT).add_modifier(Modifier::BOLD)));
        return Line::from(spans);
    } else if trimmed.starts_with("### ") {
        spans.push(Span::styled(&line[..indent_len + 4], Style::default().fg(MUTED)));
        spans.push(Span::styled(&trimmed[4..], Style::default().fg(TEXT).add_modifier(Modifier::BOLD | Modifier::ITALIC)));
        return Line::from(spans);
    } else if let Some(stripped) = trimmed.strip_prefix("- ") {
        if indent_len > 0 {
            spans.push(Span::raw(&line[..indent_len]));
        }
        spans.push(Span::styled("• ", Style::default().fg(ACCENT)));
        remaining = stripped;
    } else if let Some(stripped) = trimmed.strip_prefix("* ") {
        if indent_len > 0 {
            spans.push(Span::raw(&line[..indent_len]));
        }
        spans.push(Span::styled("• ", Style::default().fg(ACCENT)));
        remaining = stripped;
    } else if let Some(pos) = trimmed.find(". ") {
        if pos > 0 && trimmed[..pos].chars().all(|c| c.is_ascii_digit()) {
            if indent_len > 0 {
                spans.push(Span::raw(&line[..indent_len]));
            }
            spans.push(Span::styled(&trimmed[..=pos + 1], Style::default().fg(ACCENT)));
            remaining = &trimmed[pos + 2..];
        }
    }

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

fn render_markdown_line_with_cursor(line: &str, cursor_in_line: usize, cursor_style: CursorStyle) -> Line<'_> {
    let cursor_glyph = if cursor_style == CursorStyle::Block {
        "█"
    } else {
        CURSOR
    };

    let mut spans = Vec::new();
    let trimmed = line.trim_start();
    let indent_len = line.len() - trimmed.len();
    
    let mut text_start = 0;

    if trimmed.starts_with("# ") {
        text_start = indent_len + 2;
    } else if trimmed.starts_with("## ") {
        text_start = indent_len + 3;
    } else if trimmed.starts_with("### ") {
        text_start = indent_len + 4;
    } else if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
        text_start = indent_len + 2;
    } else if let Some(pos) = trimmed.find(". ") {
        if pos > 0 && trimmed[..pos].chars().all(|c| c.is_ascii_digit()) {
            text_start = indent_len + pos + 2;
        }
    }

    if cursor_in_line < text_start {
        // Cursor is within the prefix. Render it simply to avoid complex prefix splitting.
        let mut before_cursor = parse_markdown_spans(&line[..cursor_in_line]);
        before_cursor.push(Span::styled(cursor_glyph, Style::default().fg(TEXT)));
        before_cursor.extend(parse_markdown_spans(&line[cursor_in_line..]));
        return Line::from(before_cursor);
    }

    // Apply prefix styling up to text_start
    let mut remaining = line;
    let mut prefix_spans = Vec::new();

    if text_start > 0 {
        if trimmed.starts_with("# ") {
            prefix_spans.push(Span::styled(&line[..text_start], Style::default().fg(ACCENT_SOFT)));
        } else if trimmed.starts_with("## ") || trimmed.starts_with("### ") {
            prefix_spans.push(Span::styled(&line[..text_start], Style::default().fg(MUTED)));
        } else if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
            if indent_len > 0 {
                prefix_spans.push(Span::raw(&line[..indent_len]));
            }
            prefix_spans.push(Span::styled("• ", Style::default().fg(ACCENT)));
        } else if let Some(pos) = trimmed.find(". ") {
            if indent_len > 0 {
                prefix_spans.push(Span::raw(&line[..indent_len]));
            }
            prefix_spans.push(Span::styled(&trimmed[..=pos + 1], Style::default().fg(ACCENT)));
        }
        remaining = &line[text_start..];
    }

    // Normal markdown parsing for the rest, injecting the cursor
    let adjusted_cursor = cursor_in_line - text_start;
    spans.extend(prefix_spans);

    let before_cursor = &remaining[..adjusted_cursor];
    let after_cursor = &remaining[adjusted_cursor..];

    if trimmed.starts_with("# ") || trimmed.starts_with("## ") || trimmed.starts_with("### ") {
        spans.push(Span::styled(before_cursor, Style::default().fg(TEXT).add_modifier(Modifier::BOLD)));
        spans.push(Span::styled(cursor_glyph, Style::default().fg(TEXT)));
        spans.push(Span::styled(after_cursor, Style::default().fg(TEXT).add_modifier(Modifier::BOLD)));
    } else {
        spans.extend(parse_markdown_spans(before_cursor));
        spans.push(Span::styled(cursor_glyph, Style::default().fg(TEXT)));
        spans.extend(parse_markdown_spans(after_cursor));
    }

    Line::from(spans)
}

fn render_full_editor(frame: &mut Frame, app: &App, area: Rect) {
    let max_width = 80;
    let center_width = area.width.saturating_sub(8).min(max_width);
    let left_padding = area.width.saturating_sub(center_width) / 2;

    let v_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Top meta
            Constraint::Length(1), // Spacer
            Constraint::Min(0),    // Editor content
            Constraint::Length(1), // Spacer
            Constraint::Length(1), // Bottom hints
        ])
        .margin(1)
        .split(area);

    let h_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(left_padding),
            Constraint::Length(center_width),
            Constraint::Min(0),
        ])
        .split(v_chunks[2]);

    let editor_content_area = h_chunks[1];

    let title = app.editor_note_title().unwrap_or("Untitled");
    let word_count = editor_word_count(app.editor_buffer());

    let meta_style = if app.save_shimmer_ticks() > 0 {
        Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(MUTED)
    };

    let mut meta_spans = vec![Span::styled(
        format!("{} / Draft / {} words", title, word_count),
        meta_style,
    )];

    if app.search_state().active {
        meta_spans.push(Span::raw("  "));
        meta_spans.push(Span::styled(
            format!("Find: {}", app.search_state().query),
            Style::default().fg(ACCENT_SOFT).add_modifier(Modifier::BOLD),
        ));
        if let Some(current) = app.search_state().current_match {
            let total = app.search_state().matches.len();
            meta_spans.push(Span::styled(
                format!(" ({}/{})", current + 1, total),
                Style::default().fg(MUTED),
            ));
        }
    }

    let top_meta = Paragraph::new(Line::from(meta_spans)).alignment(Alignment::Left);
    frame.render_widget(top_meta, v_chunks[0]);

    let cursor = app.editor_cursor().min(app.editor_buffer().len());

    let wrap_setting = if app.editor_word_wrap() {
        Wrap { trim: false }
    } else {
        Wrap { trim: true }
    };

    let mut lines: Vec<Line> = Vec::new();
    let mut char_pos = 0;
    let mut cursor_visual_row: Option<u16> = None;
    let mut cursor_visual_col: u16 = 0;
    let mut visual_row: u16 = 0;

    for line_text in app.editor_buffer().lines() {
        let line_len = line_text.len();
        let line_start = char_pos;
        let line_end = char_pos + line_len;

        if line_text.starts_with("# ") && line_start > 0 {
            lines.push(Line::from(""));
            visual_row = visual_row.saturating_add(1);
        }

        if cursor >= line_start && cursor <= line_end {
            let cursor_in_line = cursor - line_start;
            cursor_visual_row = Some(visual_row);
            cursor_visual_col = line_text[..cursor_in_line].chars().count() as u16;
            
            lines.push(render_markdown_line_with_cursor(line_text, cursor_in_line, app.editor_cursor_style()));
        } else {
            lines.push(render_markdown_line(line_text));
        }
        visual_row = visual_row.saturating_add(1);

        if line_text.starts_with("# ") {
            lines.push(Line::from(""));
            visual_row = visual_row.saturating_add(1);
        }

        char_pos += line_len + 1;
    }

    let cursor_line = app.editor_buffer()[..cursor.min(app.editor_buffer().len())]
        .chars()
        .filter(|&c| c == '\n')
        .count();

    let total_lines = app.editor_buffer().lines().count().max(1);
    let visible_lines = editor_content_area.height as usize;

    let mut effective_scroll_offset = app.editor_scroll_offset();
    if cursor_line < effective_scroll_offset {
        effective_scroll_offset = cursor_line;
    } else if cursor_line >= effective_scroll_offset + visible_lines {
        effective_scroll_offset = cursor_line.saturating_sub(visible_lines - 1);
    }
    effective_scroll_offset = effective_scroll_offset.min(total_lines.saturating_sub(visible_lines));

    let paragraph = Paragraph::new(lines)
        .wrap(wrap_setting)
        .scroll((effective_scroll_offset as u16, 0));
    frame.render_widget(paragraph, editor_content_area);

    if total_lines > visible_lines {
        let scroll_area = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(left_padding + center_width),
                Constraint::Length(1),
                Constraint::Min(0)
            ])
            .split(v_chunks[2])[1];

        let scrollbar = Scrollbar::default()
            .orientation(ScrollbarOrientation::VerticalRight)
            .begin_symbol(None)
            .end_symbol(None)
            .track_symbol(Some("│"))
            .thumb_symbol("█");
        let mut scroll_state = ScrollbarState::new(total_lines.saturating_sub(visible_lines))
            .position(effective_scroll_offset);
        frame.render_stateful_widget(scrollbar, scroll_area, &mut scroll_state);
    }

    let hints = Span::styled(
        "Ctrl+S save · Ctrl+F find · Ctrl+Space ghost",
        Style::default().fg(MUTED),
    );
    let bottom_hints = Paragraph::new(Line::from(hints))
        .alignment(Alignment::Right);
    frame.render_widget(bottom_hints, v_chunks[4]);

    if app.ai_overlay_visible() {
        let cursor_row = cursor_visual_row
            .unwrap_or(0)
            .saturating_sub(effective_scroll_offset as u16);
        render_ai_overlay(frame, app, editor_content_area, cursor_row, cursor_visual_col);
    }
}

fn render_ai_overlay(frame: &mut Frame, app: &App, area: Rect, cursor_row: u16, cursor_col: u16) {
    let messages = app.chat_messages();
    
    // Expand significantly if there's a long conversation
    let is_extended = messages.len() > 3;
    
    let base_width = if is_extended { 60 } else { 42 };
    let base_height = if is_extended { 20 } else { 10 };
    
    let width = area.width.saturating_sub(4).min(base_width);
    let height = area.height.saturating_sub(4).min(base_height);

    if width < 28 || height < 8 {
        return;
    }

    let max_x = area.x + area.width.saturating_sub(width + 1);
    let max_y = area.y + area.height.saturating_sub(height + 1);
    let side_gap = 2;

    let desired_x = if is_extended {
        // If extended, anchor to the right
        area.x + area.width.saturating_sub(width + 2)
    } else if cursor_col + width + side_gap < area.width {
        area.x + cursor_col + side_gap
    } else if cursor_col > width + side_gap {
        area.x + cursor_col - width - side_gap
    } else {
        area.x + area.width.saturating_sub(width + 1)
    };

    let desired_y = if is_extended {
        // If extended, anchor to the center-bottom
        area.y + area.height.saturating_sub(height + 2)
    } else if cursor_row + height + side_gap < area.height {
        area.y + cursor_row + side_gap
    } else if cursor_row > height + side_gap {
        area.y + cursor_row - height - side_gap
    } else {
        area.y + 1
    };

    let x = desired_x.min(max_x).max(area.x + 1);
    let y = desired_y.min(max_y).max(area.y + 1);
    let overlay_area = Rect::new(x, y, width, height);

    let glow = if app.ai_overlay_pulse_ticks() > 0 {
        match app.tick() % 3 {
            0 => ACCENT,
            1 => ACCENT_SOFT,
            _ => BORDER,
        }
    } else {
        ACCENT_SOFT
    };
    let ripple = GHOST_FRAMES[(app.tick() as usize) % GHOST_FRAMES.len()];

    let block = Block::default()
        .title(Span::styled(
            format!("The Ghost {}", ripple),
            Style::default().fg(glow).add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(glow))
        .style(Style::default().bg(PANEL));
    frame.render_widget(Clear, overlay_area);
    let inner = block.inner(overlay_area);
    frame.render_widget(block, overlay_area);

    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(3)])
        .split(inner);

    let mut lines: Vec<Line> = Vec::new();

    if messages.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled("  Ctrl+Space", Style::default().fg(ACCENT)),
            Span::raw(" summon  "),
            Span::styled("Esc", Style::default().fg(ACCENT_SOFT)),
            Span::raw(" dismiss"),
        ]));
        lines.push(Line::from(vec![
            Span::styled("  Click anywhere", Style::default().fg(MUTED)),
            Span::raw(" to banish the overlay."),
        ]));
    } else {
        // If extended, show more messages, otherwise show latest 2
        let take_count = if is_extended { 6 } else { 2 };
        for message in messages.iter().rev().take(take_count).collect::<Vec<_>>().into_iter().rev() {
            let speaker = if message.role == "user" { "You:" } else { "Ghost:" };
            let color = if message.role == "user" { ACCENT_SOFT } else { ACCENT };
            lines.push(Line::from(vec![
                Span::styled(
                    speaker,
                    Style::default().fg(color).add_modifier(Modifier::BOLD),
                ),
            ]));

            for content_line in message.content.lines() {
                if content_line.is_empty() {
                    lines.push(Line::from(""));
                } else {
                    lines.push(Line::from(vec![
                        Span::styled("  ", Style::default()),
                        Span::styled(content_line, Style::default().fg(TEXT)),
                    ]));
                }
            }

            lines.push(Line::from(""));
        }
    }

    let transcript = Paragraph::new(lines)
        .style(Style::default().bg(PANEL))
        .wrap(Wrap { trim: false });
    frame.render_widget(transcript, sections[0]);

    let input_block = Block::default()
        .borders(Borders::TOP)
        .border_style(Style::default().fg(glow))
        .style(Style::default().bg(PANEL));
    let input_inner = input_block.inner(sections[1]);
    frame.render_widget(input_block, sections[1]);

    let mut input_text = String::with_capacity(app.ai_input_buffer().len() + 1);
    let cursor = app.ai_input_cursor().min(app.ai_input_buffer().len());
    input_text.push_str(&app.ai_input_buffer()[..cursor]);
    if app.is_thinking() {
        input_text.push_str(app.thinking_frame());
    } else {
        input_text.push_str(CURSOR);
    }
    input_text.push_str(&app.ai_input_buffer()[cursor..]);

    let input_para = Paragraph::new(input_text).style(Style::default().fg(TEXT));
    frame.render_widget(input_para, input_inner);
}

fn render_commands_panel(frame: &mut Frame, app: &App, area: Rect) {
    let has_status = !app.panel_lines().is_empty();
    let panel_title = app.panel_title();

    // If no status to show and prompt is empty, show minimalist ghost text
    if !has_status && app.is_prompt_empty() {
        let block = Block::default()
            .title(Span::styled(
                "Aleph",
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
            ))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(BORDER));
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let ghost_text = Paragraph::new(vec![
            Line::from(""),
            Line::from(vec![
                Span::styled("  Type ", Style::default().fg(MUTED)),
                Span::styled("/", Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)),
                Span::styled(" to see commands, or just ask a question", Style::default().fg(MUTED)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("  Recent: ", Style::default().fg(MUTED)),
                Span::styled("/login  /obsidian pair  /ask", Style::default().fg(ACCENT_SOFT)),
            ]),
        ])
        .style(Style::default().fg(MUTED));
        frame.render_widget(ghost_text, inner);
        return;
    }

    if has_status && panel_title == "Strix sign-in" {
        render_strix_sign_in_panel(frame, app, area);
        return;
    }

    if has_status && panel_title == "Obsidian pairing" {
        render_obsidian_pairing_panel(frame, app, area);
        return;
    }

    // If user is typing a command, show filtered commands
    if app.is_typing_command() {
        let block = Block::default()
            .title(Span::styled(
                "Commands",
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
            ))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(BORDER))
            .style(Style::default().bg(PANEL));
        let inner = block.inner(area);
        frame.render_widget(Clear, area);
        frame.render_widget(block, area);

        let (suggestions, offset) = app.visible_commands_window(8);
        let total = app.total_command_matches();
        let remaining = total.saturating_sub(offset + suggestions.len());
        let selected_global = app.selected_suggestion();

        if suggestions.is_empty() {
            let no_match = Paragraph::new(vec![
                Line::from(""),
                Line::from(vec![
                    Span::styled("  No commands match '", Style::default().fg(MUTED)),
                    Span::styled(app.prompt(), Style::default().fg(TEXT)),
                    Span::styled("'", Style::default().fg(MUTED)),
                ]),
            ]);
            frame.render_widget(no_match, inner);
        } else {
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
    } else if has_status {
        let block = Block::default()
            .title(Span::styled(
                app.panel_title(),
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
            ))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(BORDER));
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let content: Vec<Line> = app
            .panel_lines()
            .iter()
            .map(|line| render_markdown_line(line))
            .collect();
        let paragraph = Paragraph::new(content).wrap(Wrap { trim: false });
        frame.render_widget(paragraph, inner);
        return;
    } else {
        // Empty state when typing non-command text
        let block = Block::default()
            .title(Span::styled(
                "Aleph",
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
            ))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(BORDER));
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let typing_hint = Paragraph::new(vec![
            Line::from(""),
            Line::from(vec![
                Span::styled("  Press ", Style::default().fg(MUTED)),
                Span::styled("Enter", Style::default().fg(ACCENT)),
                Span::styled(" to ask, or ", Style::default().fg(MUTED)),
                Span::styled("/", Style::default().fg(ACCENT)),
                Span::styled(" for commands", Style::default().fg(MUTED)),
            ]),
        ]);
        frame.render_widget(typing_hint, inner);
        return;
    };
}

fn render_connection_panel(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    accent: Color,
    intro: Vec<Line<'static>>,
    rows: Vec<(&'static str, &'static str)>,
    footer_left: &'static str,
    footer_right: &'static str,
) {
    let block = Block::default()
        .title(Span::styled(
            title,
            Style::default().fg(accent).add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(BORDER))
        .style(Style::default().bg(PANEL));

    let inner = block.inner(area);
    frame.render_widget(Clear, area);
    frame.render_widget(block, area);

    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(4), Constraint::Min(0), Constraint::Length(1)])
        .split(inner);

    let header = Paragraph::new(intro)
        .wrap(Wrap { trim: false })
        .style(Style::default().fg(TEXT));
    frame.render_widget(header, sections[0]);

    let table_rows = rows
        .into_iter()
        .map(|(label, value)| {
            Row::new(vec![
                Cell::from(Span::styled(label, Style::default().fg(MUTED))),
                Cell::from(Span::styled(value, Style::default().fg(TEXT))),
            ])
        })
        .collect::<Vec<_>>();

    let table = Table::new(table_rows, [Constraint::Length(16), Constraint::Min(0)])
        .column_spacing(2)
        .style(Style::default().fg(TEXT));
    frame.render_widget(table, sections[1]);

    let footer = Paragraph::new(Line::from(vec![
        Span::styled(footer_left, Style::default().fg(accent)),
        Span::raw(" · "),
        Span::styled(footer_right, Style::default().fg(MUTED)),
    ]))
    .alignment(Alignment::Right)
    .style(Style::default().fg(MUTED));
    frame.render_widget(footer, sections[2]);
}

fn render_strix_sign_in_panel(frame: &mut Frame, app: &App, area: Rect) {
    render_connection_panel(
        frame,
        area,
        app.panel_title(),
        ACCENT,
        vec![
            Line::from(vec![Span::styled(
                "Authenticate with Strix",
                Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
            )]),
            Line::from(vec![Span::styled(
                "Use a personal access token or device-code login. The real flow will live in the gateway layer.",
                Style::default().fg(MUTED),
            )]),
            Line::from(vec![Span::styled(
                "Press Enter to continue the sign-in flow, or Esc to close this panel.",
                Style::default().fg(MUTED),
            )]),
        ],
        vec![
            ("Status", "Disconnected"),
            ("Method", "Device code or token"),
            ("Scope", "notes, memory, canvas, darwin"),
            ("Storage", "OS keychain or encrypted config"),
        ],
        "Enter",
        "continue",
    );
}

fn render_obsidian_pairing_panel(frame: &mut Frame, app: &App, area: Rect) {
    render_connection_panel(
        frame,
        area,
        app.panel_title(),
        ACCENT_SOFT,
        vec![
            Line::from(vec![Span::styled(
                "Pair a local Obsidian vault",
                Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
            )]),
            Line::from(vec![Span::styled(
                "Use a running vault as the source for imported notes and folder context.",
                Style::default().fg(MUTED),
            )]),
            Line::from(vec![Span::styled(
                "Press Enter to choose a vault path, or Esc to back out.",
                Style::default().fg(MUTED),
            )]),
        ],
        vec![
            ("Status", "Not paired"),
            ("Vault", "Choose a local vault folder"),
            ("Mode", "Import-first, read-mostly"),
            ("Pull", "Notes, folders, backlinks"),
        ],
        "Enter",
        "pair vault",
    );
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
            lines.push(render_markdown_line_with_cursor(line_text, cursor_in_line, app.editor_cursor_style()));
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

fn editor_word_count(text: &str) -> usize {
    text.split_whitespace()
        .filter(|token| token.chars().any(|character| character.is_alphanumeric()))
        .count()
}

fn render_full_chat(frame: &mut Frame, app: &App, area: Rect) {
    let max_width = 80;
    let center_width = area.width.saturating_sub(8).min(max_width);
    let left_padding = area.width.saturating_sub(center_width) / 2;

    let v_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Top meta
            Constraint::Length(1), // Spacer
            Constraint::Min(0),    // Chat messages
            Constraint::Length(1), // Spacer
            Constraint::Length(1), // Input area
            Constraint::Length(1), // Bottom hints
        ])
        .margin(1)
        .split(area);

    let top_h_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(left_padding),
            Constraint::Length(center_width),
            Constraint::Min(0),
        ]);

    let meta_area = top_h_chunks.split(v_chunks[0])[1];
    let chat_area = top_h_chunks.split(v_chunks[2])[1];
    let input_area = top_h_chunks.split(v_chunks[4])[1];
    let hints_area = top_h_chunks.split(v_chunks[5])[1];

    let meta_style = if app.is_thinking() {
        Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(MUTED)
    };

    let title = if app.is_thinking() {
        format!("Aleph {} focusing...", app.thinking_frame())
    } else {
        String::from("Aleph Chat")
    };

    let top_meta = Paragraph::new(Line::from(vec![Span::styled(title, meta_style)]))
        .alignment(Alignment::Left);
    frame.render_widget(top_meta, meta_area);

    let messages = app.chat_messages();
    let mut lines: Vec<Line> = Vec::new();

    for msg in messages.iter() {
        let is_user = msg.role == "user";
        let prefix = if is_user { "You" } else { "Aleph" };
        let color = if is_user { ACCENT_SOFT } else { ACCENT };

        if !lines.is_empty() {
            lines.push(Line::from(""));
        }

        lines.push(Line::from(vec![
            Span::styled(
                format!("{} ", prefix),
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("({})", msg.timestamp),
                Style::default().fg(MUTED),
            ),
        ]));

        for content_line in msg.content.lines() {
            if content_line.is_empty() {
                lines.push(Line::from(""));
            } else {
                lines.push(render_markdown_line(content_line));
            }
        }
    }

    if messages.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled("Welcome to Aleph AI chat! Type a message below to start.", Style::default().fg(MUTED)),
        ]));
    }

    let wrap_setting = Wrap { trim: false };
    let scroll_y = lines
        .len()
        .saturating_sub(chat_area.height as usize)
        .min(u16::MAX as usize) as u16;

    let messages_widget = Paragraph::new(lines)
        .wrap(wrap_setting)
        .scroll((scroll_y, 0));
    frame.render_widget(messages_widget, chat_area);

    let input_buffer = app.chat_input_buffer();
    let cursor = app.chat_input_cursor().min(input_buffer.len());
    let before_cursor = &input_buffer[..cursor];
    let after_cursor = &input_buffer[cursor..];

    let input_line = Paragraph::new(Line::from(vec![
        Span::styled("> ", Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)),
        Span::styled(before_cursor, Style::default().fg(TEXT)),
        Span::styled(CURSOR, Style::default().fg(MUTED)),
        Span::styled(after_cursor, Style::default().fg(TEXT)),
    ]));
    frame.render_widget(input_line, input_area);

    let hints_spans = vec![
        Span::styled("Enter", Style::default().fg(ACCENT)),
        Span::raw(" send · "),
        Span::styled("Esc", Style::default().fg(ACCENT_SOFT)),
        Span::raw(" exit · "),
        Span::styled("Ctrl+C", Style::default().fg(ACCENT)),
        Span::raw(" quit"),
    ];
    let bottom_hints = Paragraph::new(Line::from(hints_spans)).alignment(Alignment::Right).style(Style::default().fg(MUTED));
    frame.render_widget(bottom_hints, hints_area);
}

#[cfg(test)]
mod tests {
    use super::editor_word_count;

    #[test]
    fn editor_word_count_ignores_punctuation_only_tokens() {
        assert_eq!(editor_word_count("."), 0);
        assert_eq!(editor_word_count("...   !"), 0);
    }

    #[test]
    fn editor_word_count_still_counts_text_tokens() {
        assert_eq!(editor_word_count("hello"), 1);
        assert_eq!(editor_word_count("hello . world"), 2);
        assert_eq!(editor_word_count("note-1"), 1);
    }
}