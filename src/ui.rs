use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table};

use crate::app::App;

const BG: Color = Color::Rgb(10, 10, 12);
const ACCENT: Color = Color::Rgb(200, 146, 88);
const ACCENT_SOFT: Color = Color::Rgb(150, 110, 66);
const TEXT: Color = Color::Rgb(236, 236, 238);
const MUTED: Color = Color::Rgb(136, 136, 144);
const MUTED_SOFT: Color = Color::Rgb(90, 98, 98);
const PANEL: Color = Color::Rgb(22, 28, 28);
const BORDER: Color = Color::Rgb(34, 65, 64);

pub fn draw(frame: &mut Frame, app: &App) {
    let area = frame.area();

    frame.render_widget(
        Block::default().style(Style::default().bg(BG)),
        area,
    );

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
        .constraints([Constraint::Length(22), Constraint::Min(0)])
        .split(root[0]);

    let emblem = Paragraph::new(vec![
        Line::from(vec![
            Span::raw("        "),
            Span::styled("    ╭╲╱╲╮", Style::default().fg(ACCENT)),
        ]),
        Line::from(vec![
            Span::raw("      "),
            Span::styled("   ╱╲  ╱╲", Style::default().fg(ACCENT)),
        ]),
        Line::from(vec![
            Span::raw("    "),
            Span::styled("  ╱  ╲╱  ╲", Style::default().fg(ACCENT)),
        ]),
        Line::from(vec![
            Span::raw("   "),
            Span::styled(" ╱ ╭ℵ╮ ╲", Style::default().fg(ACCENT)),
        ]),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("╱ ╱╯ ╰╲ ╲", Style::default().fg(ACCENT)),
        ]),
        Line::from(vec![
            Span::raw("    "),
            Span::styled("╲╱  ╳  ╲╱", Style::default().fg(ACCENT)),
        ]),
        Line::from(vec![
            Span::raw("      "),
            Span::styled("   ╲╱╲╱", Style::default().fg(ACCENT)),
        ]),
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
        Span::styled("Tab", Style::default().fg(ACCENT)),
        Span::raw(" to autocomplete, "),
        Span::styled("↑/↓", Style::default().fg(ACCENT)),
        Span::raw(" cycle slash commands, "),
        Span::styled("Enter", Style::default().fg(ACCENT)),
        Span::raw(" run selected command, "),
        Span::styled("Ctrl+C", Style::default().fg(ACCENT)),
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
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(app.prompt_before_cursor(), Style::default().fg(TEXT)),
        Span::styled("█", Style::default().fg(ACCENT)),
        Span::styled(app.prompt_after_cursor(), Style::default().fg(TEXT)),
    ]))
    .alignment(Alignment::Left)
    .block(Block::default().borders(Borders::BOTTOM).border_style(Style::default().fg(BORDER)));
    frame.render_widget(prompt_block, input_row[0]);

    let command_hint = if app.is_thinking() {
        Paragraph::new(Line::from(vec![
            Span::styled(
                app.thinking_frame(),
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" thinking"),
        ]))
        .style(Style::default().fg(MUTED))
        .alignment(Alignment::Right)
    } else {
        Paragraph::new(Line::from(vec![
            Span::styled("/login", Style::default().fg(ACCENT)),
            Span::raw(" "),
            Span::styled("/status", Style::default().fg(ACCENT)),
            Span::raw(" "),
            Span::styled("/search", Style::default().fg(ACCENT)),
        ]))
        .style(Style::default().fg(MUTED))
        .alignment(Alignment::Right)
    };
    frame.render_widget(command_hint, input_row[1]);

    let suggestions = app.visible_commands(8);
    let remaining = app.total_command_matches().saturating_sub(suggestions.len());
    let rows = suggestions
        .iter()
        .enumerate()
        .map(|(index, command)| {
            let selected = index == app.selected_suggestion();
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
                    Style::default().fg(MUTED_SOFT),
                )),
                Cell::from(Span::styled("", Style::default())),
            ])
        }))
        .collect::<Vec<_>>();

    let suggestions_table = Table::new(rows, [Constraint::Length(26), Constraint::Min(10)])
        .column_spacing(3)
        .style(Style::default().fg(Color::Rgb(122, 122, 128)));
    frame.render_widget(suggestions_table, root[4]);
}