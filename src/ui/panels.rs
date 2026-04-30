use super::*;

pub(super) fn render_commands_panel(frame: &mut Frame, app: &App, area: Rect) {
    let has_status = !app.panel_lines().is_empty();
    let panel_title = app.panel_title();

    // If no status to show and prompt is empty, show minimalist ghost text
    if !has_status && app.is_prompt_empty() {
        // Determine title based on auth state
        let title_text = if app.is_openrouter_connected() || app.is_strix_connected() {
            "Aleph"
        } else {
            "Aleph"
        };

        let block = Block::default()
            .title(Span::styled(
                title_text,
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
            ))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(BORDER));
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let mut ghost_lines = vec![Line::from("")];

        // Show contextual prompt based on authentication status
        if app.is_openrouter_connected() || app.is_strix_connected() {
            ghost_lines.push(Line::from(vec![
                Span::styled("  Type ", Style::default().fg(MUTED)),
                Span::styled(
                    "/",
                    Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    " to see commands, or just ask a question",
                    Style::default().fg(MUTED),
                ),
            ]));
            ghost_lines.push(Line::from(""));
            ghost_lines.push(Line::from(vec![
                Span::styled("  Try: ", Style::default().fg(MUTED)),
                Span::styled(
                    "/ask  /note list  /memory search",
                    Style::default().fg(ACCENT_SOFT),
                ),
            ]));
        } else {
            ghost_lines.push(Line::from(vec![
                Span::styled("  Type ", Style::default().fg(MUTED)),
                Span::styled(
                    "/login",
                    Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
                ),
                Span::styled(" to get started, or ", Style::default().fg(MUTED)),
                Span::styled("/", Style::default().fg(ACCENT)),
                Span::styled(" for commands", Style::default().fg(MUTED)),
            ]));
            ghost_lines.push(Line::from(""));
            ghost_lines.push(Line::from(vec![
                Span::styled("  Available: ", Style::default().fg(MUTED)),
                Span::styled(
                    "/note list  /obsidian pair  /status",
                    Style::default().fg(ACCENT_SOFT),
                ),
            ]));
        }

        let ghost_text = Paragraph::new(ghost_lines).style(Style::default().fg(MUTED));
        frame.render_widget(ghost_text, inner);
        return;
    }

    if has_status && panel_title == "Strix sign-in" {
        render_strix_sign_in_panel(frame, app, area);
        return;
    }

    if app.is_login_picker() {
        render_login_picker_panel(frame, app, area);
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
            .map(|line| render_panel_markdown_line(line))
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

pub(super) fn render_connection_panel(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    accent: Color,
    intro: Vec<Line<'static>>,
    rows: Vec<(&'static str, String)>,
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
        .constraints([
            Constraint::Length(4),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
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

pub(super) fn render_login_picker_panel(frame: &mut Frame, app: &App, area: Rect) {
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
            Constraint::Length(3), // Header
            Constraint::Length(4), // Picker rows
            Constraint::Min(4),    // Status/Help
            Constraint::Length(1), // Footer
        ])
        .split(inner);

    // Header
    let header = Paragraph::new(vec![
        Line::from(vec![Span::styled(
            "Choose connection",
            Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
        )]),
        Line::from(vec![Span::styled(
            "Connect a Strix account or configure a model provider.",
            Style::default().fg(MUTED),
        )]),
    ])
    .wrap(Wrap { trim: false })
    .style(Style::default().fg(TEXT));
    frame.render_widget(header, sections[0]);

    // Options
    let is_openrouter_selected = app.login_picker_selected() == 0;
    let is_strix_selected = app.login_picker_selected() == 1;

    let selector_rows = vec![
        Row::new(vec![
            Cell::from(Span::styled(
                if is_openrouter_selected { "▶" } else { " " },
                Style::default().fg(if is_openrouter_selected {
                    ACCENT
                } else {
                    MUTED
                }),
            )),
            Cell::from(Span::styled(
                "OpenRouter",
                Style::default().fg(if is_openrouter_selected { TEXT } else { MUTED }),
            )),
            Cell::from(Span::styled(
                "Model provider API key",
                Style::default().fg(MUTED),
            )),
        ]),
        Row::new(vec![
            Cell::from(Span::styled(
                if is_strix_selected { "▶" } else { " " },
                Style::default().fg(if is_strix_selected { ACCENT } else { MUTED }),
            )),
            Cell::from(Span::styled(
                "Strix Gateway",
                Style::default().fg(if is_strix_selected { TEXT } else { MUTED }),
            )),
            Cell::from(Span::styled(
                "Sign in to sync Strix and use Strix AI",
                Style::default().fg(MUTED),
            )),
        ]),
    ];

    let selector_table = Table::new(
        selector_rows,
        [
            Constraint::Length(3),
            Constraint::Length(15),
            Constraint::Min(0),
        ],
    )
    .column_spacing(1)
    .style(Style::default().fg(TEXT));

    // Highlight the selected row
    let mut table_state = ratatui::widgets::TableState::default();
    table_state.select(Some(app.login_picker_selected()));
    frame.render_stateful_widget(
        selector_table.row_highlight_style(Style::default().bg(BORDER).fg(TEXT)),
        sections[1],
        &mut table_state,
    );

    // Status / Help block
    let status_block = Block::default()
        .borders(Borders::TOP)
        .border_style(Style::default().fg(BORDER));
    let status_inner = status_block.inner(sections[2]);
    frame.render_widget(status_block, sections[2]);

    let mut status_lines = Vec::new();

    if is_openrouter_selected {
        if app.is_openrouter_login_pending() {
            let pulse = crate::app::THINKING_FRAMES
                [(app.tick() as usize) % crate::app::THINKING_FRAMES.len()];
            status_lines.push(Line::from(vec![Span::styled(
                format!("{} Waiting for OpenRouter authorization...", pulse),
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
            )]));
            status_lines.push(Line::from(vec![Span::styled(
                "Please complete authorization in your browser.",
                Style::default().fg(MUTED),
            )]));
            status_lines.push(Line::from(vec![Span::styled(
                "Aleph will automatically detect when you finish.",
                Style::default().fg(MUTED),
            )]));
        } else {
            status_lines.push(Line::from(vec![Span::styled(
                "OpenRouter can be used as Aleph's external model provider.",
                Style::default().fg(MUTED),
            )]));
            status_lines.push(Line::from(""));
            status_lines.push(Line::from(vec![Span::styled(
                "Press Enter to open your browser and authorize an API key.",
                Style::default().fg(TEXT),
            )]));
        }
    } else {
        if app.is_strix_login_pending() {
            let pulse = crate::app::THINKING_FRAMES
                [(app.tick() as usize) % crate::app::THINKING_FRAMES.len()];
            status_lines.push(Line::from(vec![Span::styled(
                format!("{} Waiting for Strix browser login...", pulse),
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
            )]));
            status_lines.push(Line::from(vec![Span::styled(
                "Please complete the sign-in process in your browser.",
                Style::default().fg(MUTED),
            )]));
            status_lines.push(Line::from(vec![Span::styled(
                "Aleph will automatically store the native access token.",
                Style::default().fg(MUTED),
            )]));
        } else {
            status_lines.push(Line::from(vec![Span::styled(
                "Strix native auth uses a browser sign-in with PKCE and a localhost callback.",
                Style::default().fg(MUTED),
            )]));
            status_lines.push(Line::from(""));
            status_lines.push(Line::from(vec![Span::styled(
                "Press Enter to open your browser and authenticate with Strix.",
                Style::default().fg(TEXT),
            )]));
            status_lines.push(Line::from(vec![Span::styled(
                "Set STRIX_AUTH_BASE_URL if Strix is not running on http://localhost:3000.",
                Style::default().fg(MUTED),
            )]));
        }
    }

    let status_para = Paragraph::new(status_lines)
        .wrap(Wrap { trim: false })
        .style(Style::default().fg(TEXT));
    frame.render_widget(status_para, status_inner);

    // Footer
    let footer = Paragraph::new(Line::from(vec![
        Span::styled("↑/↓", Style::default().fg(ACCENT)),
        Span::raw(" select · "),
        Span::styled("Enter", Style::default().fg(ACCENT)),
        Span::raw(" confirm · "),
        Span::styled("Esc", Style::default().fg(MUTED)),
        Span::raw(" close"),
    ]))
    .alignment(Alignment::Right)
    .style(Style::default().fg(MUTED));
    frame.render_widget(footer, sections[3]);
}

pub(super) fn render_strix_sign_in_panel(frame: &mut Frame, app: &App, area: Rect) {
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

    // Layout: intro + provider selector + logs + footer
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Length(4), // Provider selector
            Constraint::Min(4),    // Logs area
            Constraint::Length(1), // Footer
        ])
        .split(inner);

    // Header
    let header = Paragraph::new(vec![
        Line::from(vec![Span::styled(
            "Choose model provider",
            Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
        )]),
        Line::from(vec![Span::styled(
            "Select OpenRouter or Strix as your model provider",
            Style::default().fg(MUTED),
        )]),
    ])
    .wrap(Wrap { trim: false })
    .style(Style::default().fg(TEXT));
    frame.render_widget(header, sections[0]);

    // Provider selector - minimal style like Obsidian pair
    let provider = app.ai_provider();
    let is_openrouter = matches!(provider, crate::app::AiProvider::OpenRouter);
    let is_strix = matches!(provider, crate::app::AiProvider::Strix);

    let selector_rows = vec![
        Row::new(vec![
            Cell::from(Span::styled(
                if is_openrouter { "●" } else { "○" },
                Style::default().fg(if is_openrouter { ACCENT } else { MUTED }),
            )),
            Cell::from(Span::styled(
                "OpenRouter",
                Style::default().fg(if is_openrouter { TEXT } else { MUTED }),
            )),
            Cell::from(Span::styled(
                "External models via API key",
                Style::default().fg(MUTED),
            )),
        ]),
        Row::new(vec![
            Cell::from(Span::styled(
                if is_strix { "●" } else { "○" },
                Style::default().fg(if is_strix { ACCENT } else { MUTED }),
            )),
            Cell::from(Span::styled(
                "Strix",
                Style::default().fg(if is_strix { TEXT } else { MUTED }),
            )),
            Cell::from(Span::styled(
                "Strix account and gateway",
                Style::default().fg(MUTED),
            )),
        ]),
    ];

    let selector_table = Table::new(
        selector_rows,
        [
            Constraint::Length(3),
            Constraint::Length(14),
            Constraint::Min(0),
        ],
    )
    .column_spacing(1)
    .style(Style::default().fg(TEXT));
    frame.render_widget(selector_table, sections[1]);

    // Logs area - scrollable log display
    let logs = app.strix_logs();
    let mut log_lines: Vec<Line> = Vec::new();

    if logs.is_empty() {
        log_lines.push(Line::from(vec![Span::styled(
            "No activity yet. Run /login strix or /login openrouter <key>.",
            Style::default().fg(MUTED),
        )]));
    } else {
        for log in logs
            .iter()
            .rev()
            .take(20)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
        {
            log_lines.push(Line::from(vec![Span::styled(
                log,
                Style::default().fg(TEXT),
            )]));
        }
    }

    let logs_block = Block::default()
        .title(Span::styled(
            "Activity Log",
            Style::default().fg(ACCENT_SOFT),
        ))
        .borders(Borders::TOP)
        .border_style(Style::default().fg(BORDER));
    let logs_inner = logs_block.inner(sections[2]);
    frame.render_widget(logs_block, sections[2]);

    let logs_para = Paragraph::new(log_lines)
        .wrap(Wrap { trim: false })
        .style(Style::default().fg(TEXT));
    frame.render_widget(logs_para, logs_inner);

    // Footer
    let footer = Paragraph::new(Line::from(vec![
        Span::styled("/login openrouter <key>", Style::default().fg(ACCENT)),
        Span::raw(" · "),
        Span::styled("/login strix", Style::default().fg(ACCENT)),
        Span::raw(" · "),
        Span::styled("Esc", Style::default().fg(MUTED)),
        Span::raw(" close"),
    ]))
    .alignment(Alignment::Right)
    .style(Style::default().fg(MUTED));
    frame.render_widget(footer, sections[3]);
}

pub(super) fn render_obsidian_pairing_panel(frame: &mut Frame, app: &App, area: Rect) {
    let status = if let Some(path) = app.obsidian_vault_path() {
        format!("Paired: {}", path.display())
    } else {
        String::from("Not paired")
    };
    let detected = format!("{} detected", app.obsidian_vaults().len());
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
                "Import Markdown directly and use Obsidian URIs only when opening the app.",
                Style::default().fg(MUTED),
            )]),
            Line::from(vec![Span::styled(
                "Type /obsidian pair <path>, /obsidian vaults, or /obsidian sync.",
                Style::default().fg(MUTED),
            )]),
        ],
        vec![
            ("Status", status),
            ("Vaults", detected),
            ("Mode", String::from("Direct Markdown sync")),
            ("Open", String::from("obsidian:// URI fallback")),
        ],
        "/obsidian sync",
        "sync notes",
    );
}

