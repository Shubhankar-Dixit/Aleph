use super::diff::{compute_line_diff, render_diff_line};
use super::panels::editor_word_count;
use super::*;

pub(super) fn render_full_editor(frame: &mut Frame, app: &App, area: Rect) {
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

    // Get original and proposed text for diff rendering
    let original_text = app.editor_buffer();
    let editor_text = app.editor_display_buffer();
    let is_ai_preview = app.has_live_ai_editor_preview();
    let word_count = editor_word_count(editor_text);

    let meta_style = if app.save_shimmer_ticks() > 0 {
        Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(EDITOR_MUTED)
    };

    let mut meta_spans = vec![];

    if app.is_editing_title() {
        // Show title editing mode
        let cursor = app.title_cursor().min(app.title_buffer().len());
        meta_spans.push(Span::styled("Title: ", Style::default().fg(ACCENT)));
        meta_spans.push(Span::styled(
            &app.title_buffer()[..cursor],
            Style::default().fg(EDITOR_TEXT),
        ));
        meta_spans.push(Span::styled(CURSOR, Style::default().fg(ACCENT)));
        meta_spans.push(Span::styled(
            &app.title_buffer()[cursor..],
            Style::default().fg(EDITOR_TEXT),
        ));
        meta_spans.push(Span::raw("  "));
        meta_spans.push(Span::styled(
            "(Enter to save, Esc to cancel)",
            Style::default().fg(EDITOR_MUTED),
        ));
    } else {
        meta_spans.push(Span::styled(
            if app.has_live_ai_editor_preview() {
                format!("{} / AI preview / {} words", title, word_count)
            } else {
                format!("{} / Draft / {} words", title, word_count)
            },
            meta_style,
        ));
    }

    if app.search_state().active {
        meta_spans.push(Span::raw("  "));
        meta_spans.push(Span::styled(
            format!("Find: {}", app.search_state().query),
            Style::default()
                .fg(ACCENT_SOFT)
                .add_modifier(Modifier::BOLD),
        ));
        if let Some(current) = app.search_state().current_match {
            let total = app.search_state().matches.len();
            meta_spans.push(Span::styled(
                format!(" ({}/{})", current + 1, total),
                Style::default().fg(EDITOR_MUTED),
            ));
        }
    }

    let top_meta = Paragraph::new(Line::from(meta_spans)).alignment(Alignment::Left);
    frame.render_widget(top_meta, v_chunks[0]);

    let cursor = app.editor_display_cursor().min(editor_text.len());

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
    let selection = app.editor_selection();
    let has_selection = selection.active && !is_ai_preview;

    // Use diff rendering for AI preview, otherwise normal rendering
    if is_ai_preview {
        // Render diff view
        let diff_lines = compute_line_diff(original_text, editor_text);
        for (line_text, line_type) in diff_lines {
            lines.push(render_diff_line(line_text, line_type));
        }
    } else {
        // Normal rendering with selection support
        for line_text in editor_text.lines() {
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

                if has_selection {
                    lines.push(render_markdown_line_with_selection(
                        line_text,
                        cursor_in_line,
                        app.editor_cursor_style(),
                        selection,
                        line_start,
                    ));
                } else {
                    lines.push(render_markdown_line_with_cursor(
                        line_text,
                        cursor_in_line,
                        app.editor_cursor_style(),
                    ));
                }
            } else if has_selection && selection.end > line_start && selection.start < line_end {
                // Selection covers this line but cursor is elsewhere
                lines.push(render_markdown_line_with_selection(
                    line_text,
                    0, // cursor not in this line
                    app.editor_cursor_style(),
                    selection,
                    line_start,
                ));
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

        if editor_text.is_empty() {
            if has_selection {
                lines.push(render_markdown_line_with_selection(
                    "",
                    0,
                    app.editor_cursor_style(),
                    selection,
                    0,
                ));
            } else {
                lines.push(render_markdown_line_with_cursor(
                    "",
                    0,
                    app.editor_cursor_style(),
                ));
            }
        }
    }

    let cursor_line = editor_text[..cursor.min(editor_text.len())]
        .chars()
        .filter(|&c| c == '\n')
        .count();

    let total_lines = if is_ai_preview {
        lines.len().max(1)
    } else {
        editor_text.lines().count().max(1)
    };
    let visible_lines = editor_content_area.height as usize;

    let mut effective_scroll_offset = app.editor_scroll_offset();
    if cursor_line < effective_scroll_offset {
        effective_scroll_offset = cursor_line;
    } else if cursor_line >= effective_scroll_offset + visible_lines {
        effective_scroll_offset = cursor_line.saturating_sub(visible_lines - 1);
    }
    effective_scroll_offset =
        effective_scroll_offset.min(total_lines.saturating_sub(visible_lines));

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
                Constraint::Min(0),
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
        if app.has_live_ai_editor_preview() {
            "Enter accept AI preview · Ctrl+R reject · Esc reject"
        } else {
            "Tab edit title · Ctrl+S save · Ctrl+F find · Ctrl+Space ghost"
        },
        Style::default().fg(if app.has_live_ai_editor_preview() {
            ACCENT_SOFT
        } else {
            MUTED
        }),
    );
    let bottom_hints = Paragraph::new(Line::from(hints)).alignment(Alignment::Right);
    frame.render_widget(bottom_hints, v_chunks[4]);

    if app.ai_overlay_visible() {
        let cursor_row = cursor_visual_row
            .unwrap_or(0)
            .saturating_sub(effective_scroll_offset as u16);
        render_ai_overlay(
            frame,
            app,
            editor_content_area,
            cursor_row,
            cursor_visual_col,
        );
    }
}

