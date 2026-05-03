use ratatui::prelude::*;

use crate::app::CursorStyle;

use super::{
    ACCENT, ACCENT_SOFT, CURSOR, EDITOR_MUTED, EDITOR_SELECTION_BG, EDITOR_TEXT, MUTED, TEXT,
};

#[derive(Clone, Copy)]
struct MarkdownTheme {
    text: Color,
    muted: Color,
}

const EDITOR_THEME: MarkdownTheme = MarkdownTheme {
    text: EDITOR_TEXT,
    muted: EDITOR_MUTED,
};

const PANEL_THEME: MarkdownTheme = MarkdownTheme {
    text: TEXT,
    muted: MUTED,
};

fn parse_markdown_spans(text: &str, theme: MarkdownTheme) -> Vec<Span<'_>> {
    let mut spans = Vec::new();
    let mut remaining = text;
    let base_style = Style::default().fg(theme.text);

    while !remaining.is_empty() {
        if let Some(pos) = remaining.find("**") {
            if pos > 0 {
                spans.push(Span::styled(&remaining[..pos], base_style));
            }
            remaining = &remaining[pos + 2..];
            if let Some(end_pos) = remaining.find("**") {
                spans.push(Span::styled(
                    &remaining[..end_pos],
                    base_style.add_modifier(Modifier::BOLD),
                ));
                remaining = &remaining[end_pos + 2..];
            } else {
                spans.push(Span::styled("**", base_style));
                spans.push(Span::styled(remaining, base_style));
                break;
            }
        } else if let Some(pos) = remaining.find('*') {
            if pos > 0 {
                spans.push(Span::styled(&remaining[..pos], base_style));
            }
            remaining = &remaining[pos + 1..];
            if let Some(end_pos) = remaining.find('*') {
                spans.push(Span::styled(
                    &remaining[..end_pos],
                    base_style.add_modifier(Modifier::ITALIC),
                ));
                remaining = &remaining[end_pos + 1..];
            } else {
                spans.push(Span::styled("*", base_style));
                spans.push(Span::styled(remaining, base_style));
                break;
            }
        } else if let Some(pos) = remaining.find('`') {
            if pos > 0 {
                spans.push(Span::styled(&remaining[..pos], base_style));
            }
            remaining = &remaining[pos + 1..];
            if let Some(end_pos) = remaining.find('`') {
                spans.push(Span::styled(
                    &remaining[..end_pos],
                    Style::default().fg(ACCENT_SOFT),
                ));
                remaining = &remaining[end_pos + 1..];
            } else {
                spans.push(Span::styled("`", base_style));
                spans.push(Span::styled(remaining, base_style));
                break;
            }
        } else {
            spans.push(Span::styled(remaining, base_style));
            break;
        }
    }

    if spans.is_empty() {
        spans.push(Span::styled(text, base_style));
    }

    spans
}

pub(super) fn render_markdown_line(line: &str) -> Line<'_> {
    render_markdown_line_with_theme(line, EDITOR_THEME)
}

pub(super) fn render_panel_markdown_line(line: &str) -> Line<'_> {
    render_markdown_line_with_theme(line, PANEL_THEME)
}

