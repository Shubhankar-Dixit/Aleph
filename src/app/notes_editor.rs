use super::*;

#[allow(dead_code)]
impl App {
    pub(super) fn open_note_editor(&mut self, index: usize) {
        if self.notes.is_empty() {
            return;
        }

        let index = index.min(self.notes.len() - 1);
        self.selected_note = index;
        self.editor_note_index = Some(index);
        self.editor_buffer = self.notes[index].content.clone();
        self.editor_cursor = self.editor_buffer.len();
        self.panel_mode = PanelMode::FullEditor;
        self.panel_title = format!("Editing: {}", self.notes[index].title);
        self.panel_lines.clear();
        self.close_ai_overlay();
        self.last_action = format!("Editing note: {}", self.notes[index].title);
    }

    pub(super) fn save_editor(&mut self) {
        let Some(index) = self.editor_note_index else {
            return;
        };

        let updated_at = self.uptime();
        if let Some(note) = self.notes.get_mut(index) {
            note.content = self.editor_buffer.clone();
            note.raw_content = self.editor_buffer.clone();
            note.updated_at = updated_at;
        }
        if let Err(error) = self.persist_note(index) {
            self.last_action = format!("Note save failed: {}", error);
        }
        self.save_shimmer_ticks = 4;
    }

    pub(super) fn persist_note(&mut self, index: usize) -> Result<(), String> {
        match self.note_save_target {
            NoteSaveTarget::Local => Self::save_local_notes(&self.notes),
            NoteSaveTarget::Obsidian => {
                self.ensure_note_obsidian_path(index)?;
                self.write_note_to_obsidian(index)?;
                Self::save_local_notes(&self.notes)
            }
            NoteSaveTarget::Strix => {
                self.push_note_to_strix(index)?;
                Self::save_local_notes(&self.notes)
            }
        }
    }

    pub(super) fn ensure_note_obsidian_path(&mut self, index: usize) -> Result<(), String> {
        if self.obsidian_vault_path.is_none() {
            return Err(String::from(
                "Obsidian save target requires a paired vault. Use /obsidian pair first.",
            ));
        }

        if self
            .notes
            .get(index)
            .and_then(|note| note.obsidian_path.as_ref())
            .is_some()
        {
            return Ok(());
        }

        let title = self
            .notes
            .get(index)
            .map(|note| note.title.clone())
            .unwrap_or_else(|| String::from("Untitled note"));
        let path = self
            .obsidian_note_path_for_title(&title)
            .ok_or_else(|| String::from("Unable to choose an Obsidian note path."))?;
        if let Some(note) = self.notes.get_mut(index) {
            note.obsidian_path = Some(path);
        }
        Ok(())
    }

    pub(super) fn write_note_to_obsidian(&self, index: usize) -> Result<(), String> {
        let Some(note) = self.notes.get(index) else {
            return Ok(());
        };
        let Some(path) = note.obsidian_path.as_ref() else {
            return Ok(());
        };
        fs::write(path, &note.content)
            .map_err(|error| format!("failed to write '{}': {}", path.display(), error))
    }

    pub(super) fn exit_editor(&mut self) {
        self.save_editor();
        let index = self.editor_note_index.unwrap_or(0);
        let note_title = self
            .notes
            .get(index)
            .map(|n| n.title.clone())
            .unwrap_or_default();

        self.selected_note = index;
        self.set_result_panel(
            format!("Saved note: {}", note_title),
            self.note_detail_lines(index),
        );
        self.last_action = format!("Exited note: {}", note_title);
        self.editor_note_index = None;
    }

    pub(super) fn note_detail_lines(&self, index: usize) -> Vec<String> {
        let Some(note) = self.notes.get(index) else {
            return vec![String::from("No note available.")];
        };

        let folder_info = if let Some(fid) = note.folder_id {
            format!("Folder: {}", self.get_folder_path(fid))
        } else {
            String::from("Folder: Uncategorized")
        };

        let mut lines = vec![
            format!("ID: {}", note.id),
            Self::note_source_label(note),
            format!("Updated: {}", note.updated_at),
            folder_info,
            String::new(),
        ];
        lines.extend(note.content.lines().map(|line| line.to_string()));
        lines
    }

