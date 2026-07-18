use super::*;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

pub(crate) fn settings_items_area(area: Rect) -> Rect {
    let inner = area.inner(Margin {
        horizontal: 1,
        vertical: 1,
    });
    settings_panel_sections(inner)[1]
}

fn settings_panel_sections(inner: Rect) -> std::rc::Rc<[Rect]> {
    Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(inner)
}

#[derive(Clone, Copy, Debug)]
pub(super) struct ChatLayout {
    pub meta: Rect,
    pub transcript: Rect,
    pub composer: Rect,
    pub composer_context: Rect,
    pub hints: Rect,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ChatComposerGeometry {
    pub area: Rect,
    pub content_width: usize,
}

pub(crate) fn chat_composer_geometry(app: &App, area: Rect) -> ChatComposerGeometry {
    let layout = chat_layout(app, area);
    ChatComposerGeometry {
        area: layout.composer,
        content_width: layout.composer.width.saturating_sub(2).max(1) as usize,
    }
}

pub(crate) fn chat_composer_hit_test(app: &App, area: Rect, position: Position) -> bool {
    chat_composer_geometry(app, area).area.contains(position)
}

struct RenderedTranscriptBlock {
    id: TranscriptBlockId,
    lines: Vec<Line<'static>>,
}

fn laid_out_transcript(app: &App, area: Rect) -> (Vec<Line<'static>>, TranscriptLayoutSnapshot) {
    let transcript = chat_layout(app, area).transcript;
    let mut lines = Vec::new();
    let mut blocks = Vec::new();
    for block in render_chat_workspace_blocks(app) {
        let wrapped = wrap_lines_to_width(block.lines, transcript.width as usize);
        let start_row = lines.len();
        let row_count = wrapped.len();
        lines.extend(wrapped);
        blocks.push(TranscriptLayoutBlock {
            id: block.id,
            start_row,
            row_count,
        });
    }
    let snapshot = TranscriptLayoutSnapshot {
        total_rows: lines.len(),
        viewport_rows: transcript.height as usize,
        blocks,
    };
    (lines, snapshot)
}

pub(crate) fn chat_transcript_layout(app: &App, area: Rect) -> TranscriptLayoutSnapshot {
    laid_out_transcript(app, area).1
}

pub(super) fn chat_layout(app: &App, area: Rect) -> ChatLayout {
    let max_width = 120;
    let center_width = area.width.saturating_sub(4).min(max_width);
    let left_padding = area.width.saturating_sub(center_width) / 2;
    let x = area.x.saturating_add(left_padding);
    let vertical_margin = u16::from(area.height >= 3);
    let y = area.y.saturating_add(vertical_margin);
    let height = area
        .height
        .saturating_sub(vertical_margin.saturating_mul(2));
    let row = |y, height| Rect::new(x, y, center_width, height);

    // Keep the composer usable first, then progressively restore metadata,
    // context, shortcuts, and transcript space as terminal height permits.
    let meta_height = if height >= 4 { 2 } else { 0 };
    let context_height = u16::from(height >= 5);
    let hints_height = u16::from(height >= 7);
    let top_gap = u16::from(height >= 8);
    let composer_gap = u16::from(height >= 8);
    let fixed_height = meta_height + context_height + hints_height + top_gap + composer_gap;
    let flexible_height = height.saturating_sub(fixed_height);

    let content_width = center_width.saturating_sub(2).max(1) as usize;
    let desired_composer_height = app
        .chat_composer()
        .visual_lines(content_width)
        .len()
        .clamp(2, 6) as u16;
    let composer_height = desired_composer_height
        .min(flexible_height.saturating_sub(1).max(1))
        .min(flexible_height);
    let transcript_height = flexible_height.saturating_sub(composer_height);

    let meta = row(y, meta_height);
    let transcript_y = y.saturating_add(meta_height).saturating_add(top_gap);
    let transcript = row(transcript_y, transcript_height);
    let composer_y = transcript_y
        .saturating_add(transcript_height)
        .saturating_add(composer_gap);
    let composer = row(composer_y, composer_height);
    let composer_context = row(composer_y.saturating_add(composer_height), context_height);
    let hints = row(
        composer_context.y.saturating_add(context_height),
        hints_height,
    );

    ChatLayout {
        meta,
        transcript,
        composer,
        composer_context,
        hints,
    }
}

pub(super) fn render_full_chat(frame: &mut Frame, app: &App, area: Rect) {
    let layout = chat_layout(app, area);
    let room_accent = app.room_accent();
    let current_mode = chat_console_mode(app);
    let chat_area = layout.transcript;

    let pulse_label = if app.is_streaming() || app.is_thinking() {
        app.thinking_status()
    } else {
        "ready"
    };

    let top_meta = Paragraph::new(vec![
        Line::from(vec![
            Span::styled(
                "Aleph",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("  ·  ", Style::default().fg(MUTED)),
            Span::styled(
                current_mode,
                Style::default().fg(MUTED).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(Span::styled(
            pulse_label,
            Style::default().fg(MUTED).add_modifier(Modifier::ITALIC),
        )),
    ])
    .alignment(Alignment::Left)
    .style(Style::default().fg(TEXT));
    frame.render_widget(top_meta, layout.meta);

    let (lines, transcript_layout) = laid_out_transcript(app, area);
    let scroll_y = app
        .transcript_top_row(&transcript_layout)
        .min(u16::MAX as usize) as u16;

    let messages_widget = Paragraph::new(lines)
        .scroll((scroll_y, 0))
        .style(Style::default().fg(MUTED));
    frame.render_widget(messages_widget, chat_area);

    if app.transcript_viewport().has_new_activity() && chat_area.height > 0 {
        let indicator = Paragraph::new(Line::from(Span::styled(
            "↓ new activity · End to follow",
            Style::default()
                .fg(Color::White)
                .bg(ACCENT_SOFT)
                .add_modifier(Modifier::BOLD),
        )))
        .alignment(Alignment::Right);
        let width = chat_area.width.min(29);
        let indicator_area = Rect::new(
            chat_area.right().saturating_sub(width),
            chat_area.bottom().saturating_sub(1),
            width,
            1,
        );
        frame.render_widget(indicator, indicator_area);
    }

    render_chat_composer(frame, app, layout, room_accent);
}

fn render_chat_composer(frame: &mut Frame, app: &App, layout: ChatLayout, room_accent: Color) {
    let composer = app.chat_composer();
    let input_hovered = composer.hovered();
    let prompt_style = if input_hovered {
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
            .fg(room_accent)
            .add_modifier(Modifier::BOLD)
    };
    let input_text_style = if input_hovered {
        Style::default().fg(Color::White)
    } else {
        Style::default().fg(TEXT)
    };
    let content_width = layout.composer.width.saturating_sub(2).max(1) as usize;
    let visual_lines = composer.visual_lines(content_width);
    let visible_rows = layout.composer.height.max(1) as usize;
    let (cursor_row, cursor_column) = composer.cursor_visual_position(content_width);
    let max_viewport = visual_lines.len().saturating_sub(visible_rows);
    let mut viewport = composer.viewport_row().min(max_viewport);
    if cursor_row < viewport {
        viewport = cursor_row;
    } else if cursor_row >= viewport + visible_rows {
        viewport = cursor_row + 1 - visible_rows;
    }

    let mut rendered = Vec::with_capacity(visible_rows);
    if composer.buffer().is_empty() {
        let placeholder = if app
            .active_agent_run()
            .is_some_and(|run| !run.phase.is_terminal())
        {
            "Prepare the next instruction…"
        } else {
            "Ask Aleph to inspect, plan, or change something…"
        };
        rendered.push(Line::from(vec![
            Span::styled("❯ ", prompt_style),
            Span::styled(
                placeholder,
                Style::default().fg(MUTED).add_modifier(Modifier::ITALIC),
            ),
        ]));
    } else {
        for (row_index, visual) in visual_lines
            .iter()
            .enumerate()
            .skip(viewport)
            .take(visible_rows)
        {
            let prefix = if row_index == 0 { "❯ " } else { "  " };
            rendered.push(Line::from(vec![
                Span::styled(prefix, prompt_style),
                Span::styled(
                    composer.buffer()[visual.start..visual.end].to_string(),
                    input_text_style,
                ),
            ]));
        }
    }
    frame.render_widget(Paragraph::new(rendered), layout.composer);

    let active_label = app
        .active_agent_run()
        .filter(|run| !run.phase.is_terminal())
        .map(|run| format!("run {:?}", run.phase).to_lowercase())
        .unwrap_or_else(|| String::from("idle"));
    let context_reference = app
        .active_agent_run()
        .filter(|run| !run.phase.is_terminal())
        .and_then(|run| run.context.selected_note.as_deref())
        .map(|note| format!(" · note {}", note))
        .unwrap_or_default();
    let context = format!(
        "{} · {} · {} · {}{}",
        composer_mode_label(app),
        app.agent_context_scope_label(),
        app.agent_approval_policy().label(),
        active_label,
        context_reference,
    );
    let context = if let Some(notice) = composer.notice() {
        format!("{}  ·  {}", notice, context)
    } else {
        context
    };
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            context,
            Style::default().fg(MUTED),
        ))),
        layout.composer_context,
    );

    let hint_key = |label: &'static str| {
        Span::styled(
            label,
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )
    };
    let hint_label = |label: &'static str| Span::styled(label, Style::default().fg(MUTED));
    let hints_spans = if layout.hints.width < 72 {
        vec![
            hint_key("Enter"),
            hint_label(":send  "),
            hint_key("Alt+Enter"),
            hint_label(":newline  "),
            hint_key("Esc"),
            hint_label(":back"),
        ]
    } else {
        vec![
            hint_key("Enter"),
            hint_label(":send"),
            hint_label("   |   "),
            hint_key("Alt+Enter"),
            hint_label(":newline"),
            hint_label("   |   "),
            hint_key("PgUp/PgDn"),
            hint_label(":scroll"),
            hint_label("   |   "),
            hint_key("Ctrl+G"),
            hint_label(":mode"),
            hint_label("   |   "),
            hint_key("Esc"),
            hint_label(":exit"),
            hint_label("   |   "),
            hint_key("Ctrl+C"),
            hint_label(":clear/quit"),
        ]
    };
    let bottom_hints = Paragraph::new(Line::from(hints_spans))
        .alignment(Alignment::Left)
        .style(Style::default().fg(MUTED));
    frame.render_widget(bottom_hints, layout.hints);