fn render_markdown_line_with_theme(line: &str, theme: MarkdownTheme) -> Line<'_> {
    let mut spans = Vec::new();
    let mut remaining = line;
    let trimmed = line.trim_start();
    let indent_len = line.len() - trimmed.len();
    let base_style = Style::default().fg(theme.text);

    if trimmed.starts_with("# ") {
        spans.push(Span::styled(
            &line[..indent_len + 2],
            Style::default().fg(ACCENT_SOFT),
        ));
        spans.push(Span::styled(
            &trimmed[2..],
            base_style.add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        ));
        return Line::from(spans);
    } else if trimmed.starts_with("## ") {
        spans.push(Span::styled(
            &line[..indent_len + 3],
            Style::default().fg(theme.muted),
        ));
        spans.push(Span::styled(
            &trimmed[3..],
            base_style.add_modifier(Modifier::BOLD),
        ));
        return Line::from(spans);
    } else if trimmed.starts_with("### ") {
        spans.push(Span::styled(
            &line[..indent_len + 4],
            Style::default().fg(theme.muted),
        ));
        spans.push(Span::styled(
            &trimmed[4..],
            base_style.add_modifier(Modifier::BOLD | Modifier::ITALIC),
        ));
        return Line::from(spans);
    } else if trimmed.starts_with('|') && trimmed.ends_with('|') {
        let pipe_count = trimmed.chars().filter(|&c| c == '|').count();
        if pipe_count >= 2 {
            let is_separator = trimmed
                .trim_start_matches('|')
                .trim_end_matches('|')
                .split('|')
                .all(|cell| {
                    cell.trim()
                        .chars()
                        .all(|c| c == '-' || c == ':' || c == ' ')
                });
            if is_separator {
                return Line::from(Span::styled(line, Style::default().fg(theme.muted)));
            }
            let mut table_spans = Vec::new();
            if indent_len > 0 {
                table_spans.push(Span::styled(&line[..indent_len], base_style));
            }
            let parts: Vec<&str> = trimmed.split('|').collect();
            for (i, part) in parts.iter().enumerate() {
                if i == 0 && part.is_empty() {
                    table_spans.push(Span::styled("|", Style::default().fg(theme.muted)));
                } else if i == parts.len() - 1 && part.is_empty() {
                    // trailing empty after last pipe
                } else {
                    table_spans.push(Span::styled(*part, base_style));
                    if i < parts.len() - 1 {
                        table_spans.push(Span::styled("|", Style::default().fg(theme.muted)));
                    }
                }
            }
            return Line::from(table_spans);
        }
        remaining = trimmed;
    } else if let Some(stripped) = trimmed.strip_prefix("- ") {
        if indent_len > 0 {
            spans.push(Span::styled(&line[..indent_len], base_style));
        }
        spans.push(Span::styled("• ", Style::default().fg(ACCENT)));
        remaining = stripped;
    } else if let Some(stripped) = trimmed.strip_prefix("* ") {
        if indent_len > 0 {
            spans.push(Span::styled(&line[..indent_len], base_style));
        }
        spans.push(Span::styled("• ", Style::default().fg(ACCENT)));
        remaining = stripped;
    } else if let Some(pos) = trimmed.find(". ") {
        if pos > 0 && trimmed[..pos].chars().all(|c| c.is_ascii_digit()) {
            if indent_len > 0 {
                spans.push(Span::styled(&line[..indent_len], base_style));
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
                spans.push(Span::styled(&remaining[..pos], base_style));
            }
            remaining = &remaining[pos + 2..];
            if let Some(end_pos) = remaining.find("**") {
                spans.push(Span::styled(
                    &remaining[..end_pos],
                    base_style.add_modifier(Modifier::BOLD),
                ));
                remaining = &remaining[end_pos + 2..];
            } else {
                spans.push(Span::styled("**", base_style));
                spans.push(Span::styled(remaining, base_style));
                break;
            }
        } else if let Some(pos) = remaining.find('*') {
            if pos > 0 {
                spans.push(Span::styled(&remaining[..pos], base_style));
            }
            remaining = &remaining[pos + 1..];
            if let Some(end_pos) = remaining.find('*') {
                spans.push(Span::styled(
                    &remaining[..end_pos],
                    base_style.add_modifier(Modifier::ITALIC),
                ));
                remaining = &remaining[end_pos + 1..];
            } else {
                spans.push(Span::styled("*", base_style));
                spans.push(Span::styled(remaining, base_style));
                break;
            }
        } else if let Some(pos) = remaining.find('`') {
            if pos > 0 {
                spans.push(Span::styled(&remaining[..pos], base_style));
            }
            remaining = &remaining[pos + 1..];
            if let Some(end_pos) = remaining.find('`') {
                spans.push(Span::styled(
                    &remaining[..end_pos],
                    Style::default().fg(ACCENT_SOFT),
                ));
                remaining = &remaining[end_pos + 1..];
            } else {
                spans.push(Span::styled("`", base_style));
                spans.push(Span::styled(remaining, base_style));
                break;
            }
        } else {
            spans.push(Span::styled(remaining, base_style));
            break;
        }
    }

    if spans.is_empty() {
        spans.push(Span::styled(line, base_style));
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
        let mut before_cursor = parse_markdown_spans(&line[..cursor_in_line], EDITOR_THEME);
        before_cursor.push(Span::styled(cursor_glyph, Style::default().fg(EDITOR_TEXT)));
        before_cursor.extend(parse_markdown_spans(&line[cursor_in_line..], EDITOR_THEME));
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
        spans.extend(parse_markdown_spans(before_cursor, EDITOR_THEME));
        spans.push(Span::styled(cursor_glyph, Style::default().fg(EDITOR_TEXT)));
        spans.extend(parse_markdown_spans(after_cursor, EDITOR_THEME));
    }

    Line::from(spans)
}

