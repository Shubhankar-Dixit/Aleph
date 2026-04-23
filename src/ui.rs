use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table, Wrap};

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
    let block = Block::default()
        .title(Span::styled(
            format!("Editing: {}", title),
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(BORDER));
    let inner = block.inner(editor_area);
    frame.render_widget(block, editor_area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(inner);

    let mut editor_text = String::with_capacity(app.editor_buffer().len() + 1);
    let cursor = app.editor_cursor().min(app.editor_buffer().len());
    editor_text.push_str(&app.editor_buffer()[..cursor]);
    editor_text.push('█');
    editor_text.push_str(&app.editor_buffer()[cursor..]);

    frame.render_widget(
        Paragraph::new(editor_text)
            .style(Style::default().fg(TEXT))
            .wrap(Wrap { trim: false }),
        chunks[0],
    );

    let status = Paragraph::new(Line::from(vec![
        Span::styled("Ctrl+S", Style::default().fg(ACCENT)),
        Span::raw(" save  "),
        Span::styled("Esc", Style::default().fg(ACCENT_SOFT)),
        Span::raw(" save & exit  "),
        Span::styled("Ctrl+L", Style::default().fg(ACCENT)),
        Span::raw(" toggle AI"),
    ]))
    .style(Style::default().fg(MUTED));
    frame.render_widget(status, chunks[1]);

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
    let title = app
        .editor_note_title()
        .map(|note| format!("Editing: {}", note))
        .unwrap_or_else(|| String::from("Editing note"));

    let block = Block::default()
        .title(Span::styled(title, Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(BORDER));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(inner);

    let helper = Paragraph::new(Line::from(vec![
        Span::styled("Ctrl+S", Style::default().fg(ACCENT)),
        Span::raw(" save, "),
        Span::styled("Esc", Style::default().fg(ACCENT_SOFT)),
        Span::raw(" save & exit"),
    ]))
    .style(Style::default().fg(MUTED))
    .alignment(Alignment::Left);
    frame.render_widget(helper, chunks[0]);

    let mut editor_text = String::with_capacity(app.editor_buffer().len() + 1);
    let cursor = app.editor_cursor().min(app.editor_buffer().len());
    editor_text.push_str(&app.editor_buffer()[..cursor]);
    editor_text.push('█');
    editor_text.push_str(&app.editor_buffer()[cursor..]);

    frame.render_widget(
        Paragraph::new(editor_text)
            .style(Style::default().fg(TEXT))
            .wrap(Wrap { trim: false }),
        chunks[1],
    );
}