    if matches!(
        composer.interaction(),
        ComposerInteraction::Editing | ComposerInteraction::Approval
    ) && layout.composer.width > 2
        && layout.composer.height > 0
    {
        let screen_row = cursor_row.saturating_sub(viewport).min(visible_rows - 1) as u16;
        let x = layout
            .composer
            .x
            .saturating_add(2)
            .saturating_add(cursor_column.min(content_width.saturating_sub(1)) as u16)
            .min(layout.composer.right().saturating_sub(1));
        let y = layout
            .composer
            .y
            .saturating_add(screen_row)
            .min(layout.composer.bottom().saturating_sub(1));
        frame.set_cursor_position((x, y));
    }
}

fn render_chat_workspace_blocks(app: &App) -> Vec<RenderedTranscriptBlock> {
    if app.chat_messages().is_empty() {
        return vec![RenderedTranscriptBlock {
            id: TranscriptBlockId::Empty,
            lines: vec![
                Line::from(""),
                Line::from(Span::styled(
                    "Ask Aleph to inspect local notes, memories, Trail, or workspace context.",
                    Style::default().fg(MUTED),
                )),
            ],
        }];
    }

    let messages = app.chat_messages();
    let mut blocks = Vec::new();
    let mut rendered_runs = Vec::new();
    for (index, message) in messages.iter().enumerate() {
        let mut message_lines = app.cached_transcript_message_lines(message, || {
            let mut lines = Vec::new();
            render_chat_message(&mut lines, message, app);
            lines
        });
        if !blocks.is_empty() {
            message_lines.insert(0, Line::from(""));
        }
        blocks.push(RenderedTranscriptBlock {
            id: TranscriptBlockId::Message(message.id),
            lines: message_lines,
        });

        let Some(run_id) = message.run_id else {
            continue;
        };
        let Some(run) = app.agent_run(run_id) else {
            continue;
        };

        if message.role == "user" && !rendered_runs.contains(&run_id) {
            rendered_runs.push(run_id);
            push_run_block(
                &mut blocks,
                run_id,
                TranscriptRunBlockKind::Context,
                |lines| render_run_context(lines, run),
            );
            push_run_block(
                &mut blocks,
                run_id,
                TranscriptRunBlockKind::Timeline,
                |lines| render_run_timeline(lines, run),
            );
            if run.approval.is_some() {
                push_run_block(
                    &mut blocks,
                    run_id,
                    TranscriptRunBlockKind::Approval,
                    |lines| render_run_approval(lines, run),
                );
            }
            if !run.changes.is_empty() {
                push_run_block(
                    &mut blocks,
                    run_id,
                    TranscriptRunBlockKind::Changes,
                    |lines| render_run_changes(lines, run),
                );
            }
        }

        let has_later_assistant = messages[index + 1..]
            .iter()
            .any(|later| later.run_id == Some(run_id) && later.role == "assistant");
        if message.role == "assistant" && !has_later_assistant && run.outcome.is_some() {
            push_run_block(
                &mut blocks,
                run_id,
                TranscriptRunBlockKind::Outcome,
                |lines| render_run_outcome(lines, run),
            );
        }
    }
    blocks
}