pub(super) fn render_ai_overlay(
    frame: &mut Frame,
    app: &App,
    area: Rect,
    cursor_row: u16,
    cursor_col: u16,
) {
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

    if app.has_pending_ai_edit() {
        lines.push(Line::from(vec![Span::styled(
            format!("  {}", app.pending_ai_proposal_label()),
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        )]));
        lines.push(Line::from(vec![
            Span::styled("  Enter", Style::default().fg(ACCENT_SOFT)),
            Span::styled(" apply  ", Style::default().fg(MUTED)),
            Span::styled("Ctrl+R", Style::default().fg(ACCENT_SOFT)),
            Span::styled(" reject", Style::default().fg(MUTED)),
        ]));
        if let Some(instruction) = app.pending_ai_instruction() {
            lines.push(Line::from(vec![Span::styled(
                format!("  {}", instruction),
                Style::default().fg(MUTED),
            )]));
        }
        lines.push(Line::from(""));
        lines.push(Line::from(vec![Span::styled(
            "  Preview is shown directly in the editor.",
            Style::default().fg(MUTED),
        )]));
    } else if messages.is_empty() && !app.is_ghost_streaming() && app.ghost_result().is_none() {
        lines.push(Line::from(""));
        lines.push(Line::from(vec![Span::styled(
            "  Type an instruction for the AI editor",
            Style::default().fg(MUTED),
        )]));
        lines.push(Line::from(vec![
            Span::styled("  e.g. ", Style::default().fg(MUTED)),
            Span::styled("\"make it more concise\"", Style::default().fg(ACCENT_SOFT)),
        ]));
        lines.push(Line::from(vec![
            Span::styled("  e.g. ", Style::default().fg(MUTED)),
            Span::styled(
                "\"add a summary section\"",
                Style::default().fg(ACCENT_SOFT),
            ),
        ]));
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled("  Enter", Style::default().fg(ACCENT_SOFT)),
            Span::styled(" propose edits  ", Style::default().fg(MUTED)),
            Span::styled("Esc", Style::default().fg(ACCENT_SOFT)),
            Span::styled(" dismiss", Style::default().fg(MUTED)),
        ]));
    } else if app.is_ghost_streaming() {
        let frame_char = GHOST_FRAMES[(app.tick() as usize) % GHOST_FRAMES.len()];
        lines.push(Line::from(""));
        lines.push(Line::from(vec![Span::styled(
            format!("  {} Ghost is thinking...", frame_char),
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        )]));
        lines.push(Line::from(vec![Span::styled(
            "  Drafting live in the editor...",
            Style::default().fg(MUTED),
        )]));
    } else if let Some(result) = app.ghost_result() {
        lines.push(Line::from(""));
        lines.push(Line::from(vec![Span::styled(
            "  AI editor:",
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        )]));
        for line in result.lines().take(8) {
            lines.push(Line::from(vec![Span::styled(
                format!("  {}", line),
                Style::default().fg(TEXT),
            )]));
        }
    } else {
        // If extended, show more messages, otherwise show latest 2
        let take_count = if is_extended { 6 } else { 2 };
        for message in messages
            .iter()
            .rev()
            .take(take_count)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
        {
            let speaker = if message.role == "user" {
                "You:"
            } else {
                "Ghost:"
            };
            let color = if message.role == "user" {
                ACCENT_SOFT
            } else {
                ACCENT
            };
            lines.push(Line::from(vec![Span::styled(
                speaker,
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            )]));

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
