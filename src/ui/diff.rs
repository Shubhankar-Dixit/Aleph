use super::*;

/// Represents a line in a diff view
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DiffLineType {
    Unchanged,
    Added,
    Removed,
}

/// Compute a simple line-based diff between original and proposed text
/// Returns a Vec of (line_text, line_type) tuples
pub(super) fn compute_line_diff<'a>(
    original: &'a str,
    proposed: &'a str,
) -> Vec<(&'a str, DiffLineType)> {
    let original_lines: Vec<&str> = original.lines().collect();
    let proposed_lines: Vec<&str> = proposed.lines().collect();

    let mut result = Vec::new();
    let mut orig_idx = 0;
    let mut prop_idx = 0;

    while orig_idx < original_lines.len() || prop_idx < proposed_lines.len() {
        match (original_lines.get(orig_idx), proposed_lines.get(prop_idx)) {
            (Some(orig), Some(prop)) => {
                if orig == prop {
                    result.push((*orig, DiffLineType::Unchanged));
                    orig_idx += 1;
                    prop_idx += 1;
                } else {
                    // Check if this line was replaced or just added/removed
                    // Simple heuristic: if next proposed line matches current original, this is a removal
                    if orig_idx + 1 < original_lines.len()
                        && original_lines.get(orig_idx + 1) == Some(prop)
                    {
                        result.push((*orig, DiffLineType::Removed));
                        orig_idx += 1;
                    } else if prop_idx + 1 < proposed_lines.len()
                        && proposed_lines.get(prop_idx + 1) == Some(orig)
                    {
                        result.push((*prop, DiffLineType::Added));
                        prop_idx += 1;
                    } else {
                        // Treat as replacement: show removed then added
                        result.push((*orig, DiffLineType::Removed));
                        result.push((*prop, DiffLineType::Added));
                        orig_idx += 1;
                        prop_idx += 1;
                    }
                }
            }
            (Some(orig), None) => {
                result.push((*orig, DiffLineType::Removed));
                orig_idx += 1;
            }
            (None, Some(prop)) => {
                result.push((*prop, DiffLineType::Added));
                prop_idx += 1;
            }
            (None, None) => break,
        }
    }

    result
}

/// Render a diff line with appropriate styling
pub(super) fn render_diff_line(line_text: &str, line_type: DiffLineType) -> Line<'_> {
    match line_type {
        DiffLineType::Unchanged => render_markdown_line(line_text),
        DiffLineType::Added => {
            let style = Style::default().fg(DIFF_ADDED_FG).bg(DIFF_ADDED_BG);
            Line::from(vec![
                Span::styled("+ ", style.add_modifier(Modifier::BOLD)),
                Span::styled(line_text, style),
            ])
        }
        DiffLineType::Removed => {
            let style = Style::default()
                .fg(DIFF_REMOVED_FG)
                .bg(DIFF_REMOVED_BG)
                .add_modifier(Modifier::CROSSED_OUT);
            Line::from(vec![
                Span::styled("- ", style.add_modifier(Modifier::BOLD)),
                Span::styled(line_text, style),
            ])
        }
    }
}