fn push_run_block(
    blocks: &mut Vec<RenderedTranscriptBlock>,
    run_id: u64,
    kind: TranscriptRunBlockKind,
    render: impl FnOnce(&mut Vec<Line<'static>>),
) {
    let mut lines = Vec::new();
    render(&mut lines);
    blocks.push(RenderedTranscriptBlock {
        id: TranscriptBlockId::Run { run_id, kind },
        lines,
    });
}

fn render_chat_message(lines: &mut Vec<Line<'static>>, message: &ChatMessage, app: &App) {
    if message.role == "user" {
        let mut content = message.content.lines();
        let first = content.next().unwrap_or("");
        lines.push(Line::from(vec![
            Span::styled(
                "❯ ",
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                first.to_string(),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("  ·  {}", message.timestamp),
                Style::default().fg(MUTED),
            ),
        ]));
        for extra in content {
            lines.push(Line::from(Span::styled(
                format!("  {}", extra),
                Style::default().fg(TEXT),
            )));
        }
        return;
    }

    let live = message.content.trim().is_empty() && (app.is_streaming() || app.is_thinking());
    lines.push(Line::from(vec![
        Span::styled(
            "◆ Aleph",
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            if live {
                format!("  ·  {}", app.thinking_status())
            } else if let Some(seconds) = message.thought_seconds {
                format!("  ·  thought for {:.1}s", seconds)
            } else {
                String::new()
            },
            Style::default().fg(MUTED).add_modifier(Modifier::ITALIC),
        ),
    ]));
    if !message.content.trim().is_empty() {
        lines.extend(App::render_chat_markdown_lines_owned(&message.content));
    }
}

fn render_run_context(lines: &mut Vec<Line<'static>>, run: &AgentRun) {
    lines.push(section_label("Context used"));
    lines.push(detail_line(
        "◌",
        format!(
            "{} scope · room {} · {} notes · {} memories",
            run.context.scope,
            run.context.room,
            run.context.notes_available,
            run.context.memories_available
        ),
    ));
    if let Some(note) = run.context.selected_note.as_deref() {
        lines.push(detail_line("◌", format!("selected note: {}", note)));
    }
    if !run.context.relevant_notes.is_empty() {
        lines.push(detail_line(
            "◌",
            format!("relevant notes: {}", run.context.relevant_notes.join(", ")),
        ));
    }
    if run.context.relevant_memories > 0 || run.context.relevant_trail_events > 0 {
        lines.push(detail_line(
            "◌",
            format!(
                "matched context: {} memories · {} Trail events",
                run.context.relevant_memories, run.context.relevant_trail_events
            ),
        ));
    }
    let provider = if run.context.provider_online {
        format!("{} online", run.context.provider)
    } else {
        format!(
            "{} offline · local notes, memories, Trail, and workspace tools remain available",
            run.context.provider
        )
    };
    lines.push(detail_line("◌", provider));
    if let Some(repository) = run.context.repository_summary.as_deref() {
        let source = match run.context.repository_source {
            RepositoryContextSource::Live => "live repository",
            RepositoryContextSource::Snapshot => "cached repository snapshot",
            RepositoryContextSource::Unavailable => "repository unavailable",
        };
        lines.push(detail_line("◌", format!("{}: {}", source, repository)));
    }
}

fn render_run_timeline(lines: &mut Vec<Line<'static>>, run: &AgentRun) {
    lines.push(section_label(format!(
        "Run · {}",
        run_phase_label(run.phase)
    )));
    if run.steps.is_empty() {
        let text = match run.phase {
            RunPhase::Planning => "Choosing the next action.",
            RunPhase::Streaming => "Synthesizing the response.",
            RunPhase::WaitingApproval => "Plan ready; waiting for permission.",
            _ => "No tool steps were required.",
        };
        lines.push(detail_line("·", text));
        return;
    }
    for step in &run.steps {
        let (glyph, color) = match step.status {
            StepStatus::Pending => ("○", MUTED),
            StepStatus::Running => ("●", ACCENT),
            StepStatus::Completed => ("✓", ACCENT_SOFT),
            StepStatus::Failed => ("×", Color::Rgb(190, 110, 125)),
        };
        let mut text = step.label.clone();
        if let Some(target) = step.target.as_deref() {
            text.push_str(&format!(" · {}", target));
        }
        if let Some(summary) = step.summary.as_deref() {
            text.push_str(&format!(" — {}", summary));
        }
        lines.push(Line::from(vec![
            Span::styled(format!("  {} ", glyph), Style::default().fg(color)),
            Span::styled(text, Style::default().fg(TEXT)),
        ]));
        if let Some(error) = step.error.as_deref() {
            lines.push(detail_line("  ×", format!("failed: {}", error)));
        }
    }
}

fn render_run_approval(lines: &mut Vec<Line<'static>>, run: &AgentRun) {
    let Some(approval) = run.approval.as_ref() else {
        return;
    };
    lines.push(section_label("Permission required"));
    lines.push(detail_line(
        "!",
        format!("{} · {}", approval.operation, approval.target),
    ));
    lines.push(detail_line(" ", approval.effect.clone()));
    lines.push(detail_line(" ", "Nothing has changed yet."));
    lines.push(detail_line(
        " ",
        "Press Enter or type `yes` to approve; type `no` or press Esc to reject.",
    ));
}

fn render_run_changes(lines: &mut Vec<Line<'static>>, run: &AgentRun) {
    if run.changes.is_empty() {
        return;
    }
    lines.push(section_label("Changes"));
    for change in &run.changes {
        let status = match change.status {
            ChangeStatus::Proposed => "proposed",
            ChangeStatus::Applied => "applied",
            ChangeStatus::Rejected => "rejected",
            ChangeStatus::Failed => "failed",
        };
        lines.push(detail_line(
            "◇",
            format!("{} · {} — {}", status, change.target, change.summary),
        ));
    }
}

fn render_run_outcome(lines: &mut Vec<Line<'static>>, run: &AgentRun) {
    let Some(outcome) = run.outcome.as_ref() else {
        return;
    };
    lines.push(section_label("Outcome"));
    match outcome {
        RunOutcome::Completed { summary } => {
            lines.push(detail_line("✓", summary.clone()));
            if run.changes.is_empty() {
                lines.push(detail_line(" ", "No changes were made."));
            }
            lines.push(detail_line(
                "→",
                "You can ask Aleph to continue or inspect another target.",
            ));
        }
        RunOutcome::Failed { error } => {
            lines.push(detail_line("×", format!("Run failed: {}", error)));
            lines.push(detail_line(
                "→",
                "Review the failed step, then retry or change the request.",
            ));
        }
        RunOutcome::Cancelled { reason } => {
            lines.push(detail_line("×", format!("Run cancelled: {}", reason)));
            lines.push(detail_line(
                "→",
                "Nothing was applied. You can revise the request.",
            ));
        }
    }
}

fn section_label(label: impl Into<String>) -> Line<'static> {
    Line::from(Span::styled(
        format!("  {}", label.into()),
        Style::default().fg(MUTED).add_modifier(Modifier::BOLD),
    ))
}