    pub(super) fn current_note_index(&self) -> Option<usize> {
        if self.notes.is_empty() {
            None
        } else {
            Some(self.selected_note.min(self.notes.len() - 1))
        }
    }

    pub(super) fn resolve_note_index(&self, target: &str) -> Option<usize> {
        let trimmed = target.trim();
        if trimmed.is_empty() {
            return self.current_note_index();
        }

        let normalized = trimmed.trim_start_matches('#');
        if let Ok(index) = normalized.parse::<usize>() {
            if index == 0 {
                return None;
            }

            if index > self.notes.len() {
                return None;
            }

            return Some(index - 1);
        }

        let lower = trimmed.to_lowercase();
        self.notes.iter().enumerate().find_map(|(index, note)| {
            let title = note.title.to_lowercase();
            let remote_matches = note
                .remote_id
                .as_deref()
                .map(|remote_id| remote_id.eq_ignore_ascii_case(trimmed))
                .unwrap_or(false);
            if remote_matches || note.title.eq_ignore_ascii_case(trimmed) || title.contains(&lower)
            {
                Some(index)
            } else {
                None
            }
        })
    }

    pub(super) fn note_index_by_id(&self, id: usize) -> Option<usize> {
        self.notes
            .iter()
            .enumerate()
            .find_map(|(index, note)| (note.id == id).then_some(index))
    }

    pub(super) fn search_notes(&self, query: &str) -> Vec<String> {
        let query = query.to_lowercase();
        self.notes
            .iter()
            .filter(|note| {
                query.is_empty()
                    || note.title.to_lowercase().contains(&query)
                    || note.content.to_lowercase().contains(&query)
            })
            .map(|note| {
                let id_label = note
                    .remote_id
                    .as_deref()
                    .map(|remote_id| format!("#{} {}", note.id, remote_id))
                    .unwrap_or_else(|| format!("#{}", note.id));
                format!(
                    "{} {} — {}",
                    id_label,
                    note.title,
                    Self::preview_text(&note.content, 56)
                )
            })
            .collect()
    }

    pub(super) fn preview_text(content: &str, limit: usize) -> String {
        let collapsed = content.split_whitespace().collect::<Vec<_>>().join(" ");
        let preview = collapsed.trim();

        if preview.chars().count() <= limit {
            return preview.to_string();
        }

        preview
            .chars()
            .take(limit.saturating_sub(1))
            .collect::<String>()
            + "…"
    }

    pub(super) fn resolve_folder_id(&self, target: &str) -> Option<usize> {
        let trimmed = target.trim();
        if trimmed.is_empty() {
            return self.current_folder_id;
        }

        // Try to parse as ID first (supports #1 or just 1)
        let normalized = trimmed.trim_start_matches('#');
        if let Ok(id) = normalized.parse::<usize>() {
            if self.folders.iter().any(|f| f.id == id) {
                return Some(id);
            }
        }

        // Search by name (case-insensitive)
        let lower = trimmed.to_lowercase();
        self.folders.iter().find_map(|folder| {
            if folder.name.eq_ignore_ascii_case(trimmed)
                || folder.name.to_lowercase().contains(&lower)
            {
                Some(folder.id)
            } else {
                None
            }
        })
    }

    pub(super) fn get_folder_name(&self, folder_id: usize) -> Option<String> {
        self.folders
            .iter()
            .find(|f| f.id == folder_id)
            .map(|f| f.name.clone())
    }