pub(super) fn render_obsidian_vault_picker_panel(frame: &mut Frame, app: &App, area: Rect) {
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
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(inner);

    let header = Paragraph::new(vec![
        Line::from(vec![Span::styled(
            "Pick an Obsidian vault",
            Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
        )]),
        Line::from(vec![Span::styled(
            "Aleph reads Markdown files directly; no Obsidian CLI dependency is required.",
            Style::default().fg(MUTED),
        )]),
    ]);
    frame.render_widget(header, sections[0]);

    let selected = app.obsidian_vault_selected();
    let lines = if app.obsidian_vaults().is_empty() {
        vec![Line::from(vec![Span::styled(
            "No desktop vaults detected. Type /obsidian pair /path/to/vault.",
            Style::default().fg(MUTED),
        )])]
    } else {
        app.obsidian_vaults()
            .iter()
            .enumerate()
            .map(|(index, vault)| {
                let marker = if index == selected { "▶ " } else { "  " };
                let style = if index == selected {
                    Style::default().fg(TEXT).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(MUTED)
                };
                Line::from(vec![
                    Span::styled(marker, Style::default().fg(ACCENT)),
                    Span::styled(format!("{} — {}", vault.name, vault.path.display()), style),
                ])
            })
            .collect()
    };
    frame.render_widget(
        Paragraph::new(lines).wrap(Wrap { trim: false }),
        sections[1],
    );

    let footer = Paragraph::new(Line::from(vec![
        Span::styled("Enter", Style::default().fg(ACCENT)),
        Span::raw(" pair · "),
        Span::styled("Esc", Style::default().fg(MUTED)),
        Span::raw(" cancel"),
    ]))
    .alignment(Alignment::Right)
    .style(Style::default().fg(MUTED));
    frame.render_widget(footer, sections[2]);
}