/// Render a markdown line with selection highlighting
/// selection_start and selection_end are byte positions relative to the start of the entire buffer
/// line_start is the byte position of the start of this line in the buffer
pub(super) fn render_markdown_line_with_selection<'a>(
    line: &'a str,
    cursor_in_line: usize,
    cursor_style: CursorStyle,
    selection: &crate::app::model::Selection,
    line_start: usize,
) -> Line<'a> {
    let cursor_glyph = if cursor_style == CursorStyle::Block {
        "█"
    } else {
        CURSOR
    };

    let line_end = line_start + line.len();
    let selection_in_line_start = selection.start.saturating_sub(line_start);
    let selection_in_line_end = (selection.end.saturating_sub(line_start)).min(line.len());

    // Check if there's any selection in this line
    let has_selection_in_line =
        selection.active && selection.end > line_start && selection.start < line_end;

    if !has_selection_in_line {
        // No selection in this line, just render with cursor
        return render_markdown_line_with_cursor(line, cursor_in_line, cursor_style);
    }

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

    // Handle prefix styling
    if text_start > 0 {
        if trimmed.starts_with("# ") {
            spans.push(Span::styled(
                &line[..text_start],
                Style::default().fg(ACCENT_SOFT),
            ));
        } else if trimmed.starts_with("## ") || trimmed.starts_with("### ") {
            spans.push(Span::styled(
                &line[..text_start],
                Style::default().fg(EDITOR_MUTED),
            ));
        } else if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
            if indent_len > 0 {
                spans.push(Span::raw(&line[..indent_len]));
            }
            spans.push(Span::styled("• ", Style::default().fg(ACCENT)));
        } else if let Some(pos) = trimmed.find(". ") {
            if indent_len > 0 {
                spans.push(Span::raw(&line[..indent_len]));
            }
            if pos > 0 && trimmed[..pos].chars().all(|c| c.is_ascii_digit()) {
                spans.push(Span::styled(
                    &trimmed[..=pos + 1],
                    Style::default().fg(ACCENT),
                ));
            }
        }
    }

    // Get the text portion to render
    let text_portion = if text_start > 0 {
        &line[text_start..]
    } else {
        line
    };

    let adjusted_cursor = cursor_in_line.saturating_sub(text_start);
    let adjusted_sel_start = selection_in_line_start.saturating_sub(text_start);
    let adjusted_sel_end = selection_in_line_end.saturating_sub(text_start);

    // Split text into segments: before selection, selection, after selection
    let sel_start = adjusted_sel_start.min(text_portion.len());
    let sel_end = adjusted_sel_end.min(text_portion.len());

    let before_sel = &text_portion[..sel_start];
    let selected_text = &text_portion[sel_start..sel_end];
    let after_sel = &text_portion[sel_end..];

    // Apply appropriate styles based on whether it's a heading
    let is_heading =
        trimmed.starts_with("# ") || trimmed.starts_with("## ") || trimmed.starts_with("### ");

    let base_style = if is_heading {
        Style::default()
            .fg(EDITOR_TEXT)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(EDITOR_TEXT)
    };

    let selected_style = if is_heading {
        Style::default()
            .fg(EDITOR_TEXT)
            .bg(EDITOR_SELECTION_BG)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(EDITOR_TEXT).bg(EDITOR_SELECTION_BG)
    };

    // Render before selection
    if !before_sel.is_empty() {
        if adjusted_cursor < sel_start {
            // Cursor is in before_sel
            let (before_cursor, after_cursor) = before_sel.split_at(adjusted_cursor);
            spans.extend(parse_markdown_spans(before_cursor, EDITOR_THEME));
            spans.push(Span::styled(cursor_glyph, base_style));
            spans.extend(parse_markdown_spans(after_cursor, EDITOR_THEME));
        } else {
            spans.extend(parse_markdown_spans(before_sel, EDITOR_THEME));
        }
    } else if adjusted_cursor == 0 && text_start == cursor_in_line {
        // Cursor at the very beginning
        spans.push(Span::styled(cursor_glyph, base_style));
    }

    // Render selected text
    if !selected_text.is_empty() {
        if adjusted_cursor >= sel_start && adjusted_cursor < sel_end {
            // Cursor is inside selection
            let cursor_rel = adjusted_cursor - sel_start;
            let (sel_before_cursor, sel_after_cursor) = selected_text.split_at(cursor_rel);
            spans.push(Span::styled(sel_before_cursor, selected_style));
            spans.push(Span::styled(cursor_glyph, selected_style));
            spans.push(Span::styled(sel_after_cursor, selected_style));
        } else {
            spans.push(Span::styled(selected_text, selected_style));
        }
    }

    // Render after selection
    if !after_sel.is_empty() {
        if adjusted_cursor >= sel_end {
            // Cursor is in after_sel
            let cursor_rel = adjusted_cursor - sel_end;
            let (after_cursor, rest) = after_sel.split_at(cursor_rel);
            spans.extend(parse_markdown_spans(after_cursor, EDITOR_THEME));
            spans.push(Span::styled(cursor_glyph, base_style));
            spans.extend(parse_markdown_spans(rest, EDITOR_THEME));
        } else {
            spans.extend(parse_markdown_spans(after_sel, EDITOR_THEME));
        }
    } else if adjusted_cursor == text_portion.len() {
        // Cursor at the end
        spans.push(Span::styled(cursor_glyph, base_style));
    }

    Line::from(spans)
}