    pub(super) fn get_folder_path(&self, folder_id: usize) -> String {
        let mut path = Vec::new();
        let mut current_id = Some(folder_id);

        while let Some(id) = current_id {
            if let Some(folder) = self.folders.iter().find(|f| f.id == id) {
                path.push(folder.name.clone());
                current_id = folder.parent_id;
            } else {
                break;
            }
        }

        path.reverse();
        if path.is_empty() {
            String::from("/")
        } else {
            format!("/{}", path.join("/"))
        }
    }

    pub(super) fn list_folders(&self) -> Vec<String> {
        if self.folders.is_empty() {
            return vec![String::from(
                "No folders created yet. Use /folder create <name>",
            )];
        }

        self.folders
            .iter()
            .map(|folder| {
                let prefix = if folder.parent_id.is_some() { "  " } else { "" };
                let note_count = self
                    .notes
                    .iter()
                    .filter(|n| n.folder_id == Some(folder.id))
                    .count();
                format!(
                    "{}{}. #{} {:<18} ({} notes) {}",
                    prefix,
                    folder.id,
                    folder.id,
                    folder.name,
                    note_count,
                    if let Some(parent_id) = folder.parent_id {
                        format!("[in #{}]", parent_id)
                    } else {
                        String::new()
                    }
                )
            })
            .collect()
    }

    pub(super) fn build_folder_tree_display(&self) -> Vec<String> {
        if self.folders.is_empty() {
            return vec![String::from("No folders created yet.")];
        }

        let mut lines = Vec::new();
        let root_folders: Vec<&Folder> = self
            .folders
            .iter()
            .filter(|f| f.parent_id.is_none())
            .collect();

        for (i, folder) in root_folders.iter().enumerate() {
            self.render_folder_node(folder, "", i == root_folders.len() - 1, &mut lines);
        }

        let uncategorized_count = self.notes.iter().filter(|n| n.folder_id.is_none()).count();
        if uncategorized_count > 0 {
            lines.push(format!("└── Uncategorized ({} notes)", uncategorized_count));
        }

        lines
    }

    pub(super) fn render_folder_node(
        &self,
        folder: &Folder,
        prefix: &str,
        is_last: bool,
        lines: &mut Vec<String>,
    ) {
        let note_count = self
            .notes
            .iter()
            .filter(|n| n.folder_id == Some(folder.id))
            .count();

        let connector = if is_last { "└── " } else { "├── " };
        lines.push(format!(
            "{}{}#{} {} ({} notes)",
            prefix, connector, folder.id, folder.name, note_count
        ));

        let child_prefix = format!("{}{}", prefix, if is_last { "    " } else { "│   " });
        let children: Vec<&Folder> = self
            .folders
            .iter()
            .filter(|f| f.parent_id == Some(folder.id))
            .collect();

        for (i, child) in children.iter().enumerate() {
            self.render_folder_node(child, &child_prefix, i == children.len() - 1, lines);
        }
    }

    pub(super) fn save_undo_state(&mut self) {
        if self.undo_stack.len() >= 100 {
            self.undo_stack.pop_back();
        }
        self.undo_stack.push_front(EditorState {
            buffer: self.editor_buffer.clone(),
            cursor: self.editor_cursor,
            scroll_offset: self.editor_scroll_offset,
        });
        self.redo_stack.clear();
    }

    pub(super) fn undo(&mut self) {
        if let Some(state) = self.undo_stack.pop_front() {
            if self.redo_stack.len() >= 100 {
                self.redo_stack.pop_back();
            }
            self.redo_stack.push_front(EditorState {
                buffer: self.editor_buffer.clone(),
                cursor: self.editor_cursor,
                scroll_offset: self.editor_scroll_offset,
            });
            self.editor_buffer = state.buffer;
            self.editor_cursor = state.cursor;
            self.editor_scroll_offset = state.scroll_offset;
        }
    }

