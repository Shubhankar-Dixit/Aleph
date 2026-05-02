use super::*;

#[allow(dead_code)]
impl App {
    pub fn handle_key(&mut self, key_event: KeyEvent) {
        if self.is_full_editor() {
            self.handle_full_editor_key(key_event);
            return;
        }
        if self.is_editing_note() {
            self.handle_editor_key(key_event);
            return;
        }
        if self.is_ai_chat() {
            self.handle_chat_key(key_event);
            return;
        }
        if self.is_login_picker() {
            self.handle_login_picker_key(key_event);
            return;
        }
        if self.is_note_list() {
            self.handle_note_list_key(key_event);
            return;
        }
        if self.is_path_list() {
            self.handle_path_list_key(key_event);
            return;
        }
        if self.is_vault_picker() {
            self.handle_vault_picker_key(key_event);
            return;
        }
        if self.is_obsidian_sync_confirm() {
            self.handle_obsidian_sync_confirm_key(key_event);
            return;
        }
        if self.is_settings() {
            self.handle_settings_key(key_event);
            return;
        }

        match key_event.code {
            KeyCode::Char('c')
                if key_event.kind == KeyEventKind::Press
                    && key_event.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                self.request_quit();
            }
            KeyCode::Esc if key_event.kind == KeyEventKind::Press => {
                self.request_quit();
            }
            KeyCode::Enter if key_event.kind == KeyEventKind::Press => self.submit_prompt(),
            KeyCode::Backspace if key_event.kind == KeyEventKind::Press => self.backspace(),
            KeyCode::Delete if key_event.kind == KeyEventKind::Press => self.delete(),
            KeyCode::Left
                if matches!(key_event.kind, KeyEventKind::Press | KeyEventKind::Repeat) =>
            {
                self.move_left()
            }
            KeyCode::Right
                if matches!(key_event.kind, KeyEventKind::Press | KeyEventKind::Repeat) =>
            {
                self.move_right()
            }
            KeyCode::Home if key_event.kind == KeyEventKind::Press => self.cursor = 0,
            KeyCode::End if key_event.kind == KeyEventKind::Press => {
                self.cursor = self.prompt.len()
            }
            KeyCode::Up if matches!(key_event.kind, KeyEventKind::Press | KeyEventKind::Repeat) => {
                self.cycle_suggestion(-1)
            }
            KeyCode::Down
                if matches!(key_event.kind, KeyEventKind::Press | KeyEventKind::Repeat) =>
            {
                self.cycle_suggestion(1)
            }
            KeyCode::Tab if key_event.kind == KeyEventKind::Press => self.autocomplete(),
            KeyCode::Char(character)
                if key_event.kind == KeyEventKind::Press
                    && !key_event.modifiers.contains(KeyModifiers::CONTROL)
                    && !key_event.modifiers.contains(KeyModifiers::ALT) =>
            {
                self.insert_character(character);
            }
            _ => {}
        }
    }

    pub fn handle_mouse(&mut self, mouse_event: MouseEvent) {
        if self.is_settings() {
            self.handle_settings_mouse(mouse_event);
            return;
        }

        if self.is_ai_chat() {
            match mouse_event.kind {
                MouseEventKind::ScrollUp => self.scroll_chat_up(1),
                MouseEventKind::ScrollDown => self.scroll_chat_down(1),
                _ => {}
            }
            return;
        }

        if self.is_full_editor()
            && self.ai_overlay_visible
            && matches!(mouse_event.kind, MouseEventKind::Down(_))
        {
            self.close_ai_overlay();
        }
    }

    pub(super) fn handle_settings_mouse(&mut self, mouse_event: MouseEvent) {
        if !matches!(mouse_event.kind, MouseEventKind::Down(MouseButton::Left)) {
            return;
        }

        const SETTINGS_LIST_START_ROW: u16 = 20;
        const SETTINGS_ITEM_COUNT: usize = 8;

        let Some(row) = mouse_event.row.checked_sub(SETTINGS_LIST_START_ROW) else {
            return;
        };
        let index = row as usize;
        if index >= SETTINGS_ITEM_COUNT {
            return;
        }

        self.settings_selected = index;
        self.handle_settings_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    }

    pub(super) fn handle_editor_key(&mut self, key_event: KeyEvent) {
        if self.search_state.active {
            self.handle_search_key(key_event);
            return;
        }
        match key_event.code {
            KeyCode::Char('c')
                if key_event.kind == KeyEventKind::Press
                    && key_event.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                self.exit_editor();
            }
            KeyCode::Char('s')
                if key_event.kind == KeyEventKind::Press
                    && key_event.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                self.save_editor();
            }
            KeyCode::Char('a')
                if key_event.kind == KeyEventKind::Press
                    && key_event.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                self.select_all_editor();
            }
            KeyCode::Esc if key_event.kind == KeyEventKind::Press => self.exit_editor(),
            KeyCode::Enter if key_event.kind == KeyEventKind::Press => {
                self.save_undo_state();
                self.clear_editor_selection();
                self.insert_editor_character('\n');
            }
            KeyCode::Tab if key_event.kind == KeyEventKind::Press => {
                self.clear_editor_selection();
                self.insert_editor_text("    ");
            }
            KeyCode::Backspace if key_event.kind == KeyEventKind::Press => {
                self.clear_editor_selection();
                self.editor_backspace()
            }
            KeyCode::Delete if key_event.kind == KeyEventKind::Press => {
                self.clear_editor_selection();
                self.editor_delete()
            }
            KeyCode::Left
                if matches!(key_event.kind, KeyEventKind::Press | KeyEventKind::Repeat) =>
            {
                self.clear_editor_selection();
                self.editor_move_left()
            }
            KeyCode::Right
                if matches!(key_event.kind, KeyEventKind::Press | KeyEventKind::Repeat) =>
            {
                self.clear_editor_selection();
                self.editor_move_right()
            }
            KeyCode::Up
                if matches!(key_event.kind, KeyEventKind::Press | KeyEventKind::Repeat)
                    && !key_event.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                self.clear_editor_selection();
                self.editor_move_up()
            }
            KeyCode::Down
                if matches!(key_event.kind, KeyEventKind::Press | KeyEventKind::Repeat)
                    && !key_event.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                self.clear_editor_selection();
                self.editor_move_down()
            }
            KeyCode::Up
                if key_event.kind == KeyEventKind::Press
                    && key_event.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                self.scroll_up(1)
            }
            KeyCode::Down
                if key_event.kind == KeyEventKind::Press
                    && key_event.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                self.scroll_down(1)
            }
            KeyCode::PageUp if key_event.kind == KeyEventKind::Press => self.scroll_up(10),
            KeyCode::PageDown if key_event.kind == KeyEventKind::Press => self.scroll_down(10),
            KeyCode::Home if key_event.kind == KeyEventKind::Press => {
                self.clear_editor_selection();
                self.editor_cursor = 0
            }
            KeyCode::End if key_event.kind == KeyEventKind::Press => {
                self.clear_editor_selection();
                self.editor_cursor = self.editor_buffer.len()
            }
            KeyCode::Char('z')
                if key_event.kind == KeyEventKind::Press
                    && key_event.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                self.clear_editor_selection();
                self.undo();
            }
            KeyCode::Char('y')
                if key_event.kind == KeyEventKind::Press
                    && key_event.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                self.clear_editor_selection();
                self.redo();
            }
            KeyCode::Char('f')
                if key_event.kind == KeyEventKind::Press
                    && key_event.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                self.start_search();
            }
            KeyCode::F(3) if key_event.kind == KeyEventKind::Press => {
                if key_event.modifiers.contains(KeyModifiers::SHIFT) {
                    self.search_prev();
                } else {
                    self.search_next();
                }
            }
            KeyCode::Char('w')
                if key_event.kind == KeyEventKind::Press
                    && key_event.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                self.toggle_word_wrap();
            }
            KeyCode::Char('b')
                if key_event.kind == KeyEventKind::Press
                    && key_event.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                self.toggle_cursor_style();
            }
            KeyCode::Char(character)
                if key_event.kind == KeyEventKind::Press
                    && !key_event.modifiers.contains(KeyModifiers::CONTROL)
                    && !key_event.modifiers.contains(KeyModifiers::ALT) =>
            {
                self.insert_editor_character(character);
            }
            _ => {}
        }
    }

    pub(super) fn handle_chat_key(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Esc if key_event.kind == KeyEventKind::Press => {
                if self.pending_agent_decision.is_some() {
                    self.cancel_pending_agent_action();
                    return;
                }
                // Exit chat mode and return to commands
                self.panel_mode = PanelMode::Commands;
                self.panel_title = String::from("Commands");
                self.panel_lines.clear();
                self.last_action = String::from("Exited AI chat.");
            }
            KeyCode::Char('c')
                if key_event.kind == KeyEventKind::Press
                    && key_event.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                self.request_quit();
            }
            KeyCode::Char('g')
                if key_event.kind == KeyEventKind::Press
                    && key_event.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                self.toggle_agent_mode();
            }
            KeyCode::Enter if key_event.kind == KeyEventKind::Press => {
                // Send chat message
                let msg = self.chat_input_buffer.trim().to_string();
                if self.pending_agent_decision.is_some() {
                    if msg.is_empty() || Self::is_affirmative_agent_permission(&msg) {
                        if self.confirm_pending_agent_action() {
                            self.chat_input_buffer.clear();
                            self.chat_input_cursor = 0;
                        }
                        return;
                    }

                    if Self::is_negative_agent_permission(&msg) {
                        self.cancel_pending_agent_action();
                        self.chat_input_buffer.clear();
                        self.chat_input_cursor = 0;
                        return;
                    }

                    self.cancel_pending_agent_action();
                }

                if !msg.is_empty() {
                    if (self.agent_mode_enabled && self.try_start_agent_action(&msg))
                        || self.start_chat_turn(msg)
                    {
                        self.chat_input_buffer.clear();
                        self.chat_input_cursor = 0;
                    }
                }
            }
            KeyCode::PageUp if key_event.kind == KeyEventKind::Press => self.scroll_chat_up(10),
            KeyCode::PageDown if key_event.kind == KeyEventKind::Press => self.scroll_chat_down(10),
            KeyCode::Backspace if key_event.kind == KeyEventKind::Press => {
                if self.chat_input_cursor > 0 {
                    let prev = self.chat_input_buffer[..self.chat_input_cursor]
                        .chars()
                        .next_back()
                        .map(|c| c.len_utf8())
                        .unwrap_or(1);
                    self.chat_input_buffer
                        .drain(self.chat_input_cursor - prev..self.chat_input_cursor);
                    self.chat_input_cursor -= prev;
                }
            }
            KeyCode::Delete if key_event.kind == KeyEventKind::Press => {
                if self.chat_input_cursor < self.chat_input_buffer.len() {
                    let next = self.chat_input_buffer[self.chat_input_cursor..]
                        .chars()
                        .next()
                        .map(|c| c.len_utf8())
                        .unwrap_or(1);
                    self.chat_input_buffer
                        .drain(self.chat_input_cursor..self.chat_input_cursor + next);
                }
            }
            KeyCode::Left
                if matches!(key_event.kind, KeyEventKind::Press | KeyEventKind::Repeat) =>
            {
                if self.chat_input_cursor > 0 {
                    let prev = self.chat_input_buffer[..self.chat_input_cursor]
                        .chars()
                        .next_back()
                        .map(|c| c.len_utf8())
                        .unwrap_or(1);
                    self.chat_input_cursor -= prev;
                }
            }
            KeyCode::Right
                if matches!(key_event.kind, KeyEventKind::Press | KeyEventKind::Repeat) =>
            {
                if self.chat_input_cursor < self.chat_input_buffer.len() {
                    let next = self.chat_input_buffer[self.chat_input_cursor..]
                        .chars()
                        .next()
                        .map(|c| c.len_utf8())
                        .unwrap_or(1);
                    self.chat_input_cursor += next;
                }
            }
            KeyCode::Home if key_event.kind == KeyEventKind::Press => self.chat_input_cursor = 0,
            KeyCode::End if key_event.kind == KeyEventKind::Press => {
                self.chat_input_cursor = self.chat_input_buffer.len()
            }
            KeyCode::Char(character)
                if key_event.kind == KeyEventKind::Press
                    && !key_event.modifiers.contains(KeyModifiers::CONTROL)
                    && !key_event.modifiers.contains(KeyModifiers::ALT) =>
            {
                self.chat_input_buffer
                    .insert(self.chat_input_cursor, character);
                self.chat_input_cursor += character.len_utf8();
            }
            _ => {}
        }
    }

    pub(super) fn normalized_prompt(&self) -> String {
        Self::normalize_command_input(self.prompt.trim().trim_start_matches('/'))
    }

    pub(super) fn insert_character(&mut self, character: char) {
        self.prompt.insert(self.cursor, character);
        self.cursor += character.len_utf8();
        self.history_index = None;
        self.suggestion_filter = None;
        self.sync_selection();
    }

    pub(super) fn backspace(&mut self) {
        if self.cursor == 0 {
            return;
        }

        let previous = self.prompt[..self.cursor]
            .chars()
            .next_back()
            .map(|character| character.len_utf8())
            .unwrap_or(1);
        self.prompt.drain(self.cursor - previous..self.cursor);
        self.cursor -= previous;
        self.history_index = None;
        self.suggestion_filter = None;
        self.sync_selection();
    }

    pub(super) fn delete(&mut self) {
        if self.cursor >= self.prompt.len() {
            return;
        }

        let next = self.prompt[self.cursor..]
            .chars()
            .next()
            .map(|character| character.len_utf8())
            .unwrap_or(1);
        self.prompt.drain(self.cursor..self.cursor + next);
        self.history_index = None;
        self.suggestion_filter = None;
        self.sync_selection();
    }

    pub(super) fn move_left(&mut self) {
        if self.cursor == 0 {
            return;
        }

        let previous = self.prompt[..self.cursor]
            .chars()
            .next_back()
            .map(|character| character.len_utf8())
            .unwrap_or(1);
        self.cursor -= previous;
    }

    pub(super) fn move_right(&mut self) {
        if self.cursor >= self.prompt.len() {
            return;
        }

        let next = self.prompt[self.cursor..]
            .chars()
            .next()
            .map(|character| character.len_utf8())
            .unwrap_or(1);
        self.cursor += next;
    }

    pub(super) fn cycle_suggestion(&mut self, direction: isize) {
        // Save the current query the first time we cycle
        if self.suggestion_filter.is_none() {
            let query = self.normalized_prompt().to_lowercase();
            self.suggestion_filter = Some(query);
        }

        // Get filtered list based on suggestion_filter
        let query = self.suggestion_filter.as_ref().unwrap().clone();

        let suggestions: Vec<_> = if query.is_empty() {
            COMMANDS
                .iter()
                .filter(|cmd| self.is_command_visible(cmd))
                .collect()
        } else {
            COMMANDS
                .iter()
                .filter(|cmd| {
                    self.is_command_visible(cmd)
                        && (cmd.name.contains(&query)
                            || cmd.description.to_lowercase().contains(&query))
                })
                .collect()
        };

        if suggestions.is_empty() {
            return;
        }

        let len = suggestions.len() as isize;
        let current = self.selected_suggestion as isize;
        let next_index = (current + direction).rem_euclid(len) as usize;
        self.selected_suggestion = next_index;

        let selected = suggestions[next_index];
        self.prompt = format!("/{}", selected.name);
        self.cursor = self.prompt.len();
        self.last_action = format!("Selected: {}", selected.name);
    }

    pub(super) fn autocomplete(&mut self) {
        let suggestions = self.visible_commands(16);
        if suggestions.is_empty() {
            return;
        }

        let selected = suggestions[self.selected_suggestion.min(suggestions.len() - 1)];
        self.prompt = Self::command_label(selected);
        self.cursor = self.prompt.len();
        self.history_index = None;
        self.last_action = format!("Autocomplete: {}", Self::command_label(selected));
        self.sync_selection();

        if !suggestions.is_empty() {
            self.selected_suggestion = (self.selected_suggestion + 1) % suggestions.len();
        }
    }

    pub(super) fn submit_prompt(&mut self) {
        let raw = self.prompt.trim().to_string();
        if raw.is_empty() {
            self.last_action = String::from("Type a query, command, or press Ctrl+C to quit.");
            return;
        }

        if !raw.starts_with('/') {
            let query = raw.clone();
            if self.agent_mode_enabled && self.try_start_agent_action(&query) {
                self.history.push(raw);
                self.history_index = None;
                self.reset_prompt();
                return;
            }
            if self.start_chat_turn(query) {
                self.history.push(raw);
                self.history_index = None;
                self.reset_prompt();
            }
            return;
        }

        let prompt = Self::expand_command_alias(&self.normalized_prompt());
        if prompt == "clear-notes" {
            self.history.push(format!("/{}", prompt));
            self.history_index = None;
            self.clear_notes_state();
            self.reset_prompt();
            return;
        }

        let Some((command, args)) = Self::parse_command(prompt.as_str()) else {
            self.set_result_panel(
                "Unknown command",
                vec![
                    format!("I don't know `/{}'.", prompt),
                    String::from("Try /note list, /note edit, or /memory search."),
                ],
            );
            self.last_action = format!("Unknown: /{}", prompt);
            self.reset_prompt();
            return;
        };

        if args.trim().is_empty() && Self::command_expects_argument(command.name) {
            self.prompt = format!("/{} ", command.name);
            self.cursor = self.prompt.len();
            self.last_action = format!("Add a target or text for /{}.", command.name);
            return;
        }

        self.history.push(format!("/{}", prompt));
        self.history_index = None;
        self.execute_command(command.name, args);
        self.reset_prompt();
    }

    pub(super) fn reset_prompt(&mut self) {
        self.prompt.clear();
        self.cursor = 0;
        self.selected_suggestion = 0;
        self.suggestion_filter = None;
    }

    pub(super) fn normalize_command_input(prompt: &str) -> String {
        prompt.split_whitespace().collect::<Vec<_>>().join(" ")
    }

    pub(super) fn expand_command_alias(prompt: &str) -> String {
        let trimmed = prompt.trim();
        let aliases = [
            ("find", "search"),
            ("open", "note read"),
            ("read", "note read"),
            ("edit", "note edit"),
            ("new", "note create"),
            ("create", "note create"),
            ("list", "note list"),
            ("ls", "note list"),
            ("path now", "path save"),
            ("path open", "path show"),
            ("path read", "path show"),
            ("path checkout", "path return"),
            ("path back", "path return"),
            ("world save", "path save"),
            ("world now", "path save"),
            ("world list", "path list"),
            ("world show", "path show"),
            ("world open", "path show"),
            ("world read", "path show"),
            ("world return", "path return"),
            ("world checkout", "path return"),
            ("world back", "path return"),
            ("fork now", "path save"),
            ("fork list", "path list"),
            ("fork read", "path show"),
            ("fork checkout", "path return"),
        ];

        for (alias, command) in aliases {
            if trimmed == alias {
                return command.to_string();
            }
            if let Some(rest) = trimmed.strip_prefix(alias) {
                if rest.starts_with(char::is_whitespace) {
                    return format!("{}{}", command, rest);
                }
            }
        }

        trimmed.to_string()
    }

    pub(super) fn command_expects_argument(command: &str) -> bool {
        matches!(
            command,
            "search"
                | "ask"
                | "agent edit"
                | "note move"
                | "path show"
                | "path return"
                | "folder create"
                | "folder delete"
                | "folder notes"
                | "memory save"
                | "memory search"
        )
    }

    pub(super) fn toggle_agent_mode(&mut self) {
        self.agent_mode_enabled = !self.agent_mode_enabled;
        let mode_message = if self.agent_mode_enabled {
            "Agent mode enabled. Aleph will route note-writing requests to tools."
        } else {
            "Chat mode enabled. Aleph will answer without taking note actions."
        };
        self.last_action = if let Err(error) = self.store_agent_mode_enabled() {
            format!("{} (save failed: {})", mode_message, error)
        } else {
            String::from(mode_message)
        };
    }

    pub(super) fn toggle_editor_images(&mut self) {
        self.editor_images_enabled = !self.editor_images_enabled;
        let mode_message = if self.editor_images_enabled {
            "Editor image previews enabled."
        } else {
            "Editor image previews disabled."
        };
        self.last_action = if let Err(error) = self.store_editor_images_enabled() {
            format!("{} (save failed: {})", mode_message, error)
        } else {
            String::from(mode_message)
        };
    }

    pub(super) fn cycle_note_save_target(&mut self) {
        let next = match self.note_save_target {
            NoteSaveTarget::Local => {
                if self.is_obsidian_paired() {
                    NoteSaveTarget::Obsidian
                } else if self.is_strix_connected() {
                    NoteSaveTarget::Strix
                } else {
                    NoteSaveTarget::Local
                }
            }
            NoteSaveTarget::Obsidian => {
                if self.is_strix_connected() {
                    NoteSaveTarget::Strix
                } else {
                    NoteSaveTarget::Local
                }
            }
            NoteSaveTarget::Strix => NoteSaveTarget::Local,
        };

        if next == self.note_save_target {
            self.last_action = String::from(
                "Only Local note saving is available until Obsidian is paired or Strix is connected.",
            );
            return;
        }

        self.note_save_target = next;
        if let Err(error) = self.store_note_save_target() {
            self.last_action = format!(
                "Note save target: {} (save failed: {})",
                self.note_save_target_label(),
                error
            );
            return;
        }
        self.last_action = format!("Note save target: {}", self.note_save_target_label());
    }
}
