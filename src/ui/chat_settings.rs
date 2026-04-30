use super::*;

pub(super) fn render_full_chat(frame: &mut Frame, app: &App, area: Rect) {
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

    let mode_label = if app.is_agent_mode_enabled() {
        "Agent"
    } else {
        "Chat"
    };
    let provider_connected = match app.ai_provider() {
        AiProvider::OpenRouter => app.is_openrouter_connected(),
        AiProvider::Strix => app.is_strix_connected(),
    };
    let provider_status = if provider_connected {
        "connected"
    } else {
        "offline"
    };

    let title = if app.is_streaming() {
        format!("Aleph {} {} streaming...", mode_label, app.thinking_frame())
    } else if app.is_thinking() {
        format!("Aleph {} {} focusing...", mode_label, app.thinking_frame())
    } else {
        format!(
            "Aleph {} · {} {}",
            mode_label,
            app.ai_provider_label(),
            provider_status
        )
    };

    let top_meta = Paragraph::new(Line::from(vec![Span::styled(title, meta_style)]))
        .alignment(Alignment::Left);
    frame.render_widget(top_meta, meta_area);

    let mut lines: Vec<Line<'static>> = app.chat_render_lines().to_vec();
    if app.is_streaming() {
        lines.push(Line::from(""));
        lines.push(Line::from(vec![Span::styled(
            format!("{} typing...", app.thinking_frame()),
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        )]));
    }

    let wrap_setting = Wrap { trim: false };
    let visible_lines = chat_area.height as usize;
    let max_scroll = lines.len().saturating_sub(visible_lines);
    let scroll_y = max_scroll
        .saturating_sub(app.chat_scroll_offset().min(max_scroll))
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
        Span::styled(
            "> ",
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ),
        Span::styled(before_cursor, Style::default().fg(TEXT)),
        Span::styled(CURSOR, Style::default().fg(MUTED)),
        Span::styled(after_cursor, Style::default().fg(TEXT)),
    ]));
    frame.render_widget(input_line, input_area);

    let hints_spans = vec![
        Span::styled("Enter", Style::default().fg(ACCENT)),
        Span::raw(" send · "),
        Span::styled("PgUp/PgDn", Style::default().fg(ACCENT_SOFT)),
        Span::raw(" scroll · "),
        Span::styled("Ctrl+G", Style::default().fg(ACCENT_SOFT)),
        Span::raw(" mode · "),
        Span::styled("Esc", Style::default().fg(ACCENT_SOFT)),
        Span::raw(" exit · "),
        Span::styled("Ctrl+C", Style::default().fg(ACCENT)),
        Span::raw(" quit"),
    ];
    let bottom_hints = Paragraph::new(Line::from(hints_spans))
        .alignment(Alignment::Right)
        .style(Style::default().fg(MUTED));
    frame.render_widget(bottom_hints, hints_area);
}

pub(super) fn render_settings_panel(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .title(Span::styled(
            app.panel_title(),
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(BORDER))
        .style(Style::default().bg(PANEL));
    let inner = block.inner(area);
    frame.render_widget(Clear, area);
    frame.render_widget(block, area);

    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(inner);

    let header = Paragraph::new(vec![Line::from(vec![Span::styled(
        "Manage your connections, model provider, and preferences",
        Style::default().fg(MUTED),
    )])]);
    frame.render_widget(header, sections[0]);

    let selected = app.settings_selected();
    let model_provider_label = match app.ai_provider() {
        crate::app::AiProvider::OpenRouter => {
            if app.is_openrouter_connected() {
                "OpenRouter (connected)"
            } else {
                "OpenRouter (login required)"
            }
        }
        crate::app::AiProvider::Strix => {
            if app.is_strix_connected() {
                "Strix (connected)"
            } else {
                "Strix (login required)"
            }
        }
    };

    let settings_items: Vec<(String, String)> = vec![
        (
            "Model Provider".to_string(),
            format!("{} (Enter to cycle)", model_provider_label),
        ),
        (
            "Mode".to_string(),
            if app.is_agent_mode_enabled() {
                "Agent (tool routing on)".to_string()
            } else {
                "Chat (answers only)".to_string()
            },
        ),
        (
            "Save Notes".to_string(),
            format!("{} (Enter to cycle)", app.note_save_target_label()),
        ),
        (
            "Editor Images".to_string(),
            if app.editor_images_enabled() {
                "Enabled (Enter to disable)".to_string()
            } else {
                "Disabled (Enter to enable)".to_string()
            },
        ),
        (
            "Obsidian Vault".to_string(),
            if app.obsidian_vault_path().is_some() {
                "Paired".to_string()
            } else {
                "Not paired".to_string()
            },
        ),
        (
            "Sign out".to_string(),
            "Clear all saved credentials".to_string(),
        ),
        (
            "Reset & Clear".to_string(),
            "Clear cache and reset all settings".to_string(),
        ),
        ("Close".to_string(), "Exit settings".to_string()),
    ];

    let lines: Vec<Line> = settings_items
        .iter()
        .enumerate()
        .map(|(index, (name, value))| {
            let marker = if index == selected { "▶ " } else { "  " };
            let name_style = if index == selected {
                Style::default().fg(TEXT).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(TEXT)
            };
            let value_style = if index == selected {
                Style::default().fg(ACCENT)
            } else {
                Style::default().fg(MUTED)
            };
            Line::from(vec![
                Span::styled(marker, Style::default().fg(ACCENT)),
                Span::styled(format!("{:<15}", name), name_style),
                Span::styled(value, value_style),
            ])
        })
        .collect();

    frame.render_widget(
        Paragraph::new(lines).wrap(Wrap { trim: false }),
        sections[1],
    );

    let footer = Paragraph::new(Line::from(vec![
        Span::styled("↑/↓", Style::default().fg(ACCENT)),
        Span::raw(" navigate · "),
        Span::styled("Enter", Style::default().fg(ACCENT)),
        Span::raw(" select · "),
        Span::styled("Esc", Style::default().fg(MUTED)),
        Span::raw(" close"),
    ]))
    .alignment(Alignment::Right)
    .style(Style::default().fg(MUTED));
    frame.render_widget(footer, sections[2]);
}

pub(super) fn render_obsidian_sync_confirm_panel(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .title(Span::styled(
            app.panel_title(),
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(BORDER))
        .style(Style::default().bg(PANEL));
    let inner = block.inner(area);
    frame.render_widget(Clear, area);
    frame.render_widget(block, area);

    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(inner);

    let lines: Vec<Line> = app
        .panel_lines()
        .iter()
        .map(|line| {
            Line::from(vec![Span::styled(
                format!("  {}", line),
                Style::default().fg(TEXT),
            )])
        })
        .collect();

    frame.render_widget(
        Paragraph::new(lines).wrap(Wrap { trim: false }),
        sections[0],
    );

    let footer = Paragraph::new(Line::from(vec![
        Span::styled("Enter/Y", Style::default().fg(ACCENT)),
        Span::raw(" sync now · "),
        Span::styled("Esc/N", Style::default().fg(MUTED)),
        Span::raw(" skip"),
    ]))
    .alignment(Alignment::Right)
    .style(Style::default().fg(MUTED));
    frame.render_widget(footer, sections[1]);
}