    pub(super) fn redo(&mut self) {
        if let Some(state) = self.redo_stack.pop_front() {
            if self.undo_stack.len() >= 100 {
                self.undo_stack.pop_back();
            }
            self.undo_stack.push_front(EditorState {
                buffer: self.editor_buffer.clone(),
                cursor: self.editor_cursor,
                scroll_offset: self.editor_scroll_offset,
            });
            self.editor_buffer = state.buffer;
            self.editor_cursor = state.cursor;
            self.editor_scroll_offset = state.scroll_offset;
        }
    }

    pub(super) fn insert_editor_text(&mut self, text: &str) {
        self.save_undo_state();
        for character in text.chars() {
            self.insert_editor_character(character);
        }
    }

    pub(super) fn insert_editor_character(&mut self, character: char) {
        self.clear_editor_selection();
        self.editor_buffer.insert(self.editor_cursor, character);
        self.editor_cursor += character.len_utf8();
    }

    pub(super) fn editor_backspace(&mut self) {
        if self.editor_cursor == 0 {
            return;
        }
        self.save_undo_state();
        let previous = self.editor_buffer[..self.editor_cursor]
            .chars()
            .next_back()
            .map(|character| character.len_utf8())
            .unwrap_or(1);
        self.editor_buffer
            .drain(self.editor_cursor - previous..self.editor_cursor);
        self.editor_cursor -= previous;
    }

    pub(super) fn editor_delete(&mut self) {
        if self.editor_cursor >= self.editor_buffer.len() {
            return;
        }
        self.save_undo_state();
        let next = self.editor_buffer[self.editor_cursor..]
            .chars()
            .next()
            .map(|character| character.len_utf8())
            .unwrap_or(1);
        self.editor_buffer
            .drain(self.editor_cursor..self.editor_cursor + next);
    }

    pub(super) fn toggle_word_wrap(&mut self) {
        self.editor_word_wrap = !self.editor_word_wrap;
    }

    pub(super) fn toggle_cursor_style(&mut self) {
        self.editor_cursor_style = match self.editor_cursor_style {
            CursorStyle::Block => CursorStyle::Line,
            CursorStyle::Line => CursorStyle::Block,
        };
    }

    pub(super) fn select_all_editor(&mut self) {
        let buffer_len = if self.has_live_ai_editor_preview() {
            self.editor_display_buffer().len()
        } else {
            self.editor_buffer.len()
        };
        self.editor_selection.select_all(buffer_len);
    }

    pub(super) fn clear_editor_selection(&mut self) {
        self.editor_selection.clear();
    }

    pub(super) fn scroll_up(&mut self, lines: usize) {
        self.editor_scroll_offset = self.editor_scroll_offset.saturating_sub(lines);
    }

    pub(super) fn scroll_down(&mut self, lines: usize) {
        self.editor_scroll_offset = self.editor_scroll_offset.saturating_add(lines);
    }

    pub(super) fn start_search(&mut self) {
        self.search_state.active = true;
        self.search_state.query.clear();
        self.search_state.matches.clear();
        self.search_state.current_match = None;
    }

    pub(super) fn cancel_search(&mut self) {
        self.search_state.active = false;
        self.search_state.query.clear();
        self.search_state.matches.clear();
        self.search_state.current_match = None;
    }

    pub(super) fn start_title_edit(&mut self) {
        if let Some(index) = self.editor_note_index {
            self.editing_title = true;
            self.title_buffer = self.notes[index].title.clone();
            self.title_cursor = self.title_buffer.len();
            self.last_action = String::from("Editing title. Press Enter to save, Esc to cancel.");
        }
    }

    pub(super) fn finish_title_edit(&mut self, save: bool) {
        if save && !self.title_buffer.trim().is_empty() {
            if let Some(index) = self.editor_note_index {
                self.notes[index].title = self.title_buffer.trim().to_string();
                self.panel_title = format!("Editing: {}", self.notes[index].title);
                self.last_action = format!("Title updated to: {}", self.notes[index].title);
            }
        } else if !save {
            self.last_action = String::from("Title edit cancelled.");
        }
        self.editing_title = false;
        self.title_buffer.clear();
        self.title_cursor = 0;
    }

