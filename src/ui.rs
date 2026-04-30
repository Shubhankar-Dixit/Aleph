use ratatui::prelude::*;
use ratatui::widgets::{
    Block, Borders, Cell, Clear, Paragraph, Row, Scrollbar, ScrollbarOrientation, ScrollbarState,
    Table, Wrap,
};

mod chat_settings;
mod diff;
mod editor;
mod markdown;
mod panels;

use crate::app::{AiProvider, App, PanelMode};
use chat_settings::{render_full_chat, render_obsidian_sync_confirm_panel, render_settings_panel};
use editor::render_full_editor;
use markdown::{
    render_markdown_line, render_markdown_line_with_cursor, render_markdown_line_with_selection,
    render_panel_markdown_line,
};
use self::panels::{
    render_commands_panel, render_note_editor_panel, render_note_list_panel,
    render_obsidian_vault_picker_panel,
};

const BG: Color = Color::Rgb(25, 26, 34);
const ACCENT: Color = Color::Rgb(156, 146, 201);
const ACCENT_SOFT: Color = Color::Rgb(115, 106, 155);
const TEXT: Color = Color::Rgb(198, 198, 210);
const MUTED: Color = Color::Rgb(120, 122, 138);
const EDITOR_TEXT: Color = Color::Rgb(130, 132, 145);
const EDITOR_MUTED: Color = Color::Rgb(98, 101, 116);
const EDITOR_SELECTION_BG: Color = Color::Rgb(60, 62, 78);
const DIFF_ADDED_FG: Color = Color::Rgb(120, 220, 140); // Green for additions
const DIFF_REMOVED_FG: Color = Color::Rgb(220, 100, 100); // Red for removals
const DIFF_ADDED_BG: Color = Color::Rgb(30, 60, 40); // Dark green background
const DIFF_REMOVED_BG: Color = Color::Rgb(60, 30, 35); // Dark red background
const PANEL: Color = Color::Rgb(35, 36, 48);
const BORDER: Color = Color::Rgb(34, 65, 64);
const GHOST_FRAMES: [&str; 4] = ["◌", "◎", "◍", "◉"];
const CURSOR: &str = "│";

pub fn draw(frame: &mut Frame, app: &App) {
    let area = frame.area();

    frame.render_widget(Block::default().style(Style::default().bg(BG)), area);

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
        Line::from(vec![Span::styled(
            "⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⣀⢀⢀⢀⡀",
            Style::default().fg(MUTED),
        )]),
        Line::from(vec![Span::styled(
            "⠀⠀⠀⠀⠀⠀⠀⠀⢀⠀⡴⠰⠞⠿⠛⠁⠓⠖⠲⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀",
            Style::default().fg(TEXT),
        )]),
        Line::from(vec![Span::styled(
            "⠀⠀⠀⠀⠀⠀⠀⢸⠆⢁⠶⠿⠇⠹⠁⠸⠷⠏⣈⡀⢰⠀⠈⠀⠀⠀⠀⠀⠀⠀",
            Style::default().fg(TEXT),
        )]),
        Line::from(vec![Span::styled(
            "⠀⠀⠀⠀⠀⠀⡁⠴⠛⢀⡀⠀⠀⢀⠀⠀⠀⠀⡀⠀⠀⠂⠄⠀⠀⠀⠀⠀⠀⠀",
            Style::default().fg(ACCENT),
        )]),
        Line::from(vec![Span::styled(
            "⠀⠀⠀⠀⠀⠠⠀⢠⣴⣿⠀⠄⠈⠉⠀⠀⢀⠀⢻⡗⠀⠀⠐⠡⣄⡀⠀⠀⠀⠀",
            Style::default().fg(ACCENT_SOFT),
        )]),
        Line::from(vec![Span::styled(
            "⠀⠀⠀⠀⠀⣤⠒⢺⣿⣿⣆⠙⠄⢤⠠⠔⠘⢢⣞⠋⠀⢀⣰⣧⣬⡇⠀⠀⠀⠀",
            Style::default().fg(TEXT),
        )]),
        Line::from(vec![Span::styled(
            "⠀⠀⠀⠀⠈⠪⡅⠲⢿⢽⣿⣿⣶⣶⣦⣶⣿⠇⠴⠋⠍⢉⣹⣿⠿⠀⠀⠀⠀⠀",
            Style::default().fg(TEXT),
        )]),
        Line::from(vec![Span::styled(
            "⠀⠀⠀⠀⠀⠀⠰⠆⠁⠀⢈⠉⠹⣹⠈⠁⠀⠆⢰⢆⢀⣾⣾⠉⠀⠀⠀⠀⠀⠀",
            Style::default().fg(MUTED),
        )]),
        Line::from(vec![Span::styled(
            "⠀⠀⠀⠀⠀⠀⠀⠀⠃⠷⠀⠄⣤⡀⠀⣠⠠⣤⠄⠼⠟⠉⠀⠀⠀⠀⠀⠀⠀⠀",
            Style::default().fg(MUTED),
        )]),
        Line::from(vec![Span::styled(
            "⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠉⠉⠁⠈⠀",
            Style::default().fg(MUTED),
        )]),
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
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
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
        Span::styled(">", Style::default().fg(TEXT).add_modifier(Modifier::BOLD)),
        Span::raw("  "),
        Span::styled(app.prompt_before_cursor(), Style::default().fg(TEXT)),
        Span::styled(CURSOR, Style::default().fg(MUTED)),
        Span::styled(app.prompt_after_cursor(), Style::default().fg(TEXT)),
    ]))
    .alignment(Alignment::Left)
    .block(
        Block::default()
            .borders(Borders::BOTTOM)
            .border_style(Style::default().fg(BORDER)),
    );
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
        // Show auth indicator subtly
        let mut hint_spans = vec![];

        if app.is_openrouter_connected() || app.is_strix_connected() {
            hint_spans.push(Span::styled("●", Style::default().fg(ACCENT)));
            hint_spans.push(Span::raw(" "));
        }

        hint_spans.push(Span::styled("/ask", Style::default().fg(TEXT)));
        hint_spans.push(Span::raw(" "));
        hint_spans.push(Span::styled("/note", Style::default().fg(MUTED)));

        Paragraph::new(Line::from(hint_spans))
            .style(Style::default().fg(MUTED))
            .alignment(Alignment::Right)
    };
    frame.render_widget(command_hint, input_row[1]);

    match app.panel_mode() {
        PanelMode::Commands | PanelMode::LoginPicker => render_commands_panel(frame, app, root[4]),
        PanelMode::VaultPicker => render_obsidian_vault_picker_panel(frame, app, root[4]),
        PanelMode::NoteList => render_note_list_panel(frame, app, root[4]),
        PanelMode::NoteEditor => render_note_editor_panel(frame, app, root[4]),
        PanelMode::Settings => render_settings_panel(frame, app, root[4]),
        PanelMode::ObsidianSyncConfirm => render_obsidian_sync_confirm_panel(frame, app, root[4]),
        PanelMode::FullEditor | PanelMode::AiChat => {}
    }
}

#[cfg(test)]
mod tests;