pub(super) fn render_note_list_panel(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .title(Span::styled(
            app.panel_title(),
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(BORDER));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(inner);

    let selected = app
        .note_list_selected()
        .min(app.panel_lines().len().saturating_sub(1));
    let rows = app
        .panel_lines()
        .iter()
        .map(|line| {
            Row::new(vec![Cell::from(Span::styled(
                line,
                Style::default().fg(MUTED),
            ))])
        })
        .collect::<Vec<_>>();

    let table = Table::new(rows, [Constraint::Min(0)])
        .row_highlight_style(Style::default().fg(TEXT).add_modifier(Modifier::BOLD))
        .highlight_symbol("▶ ")
        .column_spacing(0);

    let mut table_state = ratatui::widgets::TableState::default();
    if !app.panel_lines().is_empty() {
        table_state.select(Some(selected));
    }
    frame.render_stateful_widget(table, sections[0], &mut table_state);

    let footer = if app.note_list_delete_is_pending() {
        Line::from(vec![
            Span::styled("Delete", Style::default().fg(Color::Rgb(209, 118, 128))),
            Span::raw(" confirm · "),
            Span::styled("Enter", Style::default().fg(Color::Rgb(209, 118, 128))),
            Span::raw(" confirm · "),
            Span::styled("d", Style::default().fg(Color::Rgb(209, 118, 128))),
            Span::raw(" confirm · "),
            Span::styled("Esc", Style::default().fg(ACCENT_SOFT)),
            Span::raw(" cancel"),
        ])
    } else {
        Line::from(vec![
            Span::styled("Enter", Style::default().fg(ACCENT)),
            Span::raw(" open · "),
            Span::styled("Delete", Style::default().fg(ACCENT_SOFT)),
            Span::raw(" delete · "),
            Span::styled("Esc", Style::default().fg(MUTED)),
            Span::raw(" close"),
        ])
    };
    frame.render_widget(
        Paragraph::new(footer)
            .alignment(Alignment::Right)
            .style(Style::default().fg(MUTED)),
        sections[1],
    );
}

pub(super) fn render_note_editor_panel(frame: &mut Frame, app: &App, area: Rect) {
    let base_title = app
        .editor_note_title()
        .map(|note| format!("Editing: {}", note))
        .unwrap_or_else(|| String::from("Editing note"));

    let mut title_spans = vec![Span::styled(
        base_title,
        Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
    )];

    if app.search_state().active {
        title_spans.push(Span::raw("  "));
        title_spans.push(Span::styled(
            format!("Find: {}", app.search_state().query),
            Style::default()
                .fg(ACCENT_SOFT)
                .add_modifier(Modifier::BOLD),
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
        .constraints([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
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
            lines.push(render_markdown_line_with_cursor(
                line_text,
                cursor_in_line,
                app.editor_cursor_style(),
            ));
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
    effective_scroll_offset =
        effective_scroll_offset.min(total_lines.saturating_sub(visible_lines));

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

pub(super) fn editor_word_count(text: &str) -> usize {
    text.split_whitespace()
        .filter(|token| token.chars().any(|character| character.is_alphanumeric()))
        .count()
}
