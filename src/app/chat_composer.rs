use super::{ChatComposerState, ComposerInteraction};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ComposerVisualLine {
    pub start: usize,
    pub end: usize,
    pub width: usize,
    pub hard_break: bool,
}

impl ChatComposerState {
    pub fn buffer(&self) -> &str {
        &self.buffer
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn viewport_row(&self) -> usize {
        self.viewport_row
    }

    pub fn interaction(&self) -> ComposerInteraction {
        self.interaction
    }

    pub fn notice(&self) -> Option<&str> {
        self.notice.as_deref()
    }

    pub fn hovered(&self) -> bool {
        self.hovered
    }

    pub(crate) fn set_hovered(&mut self, hovered: bool) {
        self.hovered = hovered;
    }

    pub(crate) fn set_interaction(&mut self, interaction: ComposerInteraction) {
        self.interaction = interaction;
    }

    pub(crate) fn set_notice(&mut self, notice: impl Into<String>) {
        self.notice = Some(notice.into());
    }

    pub(crate) fn clear_notice(&mut self) {
        self.notice = None;
    }

    #[cfg(test)]
    pub(crate) fn set_text(&mut self, text: impl Into<String>) {
        self.buffer = text.into();
        self.cursor = self.buffer.len();
        self.preferred_display_column = None;
        self.viewport_row = 0;
        self.notice = None;
    }

    pub(crate) fn clear(&mut self) {
        self.buffer.clear();
        self.cursor = 0;
        self.preferred_display_column = None;
        self.viewport_row = 0;
        self.notice = None;
    }

    pub(crate) fn insert_char(&mut self, character: char) {
        self.buffer.insert(self.cursor, character);
        self.cursor += character.len_utf8();
        self.after_horizontal_edit();
    }

    pub(crate) fn insert_text(&mut self, text: &str) {
        self.buffer.insert_str(self.cursor, text);
        self.cursor += text.len();
        self.after_horizontal_edit();
    }

    pub(crate) fn backspace(&mut self) {
        if let Some(previous) = previous_grapheme_boundary(&self.buffer, self.cursor) {
            self.buffer.drain(previous..self.cursor);
            self.cursor = previous;
            self.after_horizontal_edit();
        }
    }

    pub(crate) fn delete(&mut self) {
        if let Some(next) = next_grapheme_boundary(&self.buffer, self.cursor) {
            self.buffer.drain(self.cursor..next);
            self.after_horizontal_edit();
        }
    }

    pub(crate) fn move_left(&mut self) {
        if let Some(previous) = previous_grapheme_boundary(&self.buffer, self.cursor) {
            self.cursor = previous;
            self.preferred_display_column = None;
        }
    }

    pub(crate) fn move_right(&mut self) {
        if let Some(next) = next_grapheme_boundary(&self.buffer, self.cursor) {
            self.cursor = next;
            self.preferred_display_column = None;
        }
    }

    pub(crate) fn move_home(&mut self, width: usize) {
        let lines = self.visual_lines(width);
        let row = visual_row_for_cursor(&lines, self.cursor);
        self.cursor = lines[row].start;
        self.preferred_display_column = None;
    }

    pub(crate) fn move_end(&mut self, width: usize) {
        let lines = self.visual_lines(width);
        let row = visual_row_for_cursor(&lines, self.cursor);
        self.cursor = lines[row].end;
        self.preferred_display_column = None;
    }

    pub(crate) fn move_vertical(&mut self, delta: isize, width: usize) {
        let lines = self.visual_lines(width);
        let row = visual_row_for_cursor(&lines, self.cursor);
        let (_, current_column) = self.cursor_visual_position_from_lines(&lines);
        let preferred = self.preferred_display_column.unwrap_or(current_column);
        let target_row = row.saturating_add_signed(delta).min(lines.len() - 1);
        if target_row == row {
            return;
        }
        self.cursor = byte_at_display_column(&self.buffer, lines[target_row], preferred);
        self.preferred_display_column = Some(preferred);
    }

    pub(crate) fn visual_lines(&self, width: usize) -> Vec<ComposerVisualLine> {
        visual_lines(&self.buffer, width.max(1))
    }

    pub(crate) fn cursor_visual_position(&self, width: usize) -> (usize, usize) {
        self.cursor_visual_position_from_lines(&self.visual_lines(width))
    }

    pub(crate) fn ensure_cursor_visible(&mut self, width: usize, visible_rows: usize) {
        let lines = self.visual_lines(width);
        let cursor_row = visual_row_for_cursor(&lines, self.cursor);
        let visible_rows = visible_rows.max(1);
        if cursor_row < self.viewport_row {
            self.viewport_row = cursor_row;
        } else if cursor_row >= self.viewport_row + visible_rows {
            self.viewport_row = cursor_row + 1 - visible_rows;
        }
        self.viewport_row = self
            .viewport_row
            .min(lines.len().saturating_sub(visible_rows));
    }

    fn cursor_visual_position_from_lines(&self, lines: &[ComposerVisualLine]) -> (usize, usize) {
        let row = visual_row_for_cursor(lines, self.cursor);
        let line = lines[row];
        let column = UnicodeWidthStr::width(&self.buffer[line.start..self.cursor.min(line.end)]);
        (row, column)
    }

    fn after_horizontal_edit(&mut self) {
        self.preferred_display_column = None;
        self.notice = None;
    }
}

fn visual_lines(buffer: &str, width: usize) -> Vec<ComposerVisualLine> {
    let mut lines = Vec::new();
    let mut start = 0;
    let mut row_width = 0;

    for (index, grapheme) in buffer.grapheme_indices(true) {
        if grapheme == "\n" {
            lines.push(ComposerVisualLine {
                start,
                end: index,
                width: row_width,
                hard_break: true,
            });
            start = index + grapheme.len();
            row_width = 0;
            continue;
        }

        let grapheme_width = UnicodeWidthStr::width(grapheme);
        if row_width > 0 && row_width + grapheme_width > width {
            lines.push(ComposerVisualLine {
                start,
                end: index,
                width: row_width,
                hard_break: false,
            });
            start = index;
            row_width = 0;
        }
        row_width += grapheme_width;
    }

    lines.push(ComposerVisualLine {
        start,
        end: buffer.len(),
        width: row_width,
        hard_break: false,
    });
    if row_width >= width && !buffer.ends_with('\n') {
        lines.push(ComposerVisualLine {
            start: buffer.len(),
            end: buffer.len(),
            width: 0,
            hard_break: false,
        });
    }
    lines
}

fn visual_row_for_cursor(lines: &[ComposerVisualLine], cursor: usize) -> usize {
    for (index, line) in lines.iter().enumerate() {
        if cursor < line.end
            || (cursor == line.end && (line.hard_break || index + 1 == lines.len()))
        {
            return index;
        }
    }
    lines.len().saturating_sub(1)
}

fn byte_at_display_column(buffer: &str, line: ComposerVisualLine, column: usize) -> usize {
    let mut byte = line.start;
    let mut display = 0;
    for (relative, grapheme) in buffer[line.start..line.end].grapheme_indices(true) {
        let next_display = display + UnicodeWidthStr::width(grapheme);
        if next_display > column {
            break;
        }
        display = next_display;
        byte = line.start + relative + grapheme.len();
    }
    byte
}

fn previous_grapheme_boundary(buffer: &str, cursor: usize) -> Option<usize> {
    buffer[..cursor]
        .grapheme_indices(true)
        .map(|(index, _)| index)
        .next_back()
}

fn next_grapheme_boundary(buffer: &str, cursor: usize) -> Option<usize> {
    buffer[cursor..]
        .grapheme_indices(true)
        .next()
        .map(|(_, grapheme)| cursor + grapheme.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf8_editing_uses_grapheme_boundaries() {
        let mut composer = ChatComposerState::default();
        composer.insert_text("A界e\u{301}🙂");

        composer.move_left();
        let before_emoji = composer.cursor();
        assert_eq!(&composer.buffer()[before_emoji..], "🙂");
        composer.backspace();
        assert_eq!(composer.buffer(), "A界🙂");
        composer.delete();
        assert_eq!(composer.buffer(), "A界");

        composer.move_left();
        assert_eq!(composer.cursor(), 1);
        composer.move_right();
        assert_eq!(composer.cursor(), composer.buffer().len());
    }

    #[test]
    fn visual_rows_use_terminal_width_and_hard_wrap_tokens() {
        let mut composer = ChatComposerState::default();
        composer.set_text("ab界cd\n🙂x");

        let lines = composer.visual_lines(4);
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0].width, 4);
        assert_eq!(&composer.buffer()[lines[0].start..lines[0].end], "ab界");
        assert_eq!(&composer.buffer()[lines[2].start..lines[2].end], "🙂x");

        composer.set_text("ab界");
        let exact = composer.visual_lines(4);
        assert_eq!(exact.len(), 2);
        assert_eq!(composer.cursor_visual_position(4), (1, 0));
    }

    #[test]
    fn vertical_movement_preserves_preferred_display_column() {
        let mut composer = ChatComposerState::default();
        composer.set_text("abcd\nx\nabcdef");
        composer.cursor = 3;

        composer.move_vertical(1, 20);
        assert_eq!(composer.cursor(), 6);
        assert_eq!(composer.preferred_display_column, Some(3));
        composer.move_vertical(1, 20);
        assert_eq!(&composer.buffer()[..composer.cursor()], "abcd\nx\nabc");
        composer.move_vertical(-1, 20);
        composer.move_vertical(-1, 20);
        assert_eq!(composer.cursor(), 3);
    }

    #[test]
    fn viewport_tracks_cursor_after_six_visible_rows() {
        let mut composer = ChatComposerState::default();
        composer.set_text("0\n1\n2\n3\n4\n5\n6\n7");
        composer.ensure_cursor_visible(20, 6);
        assert_eq!(composer.viewport_row(), 2);

        composer.move_vertical(-7, 20);
        composer.ensure_cursor_visible(20, 6);
        assert_eq!(composer.viewport_row(), 0);
    }
}