    pub(super) fn handle_title_edit_key(&mut self, key_event: KeyEvent) {
        if key_event.kind != KeyEventKind::Press && key_event.kind != KeyEventKind::Repeat {
            return;
        }
        self.title_cursor = Self::clamp_to_char_boundary(&self.title_buffer, self.title_cursor);
        match key_event.code {
            KeyCode::Enter => {
                self.finish_title_edit(true);
            }
            KeyCode::Esc => {
                self.finish_title_edit(false);
            }
            KeyCode::Backspace => {
                if self.title_cursor > 0 {
                    let previous =
                        Self::previous_char_boundary(&self.title_buffer, self.title_cursor);
                    self.title_buffer.drain(previous..self.title_cursor);
                    self.title_cursor = previous;
                }
            }
            KeyCode::Delete => {
                if self.title_cursor < self.title_buffer.len() {
                    let next = Self::next_char_boundary(&self.title_buffer, self.title_cursor);
                    self.title_buffer.drain(self.title_cursor..next);
                }
            }
            KeyCode::Left => {
                if self.title_cursor > 0 {
                    self.title_cursor =
                        Self::previous_char_boundary(&self.title_buffer, self.title_cursor);
                }
            }
            KeyCode::Right => {
                if self.title_cursor < self.title_buffer.len() {
                    self.title_cursor =
                        Self::next_char_boundary(&self.title_buffer, self.title_cursor);
                }
            }
            KeyCode::Home => {
                self.title_cursor = 0;
            }
            KeyCode::End => {
                self.title_cursor = self.title_buffer.len();
            }
            KeyCode::Char(c) if !key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                self.title_buffer.insert(self.title_cursor, c);
                self.title_cursor += c.len_utf8();
            }
            _ => {}
        }
    }

    pub(super) fn previous_char_boundary(input: &str, cursor: usize) -> usize {
        let cursor = Self::clamp_to_char_boundary(input, cursor);
        input[..cursor]
            .char_indices()
            .next_back()
            .map(|(index, _)| index)
            .unwrap_or(0)
    }

    pub(super) fn next_char_boundary(input: &str, cursor: usize) -> usize {
        let cursor = Self::clamp_to_char_boundary(input, cursor);
        input[cursor..]
            .chars()
            .next()
            .map(|character| cursor + character.len_utf8())
            .unwrap_or_else(|| input.len())
    }

    pub(super) fn clamp_to_char_boundary(input: &str, cursor: usize) -> usize {
        let mut cursor = cursor.min(input.len());
        while cursor > 0 && !input.is_char_boundary(cursor) {
            cursor -= 1;
        }
        cursor
    }

    pub(super) fn search_next(&mut self) {
        if self.search_state.matches.is_empty() {
            return;
        }
        let current = self.search_state.current_match.unwrap_or(0);
        let next = if current + 1 >= self.search_state.matches.len() {
            0
        } else {
            current + 1
        };
        self.search_state.current_match = Some(next);
        if let Some(&pos) = self.search_state.matches.get(next) {
            self.editor_cursor = pos;
        }
    }

    pub(super) fn search_prev(&mut self) {
        if self.search_state.matches.is_empty() {
            return;
        }
        let current = self.search_state.current_match.unwrap_or(0);
        let prev = if current == 0 {
            self.search_state.matches.len() - 1
        } else {
            current - 1
        };
        self.search_state.current_match = Some(prev);
        if let Some(&pos) = self.search_state.matches.get(prev) {
            self.editor_cursor = pos;
        }
    }

    pub(super) fn update_search(&mut self) {
        self.search_state.matches.clear();
        if self.search_state.query.is_empty() {
            self.search_state.current_match = None;
            return;
        }
        let query = self.search_state.query.to_lowercase();
        let buffer_lower = self.editor_buffer.to_lowercase();
        let mut start = 0;
        while let Some(pos) = buffer_lower[start..].find(&query) {
            let absolute_pos = start + pos;
            self.search_state.matches.push(absolute_pos);
            start = absolute_pos + 1;
        }
        if !self.search_state.matches.is_empty() {
            self.search_state.current_match = Some(0);
            self.editor_cursor = self.search_state.matches[0];
        } else {
            self.search_state.current_match = None;
        }
    }

    pub(super) fn handle_search_key(&mut self, key_event: KeyEvent) {
        if key_event.kind != KeyEventKind::Press {
            return;
        }
        match key_event.code {
            KeyCode::Esc | KeyCode::Enter => {
                self.cancel_search();
            }
            KeyCode::Backspace => {
                self.search_state.query.pop();
                self.update_search();
            }
            KeyCode::Char(c) => {
                self.search_state.query.push(c);
                self.update_search();
            }
            _ => {}
        }
    }

    pub(super) fn editor_move_left(&mut self) {
        if self.editor_cursor == 0 {
            return;
        }

        let previous = self.editor_buffer[..self.editor_cursor]
            .chars()
            .next_back()
            .map(|character| character.len_utf8())
            .unwrap_or(1);
        self.editor_cursor -= previous;
    }

    pub(super) fn editor_move_right(&mut self) {
        if self.editor_cursor >= self.editor_buffer.len() {
            return;
        }

        let next = self.editor_buffer[self.editor_cursor..]
            .chars()
            .next()
            .map(|character| character.len_utf8())
            .unwrap_or(1);
        self.editor_cursor += next;
    }

    pub(super) fn editor_move_up(&mut self) {
        // Find the start of the current line
        let current_pos = self.editor_cursor;
        let line_start = self.editor_buffer[..current_pos]
            .rfind('\n')
            .map(|pos| pos + 1)
            .unwrap_or(0);

        // If we're on the first line, can't move up
        if line_start == 0 {
            return;
        }

        // Calculate column position within current line
        let column = current_pos - line_start;

        // Find the start of the previous line
        let prev_line_end = line_start - 1;
        let prev_line_start = self.editor_buffer[..prev_line_end]
            .rfind('\n')
            .map(|pos| pos + 1)
            .unwrap_or(0);

        // Move cursor to the same column in the previous line (or end of line if shorter)
        let prev_line_len = prev_line_end - prev_line_start;
        let new_column = column.min(prev_line_len);
        self.editor_cursor = prev_line_start + new_column;
    }

    pub(super) fn editor_move_down(&mut self) {
        // Find the end of the current line
        let current_pos = self.editor_cursor;
        let line_end = self.editor_buffer[current_pos..]
            .find('\n')
            .map(|pos| current_pos + pos)
            .unwrap_or(self.editor_buffer.len());

        // If we're on the last line, can't move down
        if line_end >= self.editor_buffer.len() {
            return;
        }

        // Calculate column position within current line
        let line_start = self.editor_buffer[..current_pos]
            .rfind('\n')
            .map(|pos| pos + 1)
            .unwrap_or(0);
        let column = current_pos - line_start;

        // Find the end of the next line
        let next_line_start = line_end + 1;
        let next_line_end = self.editor_buffer[next_line_start..]
            .find('\n')
            .map(|pos| next_line_start + pos)
            .unwrap_or(self.editor_buffer.len());

        // Move cursor to the same column in the next line (or end of line if shorter)
        let next_line_len = next_line_end - next_line_start;
        let new_column = column.min(next_line_len);
        self.editor_cursor = next_line_start + new_column;
    }

    pub(super) fn sync_selection(&mut self) {
        let suggestions = self.visible_commands(16);
        if suggestions.is_empty() {
            self.selected_suggestion = 0;
            return;
        }

        self.selected_suggestion = self.selected_suggestion.min(suggestions.len() - 1);
    }
}
