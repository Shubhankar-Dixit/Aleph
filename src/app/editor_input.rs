use super::*;
use super::commands_notes::TreeItem;

#[allow(dead_code)]
impl App {
    pub(super) fn handle_full_editor_key(&mut self, key_event: KeyEvent) {
        if self.editing_title {
            self.handle_title_edit_key(key_event);
            return;
        }

        if self.search_state.active {
            self.handle_search_key(key_event);
            return;
        }

        if self.ai_overlay_visible {
            match key_event.code {
                KeyCode::Enter
                    if key_event.kind == KeyEventKind::Press && self.has_pending_ai_edit() =>
                {
                    self.apply_pending_ai_edit();
                    return;
                }
                KeyCode::Char('r')
                    if key_event.kind == KeyEventKind::Press
                        && key_event.modifiers.contains(KeyModifiers::CONTROL)
                        && self.has_pending_ai_edit() =>
                {
                    self.reject_pending_ai_edit();
                    return;
                }
                KeyCode::Esc if key_event.kind == KeyEventKind::Press => {
                    if self.has_pending_ai_edit() {
                        self.reject_pending_ai_edit();
                    } else {
                        self.close_ai_overlay();
                    }
                    return;
                }
                KeyCode::Char(' ')
                    if key_event.kind == KeyEventKind::Press
                        && key_event.modifiers.contains(KeyModifiers::CONTROL) =>
                {
                    if self.has_pending_ai_edit() {
                        self.last_action =
                            String::from("Apply or reject the pending AI edits first.");
                        return;
                    }
                    self.toggle_ai_overlay();
                    return;
                }
                KeyCode::Char('c')
                    if key_event.kind == KeyEventKind::Press
                        && key_event.modifiers.contains(KeyModifiers::CONTROL) =>
                {
                    self.exit_editor();
                    return;
                }
                KeyCode::Char('s')
                    if key_event.kind == KeyEventKind::Press
                        && key_event.modifiers.contains(KeyModifiers::CONTROL) =>
                {
                    self.save_editor();
                    return;
                }
                KeyCode::Char('f')
                    if key_event.kind == KeyEventKind::Press
                        && key_event.modifiers.contains(KeyModifiers::CONTROL) =>
                {
                    self.start_search();
                    return;
                }
                KeyCode::Char('z')
                    if key_event.kind == KeyEventKind::Press
                        && key_event.modifiers.contains(KeyModifiers::CONTROL) =>
                {
                    self.undo();
                    return;
                }
                KeyCode::Char('y')
                    if key_event.kind == KeyEventKind::Press
                        && key_event.modifiers.contains(KeyModifiers::CONTROL) =>
                {
                    self.redo();
                    return;
                }
                KeyCode::Char('w')
                    if key_event.kind == KeyEventKind::Press
                        && key_event.modifiers.contains(KeyModifiers::CONTROL) =>
                {
                    self.toggle_word_wrap();
                    return;
                }
                KeyCode::Char('b')
                    if key_event.kind == KeyEventKind::Press
                        && key_event.modifiers.contains(KeyModifiers::CONTROL) =>
                {
                    self.toggle_cursor_style();
                    return;
                }
                KeyCode::Char('a')
                    if key_event.kind == KeyEventKind::Press
                        && key_event.modifiers.contains(KeyModifiers::CONTROL) =>
                {
                    self.select_all_editor();
                    return;
                }
                _ => {
                    if self.has_pending_ai_edit() {
                        self.last_action =
                            String::from("Apply or reject the pending AI edits first.");
                        return;
                    }
                    self.handle_ai_input_key(key_event);
                    return;
                }
            }
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
            KeyCode::Tab if key_event.kind == KeyEventKind::Press => {
                self.start_title_edit();
            }
            KeyCode::Char(' ')
                if key_event.kind == KeyEventKind::Press
                    && key_event.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                self.toggle_ai_overlay();
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
            _ => self.handle_editor_content_key(key_event),
        }
    }

