use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Tabs, Wrap};

use crate::app::App;

pub fn draw(frame: &mut Frame, app: &App) {
    let area = frame.area();

    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(4),
            Constraint::Length(1),
        ])
        .margin(1)
        .split(area);

    let header = Paragraph::new(Line::from(vec![
        Span::styled("ALEPH", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::raw("  terminal and agent runtime for Strix"),
    ]))
    .block(Block::default().borders(Borders::ALL).title("boot"));
    frame.render_widget(header, root[0]);

    let status_line = Paragraph::new(Line::from(vec![
        Span::styled("status ", Style::default().fg(Color::DarkGray)),
        Span::styled("ready", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
        Span::raw("  "),
        Span::styled(app.spinner(), Style::default().fg(Color::Yellow)),
        Span::raw("  "),
        Span::styled(format!("uptime {}", app.uptime()), Style::default().fg(Color::Gray)),
        Span::raw("  "),
        Span::styled(
            format!("tick {}", app.tick()),
            Style::default().fg(Color::DarkGray),
        ),
    ]))
    .block(Block::default().borders(Borders::ALL).title("runtime"));
    frame.render_widget(status_line, root[1]);

    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(62), Constraint::Percentage(38)])
        .split(root[2]);

    let left = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(5), Constraint::Min(0)])
        .split(body[0]);

    let tabs = Tabs::new(vec![
        Line::from("Home"),
        Line::from("Search"),
        Line::from("Notes"),
        Line::from("Memories"),
        Line::from("Canvas"),
        Line::from("Darwin"),
    ])
    .select(0)
    .block(Block::default().borders(Borders::ALL).title("modes"))
    .style(Style::default().fg(Color::Gray))
    .highlight_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD));
    frame.render_widget(tabs, left[0]);

    let intro = Paragraph::new(vec![
        Line::from("Base TUI scaffold for Aleph."),
        Line::from(""),
        Line::from("This shell proves the terminal surface and leaves room for CLI, MCP, and gateway work."),
    ])
    .wrap(Wrap { trim: true })
    .block(Block::default().borders(Borders::ALL).title("welcome"));
    frame.render_widget(intro, left[1]);

    let right = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(6), Constraint::Min(0)])
        .split(body[1]);

    let runtime_items = List::new(vec![
        ListItem::new("- transport: terminal"),
        ListItem::new("- ui: ratatui"),
        ListItem::new("- backend: not wired"),
        ListItem::new("- mcp: pending"),
        ListItem::new("- cache: pending"),
    ])
    .block(Block::default().borders(Borders::ALL).title("surface"));
    frame.render_widget(runtime_items, right[0]);

    let checklist = Paragraph::new(vec![
        Line::from(vec![
            Span::styled("[x]", Style::default().fg(Color::Green)),
            Span::raw(" terminal shell boots"),
        ]),
        Line::from(vec![
            Span::styled("[ ]", Style::default().fg(Color::DarkGray)),
            Span::raw(" command entry"),
        ]),
        Line::from(vec![
            Span::styled("[ ]", Style::default().fg(Color::DarkGray)),
            Span::raw(" gateway client"),
        ]),
        Line::from(vec![
            Span::styled("[ ]", Style::default().fg(Color::DarkGray)),
            Span::raw(" streaming output"),
        ]),
    ])
    .block(Block::default().borders(Borders::ALL).title("first milestones"));
    frame.render_widget(checklist, right[1]);

    let footer = Paragraph::new(Line::from(vec![
        Span::styled(
            "aleph >",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        ),
        Span::raw(" press q or Esc to quit"),
    ]))
    .block(Block::default().borders(Borders::ALL).title("command line"));
    frame.render_widget(footer, root[3]);

    let help = Paragraph::new(Line::from(vec![
        Span::styled("Tab", Style::default().fg(Color::Cyan)),
        Span::raw(" modes  "),
        Span::styled("Ctrl+R", Style::default().fg(Color::Cyan)),
        Span::raw(" history  "),
        Span::styled("Ctrl+J", Style::default().fg(Color::Cyan)),
        Span::raw(" output  "),
        Span::styled("q", Style::default().fg(Color::Cyan)),
        Span::raw(" quit"),
    ]));
    frame.render_widget(help, root[4]);
}