use ratatui::prelude::*;

use crate::app::CursorStyle;

use super::{ACCENT, ACCENT_SOFT, CURSOR, EDITOR_MUTED, EDITOR_TEXT};

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
                spans.push(Span::styled(
                    &remaining[..end_pos],
                    Style::default().add_modifier(Modifier::BOLD),
                ));
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
                spans.push(Span::styled(
                    &remaining[..end_pos],
                    Style::default().add_modifier(Modifier::ITALIC),
                ));
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
                spans.push(Span::styled(
                    &remaining[..end_pos],
                    Style::default().fg(ACCENT_SOFT),
                ));
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

pub(super) fn render_markdown_line(line: &str) -> Line<'_> {
    let mut spans = Vec::new();
    let mut remaining = line;
    let trimmed = line.trim_start();
    let indent_len = line.len() - trimmed.len();

    if trimmed.starts_with("# ") {
        spans.push(Span::styled(
            &line[..indent_len + 2],
            Style::default().fg(ACCENT_SOFT),
        ));
        spans.push(Span::styled(
            &trimmed[2..],
            Style::default()
                .fg(EDITOR_TEXT)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        ));
        return Line::from(spans);
    } else if trimmed.starts_with("## ") {
        spans.push(Span::styled(
            &line[..indent_len + 3],
            Style::default().fg(EDITOR_MUTED),
        ));
        spans.push(Span::styled(
            &trimmed[3..],
            Style::default()
                .fg(EDITOR_TEXT)
                .add_modifier(Modifier::BOLD),
        ));
        return Line::from(spans);
    } else if trimmed.starts_with("### ") {
        spans.push(Span::styled(
            &line[..indent_len + 4],
            Style::default().fg(EDITOR_MUTED),
        ));
        spans.push(Span::styled(
            &trimmed[4..],
            Style::default()
                .fg(EDITOR_TEXT)
                .add_modifier(Modifier::BOLD | Modifier::ITALIC),
        ));
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
            spans.push(Span::styled(
                &trimmed[..=pos + 1],
                Style::default().fg(ACCENT),
            ));
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
                spans.push(Span::styled(
                    &remaining[..end_pos],
                    Style::default().add_modifier(Modifier::BOLD),
                ));
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
                spans.push(Span::styled(
                    &remaining[..end_pos],
                    Style::default().add_modifier(Modifier::ITALIC),
                ));
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
                spans.push(Span::styled(
                    &remaining[..end_pos],
                    Style::default().fg(ACCENT_SOFT),
                ));
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

pub(super) fn render_markdown_line_with_cursor(
    line: &str,
    cursor_in_line: usize,
    cursor_style: CursorStyle,
) -> Line<'_> {
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
        before_cursor.push(Span::styled(cursor_glyph, Style::default().fg(EDITOR_TEXT)));
        before_cursor.extend(parse_markdown_spans(&line[cursor_in_line..]));
        return Line::from(before_cursor);
    }

    // Apply prefix styling up to text_start.
    let mut remaining = line;
    let mut prefix_spans = Vec::new();

    if text_start > 0 {
        if trimmed.starts_with("# ") {
            prefix_spans.push(Span::styled(
                &line[..text_start],
                Style::default().fg(ACCENT_SOFT),
            ));
        } else if trimmed.starts_with("## ") || trimmed.starts_with("### ") {
            prefix_spans.push(Span::styled(
                &line[..text_start],
                Style::default().fg(EDITOR_MUTED),
            ));
        } else if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
            if indent_len > 0 {
                prefix_spans.push(Span::raw(&line[..indent_len]));
            }
            prefix_spans.push(Span::styled("• ", Style::default().fg(ACCENT)));
        } else if let Some(pos) = trimmed.find(". ") {
            if indent_len > 0 {
                prefix_spans.push(Span::raw(&line[..indent_len]));
            }
            prefix_spans.push(Span::styled(
                &trimmed[..=pos + 1],
                Style::default().fg(ACCENT),
            ));
        }
        remaining = &line[text_start..];
    }

    // Normal markdown parsing for the rest, injecting the cursor.
    let adjusted_cursor = cursor_in_line - text_start;
    spans.extend(prefix_spans);

    let before_cursor = &remaining[..adjusted_cursor];
    let after_cursor = &remaining[adjusted_cursor..];

    if trimmed.starts_with("# ") || trimmed.starts_with("## ") || trimmed.starts_with("### ") {
        spans.push(Span::styled(
            before_cursor,
            Style::default()
                .fg(EDITOR_TEXT)
                .add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled(cursor_glyph, Style::default().fg(EDITOR_TEXT)));
        spans.push(Span::styled(
            after_cursor,
            Style::default()
                .fg(EDITOR_TEXT)
                .add_modifier(Modifier::BOLD),
        ));
    } else {
        spans.extend(parse_markdown_spans(before_cursor));
        spans.push(Span::styled(cursor_glyph, Style::default().fg(EDITOR_TEXT)));
        spans.extend(parse_markdown_spans(after_cursor));
    }

    Line::from(spans)
}