    pub(super) fn handle_editor_content_key(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Char('a')
                if key_event.kind == KeyEventKind::Press
                    && key_event.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                self.select_all_editor();
            }
            KeyCode::Enter if key_event.kind == KeyEventKind::Press => {
                self.save_undo_state();
                self.clear_editor_selection();
                self.insert_editor_character('\n');
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

    pub(super) fn handle_ai_input_key(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Enter if key_event.kind == KeyEventKind::Press => {
                if !self.ai_input_buffer.trim().is_empty() {
                    self.ghost_submit_instruction();
                }
            }
            KeyCode::Backspace if key_event.kind == KeyEventKind::Press => {
                if self.ai_input_cursor > 0 {
                    let prev = self.ai_input_buffer[..self.ai_input_cursor]
                        .chars()
                        .next_back()
                        .map(|c| c.len_utf8())
                        .unwrap_or(1);
                    self.ai_input_buffer
                        .drain(self.ai_input_cursor - prev..self.ai_input_cursor);
                    self.ai_input_cursor -= prev;
                }
            }
            KeyCode::Delete if key_event.kind == KeyEventKind::Press => {
                if self.ai_input_cursor < self.ai_input_buffer.len() {
                    let next = self.ai_input_buffer[self.ai_input_cursor..]
                        .chars()
                        .next()
                        .map(|c| c.len_utf8())
                        .unwrap_or(1);
                    self.ai_input_buffer
                        .drain(self.ai_input_cursor..self.ai_input_cursor + next);
                }
            }
            KeyCode::Left
                if matches!(key_event.kind, KeyEventKind::Press | KeyEventKind::Repeat) =>
            {
                if self.ai_input_cursor > 0 {
                    let prev = self.ai_input_buffer[..self.ai_input_cursor]
                        .chars()
                        .next_back()
                        .map(|c| c.len_utf8())
                        .unwrap_or(1);
                    self.ai_input_cursor -= prev;
                }
            }
            KeyCode::Right
                if matches!(key_event.kind, KeyEventKind::Press | KeyEventKind::Repeat) =>
            {
                if self.ai_input_cursor < self.ai_input_buffer.len() {
                    let next = self.ai_input_buffer[self.ai_input_cursor..]
                        .chars()
                        .next()
                        .map(|c| c.len_utf8())
                        .unwrap_or(1);
                    self.ai_input_cursor += next;
                }
            }
            KeyCode::Home if key_event.kind == KeyEventKind::Press => self.ai_input_cursor = 0,
            KeyCode::End if key_event.kind == KeyEventKind::Press => {
                self.ai_input_cursor = self.ai_input_buffer.len()
            }
            KeyCode::Char(character)
                if key_event.kind == KeyEventKind::Press
                    && !key_event.modifiers.contains(KeyModifiers::CONTROL)
                    && !key_event.modifiers.contains(KeyModifiers::ALT) =>
            {
                self.ai_input_buffer.insert(self.ai_input_cursor, character);
                self.ai_input_cursor += character.len_utf8();
            }
            _ => {}
        }
    }

    pub(super) fn open_ai_overlay(&mut self) {
        self.ai_overlay_visible = true;
        self.ai_overlay_pulse_ticks = 6;
        self.thinking = false;
        self.thinking_ticks_remaining = 0;
        self.ai_input_buffer.clear();
        self.ai_input_cursor = 0;
        self.ghost_result = None;
        self.pending_ai_edit = None;
        self.ai_draft_create_title = None;
        self.last_action = String::from("Opened AI note editor.");
    }

    pub(super) fn close_ai_overlay(&mut self) {
        self.ai_overlay_visible = false;
        self.ai_overlay_pulse_ticks = 0;
        self.thinking = false;
        self.thinking_ticks_remaining = 0;
        self.ghost_result = None;
        self.pending_ai_edit = None;
        self.ai_draft_create_title = None;
        self.ghost_streaming = false;
        self.ghost_stream_rx = None;
    }

    pub(super) fn toggle_ai_overlay(&mut self) {
        if self.ai_overlay_visible {
            self.close_ai_overlay();
        } else {
            self.open_ai_overlay();
        }
    }

    pub(super) fn open_login_picker(&mut self) {
        self.panel_mode = PanelMode::LoginPicker;
        self.panel_title = String::from("Sign in");
        self.panel_lines = vec![String::from("picker")];
        self.login_picker_selected = 0;
        self.last_action = String::from("Choose a Strix account or model provider.");
    }

    pub(super) fn open_settings_panel(&mut self) {
        self.panel_mode = PanelMode::Settings;
        self.panel_title = String::from("Settings");
        self.panel_lines.clear();
        self.settings_selected = 0;
        self.last_action = String::from("Open settings to manage connections and preferences.");
    }