fn detail_line(glyph: &str, text: impl Into<String>) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("  {} ", glyph), Style::default().fg(ACCENT_SOFT)),
        Span::styled(text.into(), Style::default().fg(TEXT)),
    ])
}

fn run_phase_label(phase: RunPhase) -> &'static str {
    match phase {
        RunPhase::Planning => "planning",
        RunPhase::Acting => "acting",
        RunPhase::WaitingApproval => "waiting for approval",
        RunPhase::Streaming => "streaming",
        RunPhase::Completed => "completed",
        RunPhase::Failed => "failed",
        RunPhase::Cancelled => "cancelled",
    }
}

fn wrap_lines_to_width(lines: Vec<Line<'static>>, width: usize) -> Vec<Line<'static>> {
    let width = width.saturating_sub(1).max(1);
    let mut wrapped = Vec::new();

    for line in lines {
        let mut current_spans = Vec::new();
        let mut current_width = 0usize;

        for span in line.spans {
            let style = span.style;
            if span.content.trim().is_empty() {
                if !current_spans.is_empty() {
                    current_spans.push(Span::styled(" ", style));
                    current_width = current_width.saturating_add(1);
                }
                continue;
            }

            let mut words = span.content.split_whitespace().peekable();
            while let Some(word) = words.next() {
                let word_width = UnicodeWidthStr::width(word);
                let needs_space = !current_spans.is_empty() && current_width > 0;
                let projected = current_width + word_width + usize::from(needs_space);

                if projected > width && !current_spans.is_empty() {
                    wrapped.push(Line::from(std::mem::take(&mut current_spans)));
                    current_width = 0;
                }

                if current_width > 0 {
                    current_spans.push(Span::styled(" ", style));
                    current_width += 1;
                }

                if word_width <= width {
                    current_spans.push(Span::styled(word.to_string(), style));
                    current_width += word_width;
                } else {
                    for grapheme in word.graphemes(true) {
                        let grapheme_width = UnicodeWidthStr::width(grapheme).max(1);
                        if current_width + grapheme_width > width && !current_spans.is_empty() {
                            wrapped.push(Line::from(std::mem::take(&mut current_spans)));
                            current_width = 0;
                        }
                        current_spans.push(Span::styled(grapheme.to_string(), style));
                        current_width += grapheme_width;
                    }
                }

                if words.peek().is_some() && current_width < width {
                    current_spans.push(Span::styled(" ", style));
                    current_width += 1;
                }
            }
        }

        wrapped.push(Line::from(current_spans));
    }

    wrapped
}

fn chat_console_mode(app: &App) -> &'static str {
    if let Some(run) = app.active_agent_run() {
        return match run.phase {
            RunPhase::Planning => "Planning",
            RunPhase::Acting => "Acting",
            RunPhase::WaitingApproval => "Approval",
            RunPhase::Streaming => "Streaming",
            RunPhase::Completed => "Completed",
            RunPhase::Failed => "Failed",
            RunPhase::Cancelled => "Cancelled",
        };
    }
    if app.is_agent_mode_enabled() {
        "Agent"
    } else {
        "Chat"
    }
}

