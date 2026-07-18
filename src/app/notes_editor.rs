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
        self.editor_save_status = EditorSaveStatus::Clean;
        self.editor_drag_anchor = None;
        self.clear_editor_selection();
        self.last_action = format!("Editing note: {}", self.notes[index].title);
    }

    pub(super) fn save_editor(&mut self) {
        // Ordinary human saves already have editor undo history and should stay
        // cheap. Explicit paths, command mutations, and AI applies retain their
        // existing Temporal Fork checkpoints.
        self.save_editor_contents();
    }

    pub(super) fn save_editor_contents(&mut self) {
        let Some(index) = self.editor_note_index else {
            return;
        };

        let content_unchanged = self
            .notes
            .get(index)
            .map(|note| note.content == self.editor_buffer)
            .unwrap_or(false);
        if content_unchanged
            && matches!(
                self.editor_save_status,
                EditorSaveStatus::Clean | EditorSaveStatus::Saved
            )
        {
            return;
        }

        let updated_at = self.uptime();
        if let Some(note) = self.notes.get_mut(index) {
            note.content = self.editor_buffer.clone();
            // raw_content stores the original rich remote representation. Once
            // plain text is edited it is stale; clearing it also avoids keeping
            // and serializing a second full copy of every edited note.
            note.raw_content.clear();
            note.updated_at = updated_at;
        }
        match self.persist_note(index) {
            Err(error) => {
                self.last_action = format!("Note save failed: {}", error);
                self.editor_save_status = EditorSaveStatus::Failed(error);
            }
            Ok(()) => {
                self.editor_save_status = EditorSaveStatus::Saved;
                self.last_action = String::from("Note saved successfully.");
                if let Some(note) = self.notes.get(index) {
                    let _ = self.append_trail_event(
                        "note",
                        format!("Saved note: {}.", note.title),
                        vec![note.id.to_string()],
                        TrailImportance::High,
                    );
                }
            }
        }
        // Keep the initiated state visible before revealing the completed result.
        self.save_shimmer_ticks = 18;
    }

    pub(super) fn persist_note(&mut self, index: usize) -> Result<(), String> {
        // A note always writes back to every home it already has, regardless
        // of the global save target. Otherwise an Obsidian-backed note saved
        // while the target is Strix forks into a duplicate remote note while
        // the vault file goes stale (and vice versa).
        let has_obsidian_home = self
            .notes
            .get(index)
            .and_then(|note| note.obsidian_path.as_ref())
            .is_some();
        let has_strix_home = self
            .notes
            .get(index)
            .and_then(|note| note.remote_id.as_ref())
            .is_some();

        let mut first_error: Option<String> = None;
        let record_error = |error: String, first_error: &mut Option<String>| {
            if first_error.is_none() {
                *first_error = Some(error);
            }
        };

        if has_obsidian_home {
            if let Err(error) = self.write_note_to_obsidian(index) {
                record_error(error, &mut first_error);
            }
        }
        if has_strix_home && self.is_strix_connected() {
            if let Err(error) = self.queue_strix_note_sync(index) {
                record_error(error, &mut first_error);
            }
        }

        // Local-only notes adopt the configured save target as their home.
        if !has_obsidian_home && !has_strix_home {
            match self.note_save_target {
                NoteSaveTarget::Local => {}
                NoteSaveTarget::Obsidian => {
                    let result = self
                        .ensure_note_obsidian_path(index)
                        .and_then(|_| self.write_note_to_obsidian(index));
                    if let Err(error) = result {
                        record_error(error, &mut first_error);
                    }
                }
                NoteSaveTarget::Strix => {
                    if let Err(error) = self.queue_strix_note_sync(index) {
                        record_error(error, &mut first_error);
                    }
                }
            }
        }

        // The local cache is the source of truth for the TUI; always write it
        // even when a remote home failed.
        if let Err(error) = Self::save_local_notes(&self.notes) {
            record_error(error, &mut first_error);
        }

        match first_error {
            Some(error) => Err(error),
            None => Ok(()),
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
        if matches!(self.editor_save_status, EditorSaveStatus::Failed(_)) {
            return;
        }
        let index = self.editor_note_index.unwrap_or(0);
        let note_title = self
            .notes
            .get(index)
            .map(|n| n.title.clone())
            .unwrap_or_default();

        self.selected_note = index;
        self.set_result_panel(
            format!("Saved note: {}", note_title),
            self.note_exit_lines(index),
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

    pub(super) fn note_exit_lines(&self, index: usize) -> Vec<String> {
        const EXIT_PREVIEW_LINES: usize = 18;
        let Some(note) = self.notes.get(index) else {
            return vec![String::from("No note available.")];
        };
        let folder_info = note
            .folder_id
            .map(|folder_id| format!("Folder: {}", self.get_folder_path(folder_id)))
            .unwrap_or_else(|| String::from("Folder: Uncategorized"));
        let mut lines = vec![
            format!("ID: {}", note.id),
            Self::note_source_label(note),
            format!("Updated: {}", note.updated_at),
            folder_info,
            String::new(),
        ];
        let mut content_lines = note.content.lines();
        lines.extend(
            content_lines
                .by_ref()
                .take(EXIT_PREVIEW_LINES)
                .map(str::to_string),
        );
        if content_lines.next().is_some() {
            lines.push(String::new());
            lines.push(String::from(
                "… more lines · use /note read to reopen the full note",
            ));
        }
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

        if let Some(id) = trimmed.strip_prefix('#') {
            return id
                .parse::<usize>()
                .ok()
                .and_then(|id| self.note_index_by_id(id));
        }

        if let Ok(index) = trimmed.parse::<usize>() {
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
            if !self.room_matches_note(note) {
                return None;
            }

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
        self.notes.iter().enumerate().find_map(|(index, note)| {
            (note.id == id && self.room_matches_note(note)).then_some(index)
        })
    }

    pub(super) fn search_notes(&self, query: &str) -> Vec<String> {
        let query = query.to_lowercase();
        self.notes
            .iter()
            .filter(|note| self.room_matches_note(note))
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
        if limit == 0 {
            return String::new();
        }

        let mut preview = String::new();
        let mut character_count = 0;
        let mut truncated = false;

        'words: for word in content.split_whitespace() {
            if !preview.is_empty() {
                if character_count == limit {
                    truncated = true;
                    break;
                }
                preview.push(' ');
                character_count += 1;
            }
            for character in word.chars() {
                if character_count == limit {
                    truncated = true;
                    break 'words;
                }
                preview.push(character);
                character_count += 1;
            }
        }

        if truncated {
            preview.pop();
            preview.push('…');
        }
        preview
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
        self.editor_save_status = EditorSaveStatus::Unsaved;
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
            self.editor_save_status = EditorSaveStatus::Unsaved;
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
            self.editor_save_status = EditorSaveStatus::Unsaved;
        }
    }

    pub(super) fn insert_editor_text(&mut self, text: &str) {
        self.save_undo_state();
        self.delete_editor_selection_without_undo();
        self.editor_buffer.insert_str(self.editor_cursor, text);
        self.editor_cursor += text.len();
        self.editor_save_status = EditorSaveStatus::Unsaved;
    }

    pub(super) fn insert_editor_character(&mut self, character: char) {
        if self.editor_selection.active {
            self.save_undo_state();
            self.delete_editor_selection_without_undo();
        }
        self.editor_save_status = EditorSaveStatus::Unsaved;
        self.editor_buffer.insert(self.editor_cursor, character);
        self.editor_cursor += character.len_utf8();
    }

    pub(super) fn editor_backspace(&mut self) {
        if self.delete_editor_selection() {
            return;
        }
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
        if self.delete_editor_selection() {
            return;
        }
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
        self.editor_cursor = buffer_len;
    }

    pub(super) fn clear_editor_selection(&mut self) {
        self.editor_selection.clear();
    }

    pub(super) fn selected_editor_text(&self) -> Option<&str> {
        if !self.editor_selection.active || self.has_live_ai_editor_preview() {
            return None;
        }
        let start = Self::clamp_to_char_boundary(&self.editor_buffer, self.editor_selection.start);
        let end = Self::clamp_to_char_boundary(&self.editor_buffer, self.editor_selection.end);
        (start < end).then(|| &self.editor_buffer[start..end])
    }

    pub(super) fn copy_editor_selection(&mut self) -> bool {
        let Some(text) = self.selected_editor_text().map(str::to_owned) else {
            return false;
        };
        match arboard::Clipboard::new().and_then(|mut clipboard| clipboard.set_text(text)) {
            Ok(()) => {
                self.last_action = String::from("Selection copied to clipboard.");
                true
            }
            Err(error) => {
                self.last_action = format!("Could not copy selection: {}", error);
                false
            }
        }
    }

    pub(super) fn paste_editor_clipboard(&mut self) {
        match arboard::Clipboard::new().and_then(|mut clipboard| clipboard.get_text()) {
            Ok(text) => self.insert_editor_text(&text),
            Err(error) => self.last_action = format!("Could not paste from clipboard: {}", error),
        }
    }

    pub(super) fn delete_editor_selection(&mut self) -> bool {
        if !self.editor_selection.active {
            return false;
        }
        self.save_undo_state();
        self.delete_editor_selection_without_undo()
    }

    fn delete_editor_selection_without_undo(&mut self) -> bool {
        if !self.editor_selection.active {
            return false;
        }
        let start = Self::clamp_to_char_boundary(&self.editor_buffer, self.editor_selection.start);
        let end = Self::clamp_to_char_boundary(&self.editor_buffer, self.editor_selection.end);
        self.clear_editor_selection();
        if start >= end {
            return false;
        }
        self.editor_buffer.drain(start..end);
        self.editor_cursor = start;
        self.editor_save_status = EditorSaveStatus::Unsaved;
        true
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
                self.editor_save_status = EditorSaveStatus::Unsaved;
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
        let mut buffer_lower = String::with_capacity(self.editor_buffer.len());
        let mut boundaries = Vec::new();
        for (original_index, character) in self.editor_buffer.char_indices() {
            boundaries.push((buffer_lower.len(), original_index));
            buffer_lower.extend(character.to_lowercase());
        }
        self.search_state.matches.extend(
            boundaries
                .into_iter()
                .filter(|(lower_index, _)| buffer_lower[*lower_index..].starts_with(&query))
                .map(|(_, original_index)| original_index),
        );
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
        let current_pos = Self::clamp_to_char_boundary(&self.editor_buffer, self.editor_cursor);
        let line_start = self.editor_buffer[..current_pos]
            .rfind('\n')
            .map(|pos| pos + 1)
            .unwrap_or(0);

        if line_start == 0 {
            return;
        }

        let column = self.editor_buffer[line_start..current_pos].chars().count();
        let prev_line_end = line_start - 1;
        let prev_line_start = self.editor_buffer[..prev_line_end]
            .rfind('\n')
            .map(|pos| pos + 1)
            .unwrap_or(0);
        let prev_line = &self.editor_buffer[prev_line_start..prev_line_end];
        let new_column = column.min(prev_line.chars().count());
        let new_offset = prev_line
            .char_indices()
            .nth(new_column)
            .map(|(offset, _)| offset)
            .unwrap_or(prev_line.len());

        self.editor_cursor = prev_line_start + new_offset;
    }

    pub(super) fn editor_move_down(&mut self) {
        let current_pos = Self::clamp_to_char_boundary(&self.editor_buffer, self.editor_cursor);
        let line_end = self.editor_buffer[current_pos..]
            .find('\n')
            .map(|pos| current_pos + pos)
            .unwrap_or(self.editor_buffer.len());

        if line_end >= self.editor_buffer.len() {
            return;
        }

        let line_start = self.editor_buffer[..current_pos]
            .rfind('\n')
            .map(|pos| pos + 1)
            .unwrap_or(0);
        let column = self.editor_buffer[line_start..current_pos].chars().count();

        let next_line_start = line_end + 1;
        let next_line_end = self.editor_buffer[next_line_start..]
            .find('\n')
            .map(|pos| next_line_start + pos)
            .unwrap_or(self.editor_buffer.len());
        let next_line = &self.editor_buffer[next_line_start..next_line_end];
        let new_column = column.min(next_line.chars().count());
        let new_offset = next_line
            .char_indices()
            .nth(new_column)
            .map(|(offset, _)| offset)
            .unwrap_or(next_line.len());

        self.editor_cursor = next_line_start + new_offset;
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