    pub(super) fn handle_settings_key(&mut self, key_event: KeyEvent) {
        if key_event.kind != KeyEventKind::Press && key_event.kind != KeyEventKind::Repeat {
            return;
        }
        match key_event.code {
            KeyCode::Esc => {
                self.panel_mode = PanelMode::Commands;
                self.panel_title = String::from("Commands");
                self.panel_lines.clear();
                self.last_action = String::from("Closed settings.");
            }
            KeyCode::Up => {
                if self.settings_selected > 0 {
                    self.settings_selected -= 1;
                }
            }
            KeyCode::Down => {
                if self.settings_selected < 6 {
                    self.settings_selected += 1;
                }
            }
            KeyCode::Enter => {
                match self.settings_selected {
                    0 => {
                        let next_provider = match self.ai_provider {
                            AiProvider::OpenRouter => AiProvider::Strix,
                            AiProvider::Strix => AiProvider::OpenRouter,
                        };

                        self.set_ai_provider(next_provider);

                        match next_provider {
                            AiProvider::OpenRouter => {
                                if self.is_openrouter_connected() {
                                    self.rebuild_chat_render_cache();
                                    self.last_action =
                                        String::from("Switched model provider to OpenRouter.");
                                } else if !self.start_openrouter_browser_login() {
                                    self.set_result_panel(
                                        "OpenRouter provider failed",
                                        vec![String::from(
                                            "Unable to start the browser-based OpenRouter authorization flow.",
                                        )],
                                    );
                                    self.last_action =
                                        String::from("OpenRouter provider setup failed.");
                                }
                            }
                            AiProvider::Strix => {
                                if self.is_strix_connected() {
                                    self.last_action =
                                        String::from("Switched model provider to Strix.");
                                } else if !self.start_strix_browser_login() {
                                    self.set_result_panel(
                                        "Strix login failed",
                                        vec![String::from(
                                            "Unable to start the browser-based Strix login flow.",
                                        )],
                                    );
                                    self.last_action = String::from("Strix login failed.");
                                }
                            }
                        }
                    }
                    1 => {
                        self.toggle_agent_mode();
                    }
                    2 => {
                        self.cycle_note_save_target();
                    }
                    3 => {
                        // Pair Obsidian vault
                        self.open_vault_picker();
                    }
                    4 => {
                        // Sign out / Logout
                        self.openrouter_api_key = None;
                        self.strix_access_token = None;
                        if self.note_save_target == NoteSaveTarget::Strix {
                            self.note_save_target = NoteSaveTarget::Local;
                            let _ = self.store_note_save_target();
                        }
                        self.chat_stream_rx = None;
                        self.openrouter_login_rx = None;
                        if let Some(cancel_flag) = &self.openrouter_login_cancel {
                            cancel_flag.store(true, Ordering::Relaxed);
                        }
                        self.openrouter_login_cancel = None;
                        self.strix_login_rx = None;
                        if let Some(cancel_flag) = &self.strix_login_cancel {
                            cancel_flag.store(true, Ordering::Relaxed);
                        }
                        self.strix_login_cancel = None;
                        self.thinking = false;
                        self.thinking_ticks_remaining = 0;
                        self.streaming_buffer.clear();
                        self.streaming_active = false;
                        self.clear_openrouter_api_key();
                        self.clear_strix_access_token();
                        self.refresh_connection_state();
                        self.chat_messages.clear();
                        self.chat_input_buffer.clear();
                        self.chat_input_cursor = 0;
                        self.rebuild_chat_render_cache();
                        self.panel_mode = PanelMode::Commands;
                        self.panel_title = String::from("Commands");
                        self.panel_lines.clear();
                        self.last_action = String::from("Signed out.");
                    }
                    5 => {
                        // Reset & Clear Cache
                        self.reset_and_clear_all();
                        self.panel_mode = PanelMode::Commands;
                        self.panel_title = String::from("Commands");
                        self.panel_lines.clear();
                        self.last_action = String::from("Reset complete. All data cleared.");
                    }
                    6 => {
                        // Close settings
                        self.panel_mode = PanelMode::Commands;
                        self.panel_title = String::from("Commands");
                        self.panel_lines.clear();
                        self.last_action = String::from("Closed settings.");
                    }
                    _ => {}
                }
            }
            KeyCode::Char('c') if key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                self.request_quit();
            }
            _ => {}
        }
    }

    pub(super) fn handle_login_picker_key(&mut self, key_event: KeyEvent) {
        if key_event.kind != KeyEventKind::Press {
            return;
        }
        match key_event.code {
            KeyCode::Esc => {
                self.panel_mode = PanelMode::Commands;
                self.panel_title = String::from("Commands");
                self.panel_lines.clear();
                self.last_action = String::from("Cancelled login.");
            }
            KeyCode::Up => {
                if self.login_picker_selected > 0 {
                    self.login_picker_selected -= 1;
                }
            }
            KeyCode::Down => {
                if self.login_picker_selected < 1 {
                    self.login_picker_selected += 1;
                }
            }
            KeyCode::Enter => {
                match self.login_picker_selected {
                    0 => {
                        // OpenRouter: start browser authorization for a model-provider API key
                        self.panel_mode = PanelMode::Commands;
                        if !self.start_openrouter_browser_login() {
                            self.set_result_panel(
                                "OpenRouter provider failed",
                                vec![String::from(
                                    "Unable to start the browser-based OpenRouter authorization flow.",
                                )],
                            );
                            self.last_action = String::from("OpenRouter provider setup failed.");
                        }
                    }
                    1 => {
                        self.panel_mode = PanelMode::Commands;
                        if !self.start_strix_browser_login() {
                            self.set_result_panel(
                                "Strix login failed",
                                vec![String::from(
                                    "Unable to start the browser-based Strix login flow.",
                                )],
                            );
                            self.last_action = String::from("Strix login failed.");
                        }
                    }
                    _ => {}
                }
            }
            KeyCode::Char('c') if key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                self.request_quit();
            }
            _ => {}
        }
    }

    pub(super) fn handle_note_list_key(&mut self, key_event: KeyEvent) {
        if key_event.kind != KeyEventKind::Press && key_event.kind != KeyEventKind::Repeat {
            return;
        }
        match key_event.code {
            KeyCode::Esc => {
                self.note_list_pending_delete = None;
                self.panel_mode = PanelMode::Commands;
                self.panel_title = String::from("Commands");
                self.panel_lines.clear();
                self.last_action = String::from("Exited note list.");
            }
            KeyCode::Up => {
                if self.note_list_selected > 0 {
                    self.note_list_pending_delete = None;
                    self.note_list_selected -= 1;
                    self.last_action = format!("Selected item {}", self.note_list_selected + 1);
                }
            }
            KeyCode::Down => {
                if self.note_list_selected + 1 < self.panel_lines.len() {
                    self.note_list_pending_delete = None;
                    self.note_list_selected += 1;
                    self.last_action = format!("Selected item {}", self.note_list_selected + 1);
                }
            }
            KeyCode::Enter => {
                if self.note_list_delete_is_pending() {
                    self.confirm_or_stage_note_delete();
                    return;
                }
                // Check if selected item is a note (not a folder marker)
                if let Some(&note_index) = self.note_list_indices.get(self.note_list_selected) {
                    if note_index != usize::MAX {
                        self.open_note_editor(note_index);
                    }
                }
            }
            KeyCode::Char(' ') if key_event.kind == KeyEventKind::Press => {
                // Toggle folder expansion
                if let Some(&note_index) = self.note_list_indices.get(self.note_list_selected) {
                    if note_index == usize::MAX {
                        // This is a folder - need to extract folder ID from the line
                        // For now, we'll rebuild the tree and toggle based on line content
                        self.toggle_folder_at_selection();
                    }
                }
            }
            KeyCode::Delete | KeyCode::Backspace if key_event.kind == KeyEventKind::Press => {
                self.confirm_or_stage_note_delete();
            }
            KeyCode::Char('d') | KeyCode::Char('D')
                if key_event.kind == KeyEventKind::Press && self.note_list_delete_is_pending() =>
            {
                self.confirm_or_stage_note_delete();
            }
            KeyCode::Char('c') if key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                self.request_quit();
            }
            _ => {}
        }
    }
    
    fn toggle_folder_at_selection(&mut self) {
        // Extract folder ID from the current selection
        // Since we use usize::MAX as a marker, we need to track which folder is at which position
        // For simplicity, we'll rebuild the tree to find the folder
        let mut tree_items: Vec<TreeItem> = Vec::new();
        let root_folders: Vec<&Folder> = self.folders.iter().filter(|f| f.parent_id.is_none()).collect();
        
        let uncategorized_notes: Vec<usize> = self.notes
            .iter()
            .enumerate()
            .filter(|(_, note)| note.folder_id.is_none())
            .map(|(index, _)| index)
            .collect();
        
        if !uncategorized_notes.is_empty() {
            tree_items.push(TreeItem::Folder {
                id: 0,
                name: String::from("Uncategorized"),
                depth: 0,
                expanded: self.expanded_folders.contains(&0),
                note_count: uncategorized_notes.len(),
            });
            
            if self.expanded_folders.contains(&0) {
                for &note_index in &uncategorized_notes {
                    tree_items.push(TreeItem::Note {
                        index: note_index,
                        depth: 1,
                    });
                }
            }
        }
        
        for folder in &root_folders {
            self.build_folder_tree_items(&mut tree_items, folder.id, 0);
        }
        
        // Find the folder at the current selection
        if let Some(item) = tree_items.get(self.note_list_selected) {
            if let TreeItem::Folder { id, .. } = item {
                if self.expanded_folders.contains(id) {
                    self.expanded_folders.retain(|&x| x != *id);
                } else {
                    self.expanded_folders.push(*id);
                }
                self.open_note_list_panel();
                self.last_action = format!("Toggled folder expansion");
            }
        }
    }

    pub(super) fn confirm_or_stage_note_delete(&mut self) {
        let Some(&note_index) = self.note_list_indices.get(self.note_list_selected) else {
            self.last_action = String::from("No note selected to delete.");
            return;
        };
        let note_title = self
            .notes
            .get(note_index)
            .map(|note| note.title.clone())
            .unwrap_or_else(|| String::from("Untitled note"));
        if self.note_list_pending_delete == Some(note_index) {
            match self.delete_note_at_index(note_index) {
                Ok(title) => {
                    self.open_note_list_panel();
                    self.last_action = format!("Deleted note: {}", title);
                }
                Err(error) => {
                    self.note_list_pending_delete = None;
                    self.last_action = format!("Delete failed: {}", error);
                }
            }
        } else {
            self.note_list_pending_delete = Some(note_index);
            self.last_action = format!(
                "Press Delete, Enter, or d again to delete '{}'. Esc or move to cancel.",
                note_title
            );
        }
    }

    pub(super) fn handle_vault_picker_key(&mut self, key_event: KeyEvent) {
        if key_event.kind != KeyEventKind::Press && key_event.kind != KeyEventKind::Repeat {
            return;
        }
        match key_event.code {
            KeyCode::Esc => {
                self.panel_mode = PanelMode::Commands;
                self.panel_title = String::from("Commands");
                self.panel_lines.clear();
                self.last_action = String::from("Cancelled Obsidian pairing.");
            }
            KeyCode::Up => {
                if self.obsidian_vault_selected > 0 {
                    self.obsidian_vault_selected -= 1;
                    self.last_action =
                        format!("Selected vault {}", self.obsidian_vault_selected + 1);
                }
            }
            KeyCode::Down => {
                if self.obsidian_vault_selected + 1 < self.obsidian_vaults.len() {
                    self.obsidian_vault_selected += 1;
                    self.last_action =
                        format!("Selected vault {}", self.obsidian_vault_selected + 1);
                }
            }
            KeyCode::Enter => {
                if let Some(vault) = self
                    .obsidian_vaults
                    .get(self.obsidian_vault_selected)
                    .cloned()
                {
                    match self.pair_obsidian_vault(vault.path) {
                        Ok(message) => {
                            self.panel_mode = PanelMode::ObsidianSyncConfirm;
                            self.panel_title = String::from("Sync Obsidian?");
                            self.panel_lines = vec![
                                message,
                                String::from("Would you like to import notes from this vault now?"),
                            ];
                            self.last_action = String::from("Paired Obsidian vault.");
                        }
                        Err(error) => {
                            self.set_result_panel("Obsidian pairing failed", vec![error]);
                            self.last_action = String::from("Obsidian pairing failed.");
                        }
                    }
                }
            }
            KeyCode::Char('c') if key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                self.request_quit();
            }
            _ => {}
        }
    }

    pub(super) fn handle_obsidian_sync_confirm_key(&mut self, key_event: KeyEvent) {
        if key_event.kind != KeyEventKind::Press {
            return;
        }
        match key_event.code {
            KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('N') => {
                self.panel_mode = PanelMode::Commands;
                self.panel_title = String::from("Commands");
                self.panel_lines.clear();
                self.last_action = String::from("Skipped Obsidian sync.");
            }
            KeyCode::Enter | KeyCode::Char('y') | KeyCode::Char('Y') => {
                match self.sync_obsidian_notes() {
                    Ok(count) => {
                        let vault = self
                            .obsidian_vault_path
                            .as_ref()
                            .map(|path| path.display().to_string())
                            .unwrap_or_else(|| String::from("unknown vault"));
                        self.set_result_panel(
                            "Obsidian sync",
                            vec![
                                format!("Imported {} Markdown notes.", count),
                                format!("Vault: {}", vault),
                                String::from(
                                    "Use /note list, /search, /note edit, and /obsidian open.",
                                ),
                            ],
                        );
                        self.last_action = format!("Synced {} Obsidian notes.", count);
                    }
                    Err(error) => {
                        self.set_result_panel("Obsidian sync failed", vec![error]);
                        self.last_action = String::from("Obsidian sync failed.");
                    }
                }
                self.panel_mode = PanelMode::Commands;
            }
            _ => {}
        }
    }
}