fn composer_mode_label(app: &App) -> &'static str {
    if let Some(run) = app
        .active_agent_run()
        .filter(|run| !run.phase.is_terminal())
    {
        return match run.phase {
            RunPhase::Planning => "Plan",
            RunPhase::Acting | RunPhase::Streaming => "Act",
            RunPhase::WaitingApproval => "Approval",
            RunPhase::Completed | RunPhase::Failed | RunPhase::Cancelled => unreachable!(),
        };
    }
    if app.is_agent_mode_enabled() {
        "Ask"
    } else {
        "Chat"
    }
}

pub(super) fn render_settings_panel(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .title(Span::styled(
            app.panel_title(),
            Style::default()
                .fg(app.room_accent())
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(BORDER))
        .style(Style::default().bg(PANEL));
    let inner = block.inner(area);
    frame.render_widget(Clear, area);
    frame.render_widget(block, area);

    let sections = settings_panel_sections(inner);

    let header = Paragraph::new(vec![Line::from(vec![Span::styled(
        "Manage your connections, room scope, and preferences",
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
            "Room Scope".to_string(),
            format!("{} ({})", app.active_room_label(), app.room_scope_summary()),
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
            "Agent Context".to_string(),
            format!("{} (Enter to cycle)", app.agent_context_scope_label()),
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
