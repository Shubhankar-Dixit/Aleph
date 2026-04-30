use super::*;

#[allow(dead_code)]
impl App {
    pub fn new() -> Self {
        let openrouter_api_key = Self::load_openrouter_api_key();
        let strix_access_token = Self::load_strix_access_token();
        let obsidian_vault_path = Self::load_obsidian_vault_path();
        let obsidian_vaults = Self::discover_obsidian_vaults();
        let connected = openrouter_api_key.is_some() || strix_access_token.is_some();
        let default_ai_provider = if openrouter_api_key.is_some() {
            AiProvider::OpenRouter
        } else if strix_access_token.is_some() {
            AiProvider::Strix
        } else {
            AiProvider::OpenRouter
        };
        let ai_provider = Self::load_ai_provider().unwrap_or(default_ai_provider);
        let default_note_save_target = if strix_access_token.is_some() {
            NoteSaveTarget::Strix
        } else if obsidian_vault_path.is_some() {
            NoteSaveTarget::Obsidian
        } else {
            NoteSaveTarget::Local
        };
        let note_save_target = Self::load_note_save_target()
            .filter(|target| {
                Self::note_save_target_is_available(
                    *target,
                    obsidian_vault_path.is_some(),
                    strix_access_token.is_some(),
                )
            })
            .unwrap_or(default_note_save_target);

        let mut app = Self {
            started_at: Instant::now(),
            tick: 0,
            quit: false,
            prompt: String::new(),
            cursor: 0,
            history: Vec::new(),
            history_index: None,
            selected_suggestion: 0,
            last_action: String::from("Ready to accept input."),
            connected,
            folders: Vec::new(),
            notes: Self::load_local_notes().unwrap_or_else(|_| Self::default_local_notes()),
            memories: Vec::new(),
            canvases: Vec::new(),
            selected_note: 0,
            current_folder_id: None,
            panel_mode: PanelMode::Commands,
            panel_title: String::from("Commands"),
            panel_lines: Vec::new(),
            editor_note_index: None,
            editor_buffer: String::new(),
            editor_cursor: 0,
            thinking: false,
            thinking_ticks_remaining: 0,
            ai_overlay_visible: false,
            ai_overlay_pulse_ticks: 0,
            save_shimmer_ticks: 0,
            ai_input_buffer: String::new(),
            ai_input_cursor: 0,
            suggestion_filter: None,
            editor_scroll_offset: 0,
            editor_word_wrap: true,
            editor_images_enabled: Self::load_editor_images_enabled().unwrap_or(false),
            editor_cursor_style: CursorStyle::Line,
            editor_selection: Selection::default(),
            undo_stack: VecDeque::with_capacity(100),
            redo_stack: VecDeque::with_capacity(100),
            search_state: SearchState {
                query: String::new(),
                matches: Vec::new(),
                current_match: None,
                active: false,
            },
            chat_messages: Vec::new(),
            chat_input_buffer: String::new(),
            chat_input_cursor: 0,
            chat_scroll_offset: 0,
            openrouter_api_key,
            strix_access_token,
            chat_stream_rx: None,
            openrouter_login_rx: None,
            openrouter_login_cancel: None,
            strix_login_rx: None,
            strix_login_cancel: None,
            obsidian_vault_path,
            obsidian_vaults,
            obsidian_vault_selected: 0,
            note_save_target,
            ai_provider,
            strix_logs: Vec::new(),
            streaming_buffer: String::new(),
            streaming_active: false,
            chat_render_cache: Vec::new(),
            chat_render_dirty: false,
            chat_cache_stable_len: 0,
            agent_mode_enabled: Self::load_agent_mode_enabled().unwrap_or(true),
            login_picker_selected: 0,
            settings_selected: 0,
            pending_agent_query: None,
            pending_agent_decision: None,
            ghost_stream_rx: None,
            ghost_streaming: false,
            ghost_result: None,
            pending_ai_edit: None,
            ai_draft_create_title: None,
            note_list_selected: 0,
            note_list_indices: Vec::new(),
            note_list_pending_delete: None,
            editing_title: false,
            title_buffer: String::new(),
            title_cursor: 0,
            expanded_folders: Vec::new(),
        };

        if app.strix_access_token.is_some() {
            if let Ok(notes) = Self::load_cached_strix_notes() {
                if !notes.is_empty() {
                    app.merge_strix_notes(notes);
                    app.selected_note = 0;
                    app.add_strix_log("Loaded cached Strix notes");
                    app.last_action =
                        String::from("Loaded cached Strix notes. Run /sync to refresh.");
                }
            }
        }

        app.rebuild_chat_render_cache();
        app
    }

    pub(super) fn default_local_notes() -> Vec<Note> {
        let content = String::from(
            "# Welcome to Aleph\n\n\
    Aleph is a terminal workspace for notes, search, AI assistance, and sync. Start with `/settings` to choose how notes are saved, pair Obsidian, or connect Strix. Use `/note list` to browse notes, `/note create <title> :: <body>` to start writing, `/note edit` to edit the selected note, and `/ask <question>` when you want help from the selected AI provider.\n\n\
    To use Obsidian, open `/obsidian pair`, choose your vault, then confirm the sync prompt to import Markdown notes. You can run `/obsidian sync` again later whenever you want to refresh Aleph from the paired vault.",
        );

        vec![Note {
            id: 1,
            remote_id: None,
            obsidian_path: None,
            title: String::from("Welcome to Aleph"),
            raw_content: content.clone(),
            content,
            updated_at: String::from("seed"),
            folder_id: None,
        }]
    }

    pub fn run_cli_command(&mut self, args: &[String]) -> Result<Vec<String>, String> {
        if args.is_empty() {
            return Ok(vec![
                String::from("Usage: aleph notes <list|search|read|write|append|create> ..."),
                String::from("Examples:"),
                String::from("  aleph notes search roadmap"),
                String::from("  aleph notes read <id>"),
                String::from("  aleph notes write <id> -   # content from stdin"),
            ]);
        }

        let area = args[0].as_str();
        if area == "sync" {
            let count = self.sync_strix_notes()?;
            return Ok(vec![format!("Synced {} notes from Strix.", count)]);
        }

        if area == "obsidian" {
            self.refresh_obsidian_vaults();
            return self.run_obsidian_cli_command(&args[1..]);
        }

        if area != "notes" && area != "note" {
            return Err(format!(
                "Unknown Aleph CLI area '{}'. Try 'notes' or 'obsidian'.",
                area
            ));
        }

        let action = args.get(1).map(|value| value.as_str()).unwrap_or("list");
        match action {
            "list" => {
                self.ensure_cached_strix_notes_loaded();
                if self.notes.is_empty() {
                    self.sync_strix_notes()?;
                }
                Ok(self
                    .notes
                    .iter()
                    .map(|note| {
                        format!(
                            "{}\t{}\t{}",
                            note.obsidian_path
                                .as_ref()
                                .map(|path| format!("obsidian:{}", path.display()))
                                .or_else(|| note.remote_id.clone())
                                .unwrap_or_else(|| String::from("local-only")),
                            note.title,
                            Self::preview_text(&note.content, 120)
                        )
                    })
                    .collect())
            }
            "search" => {
                let query = args.get(2..).unwrap_or(&[]).join(" ");
                self.ensure_cached_strix_notes_loaded();
                if self.notes.is_empty() {
                    self.sync_strix_notes()?;
                }
                Ok(self.search_notes(&query))
            }
            "read" => {
                let id = args
                    .get(2)
                    .ok_or_else(|| String::from("Usage: aleph notes read <id|title>"))?;
                self.ensure_cached_strix_notes_loaded();
                let note = self
                    .resolve_note_index(id)
                    .and_then(|index| self.notes.get(index).cloned())
                    .map(Ok)
                    .unwrap_or_else(|| self.load_strix_note(id, true))?;
                Ok(vec![
                    format!("# {}", note.title),
                    Self::note_source_label(&note),
                    String::new(),
                    note.content,
                ])
            }
            "write" => {
                let id = args
                    .get(2)
                    .ok_or_else(|| String::from("Usage: aleph notes write <id|title> <content>"))?;
                let content = args.get(3..).unwrap_or(&[]).join(" ");
                if content.is_empty() {
                    return Err(String::from(
                        "Provide content or pass '-' to read content from stdin.",
                    ));
                }
                self.ensure_cached_strix_notes_loaded();
                let local_index = self.resolve_note_index(id);
                let mut note = local_index
                    .and_then(|index| self.notes.get(index).cloned())
                    .map(Ok)
                    .unwrap_or_else(|| self.load_strix_note(id, true))?;
                note.content = content;
                note.raw_content = note.content.clone();
                if let Some(index) = local_index {
                    if let Some(slot) = self.notes.get_mut(index) {
                        *slot = note.clone();
                    }
                    self.write_note_to_obsidian(index)?;
                    Self::save_local_notes(&self.notes)?;
                }
                let updated = if note.remote_id.is_some() {
                    self.update_strix_note(&note)?
                } else {
                    note.clone()
                };
                if updated.remote_id.is_some() || local_index.is_none() {
                    self.upsert_synced_note(updated.clone());
                }
                Ok(vec![format!(
                    "Updated {} ({})",
                    updated.title,
                    Self::note_source_label(&updated)
                )])
            }
            "append" => {
                let id = args.get(2).ok_or_else(|| {
                    String::from("Usage: aleph notes append <id|title> <content>")
                })?;
                let content = args.get(3..).unwrap_or(&[]).join(" ");
                if content.is_empty() {
                    return Err(String::from(
                        "Provide content or pass '-' to read content from stdin.",
                    ));
                }
                self.ensure_cached_strix_notes_loaded();
                let local_index = self.resolve_note_index(id);
                let mut note = local_index
                    .and_then(|index| self.notes.get(index).cloned())
                    .map(Ok)
                    .unwrap_or_else(|| self.load_strix_note(id, true))?;
                if !note.content.is_empty() {
                    note.content.push('\n');
                }
                note.content.push_str(&content);
                note.raw_content = note.content.clone();
                if let Some(index) = local_index {
                    if let Some(slot) = self.notes.get_mut(index) {
                        *slot = note.clone();
                    }
                    self.write_note_to_obsidian(index)?;
                    Self::save_local_notes(&self.notes)?;
                }
                let updated = if note.remote_id.is_some() {
                    self.update_strix_note(&note)?
                } else {
                    note.clone()
                };
                if updated.remote_id.is_some() || local_index.is_none() {
                    self.upsert_synced_note(updated.clone());
                }
                Ok(vec![format!(
                    "Appended to {} ({})",
                    updated.title,
                    Self::note_source_label(&updated)
                )])
            }
            "create" => {
                let title = args
                    .get(2)
                    .map(|title| title.as_str())
                    .filter(|title| !title.trim().is_empty())
                    .unwrap_or("Untitled note");
                let content = args.get(3..).unwrap_or(&[]).join(" ");
                let mut note = self.create_strix_note(title, &content)?;
                if let Some(path) = self.obsidian_note_path_for_title(title) {
                    if let Some(parent) = path.parent() {
                        fs::create_dir_all(parent).map_err(|error| {
                            format!("failed to create '{}': {}", parent.display(), error)
                        })?;
                    }
                    fs::write(&path, &content).map_err(|error| {
                        format!("failed to write '{}': {}", path.display(), error)
                    })?;
                    note.obsidian_path = Some(path);
                }
                self.upsert_synced_note(note.clone());
                Ok(vec![format!(
                    "Created {} ({})",
                    note.title,
                    Self::note_source_label(&note)
                )])
            }
            _ => Err(format!("Unknown notes action '{}'.", action)),
        }
    }

    pub(super) fn run_obsidian_cli_command(
        &mut self,
        args: &[String],
    ) -> Result<Vec<String>, String> {
        let action = args.first().map(|value| value.as_str()).unwrap_or("status");
        match action {
            "pair" => {
                self.refresh_obsidian_vaults();
                let target = args.get(1).map(|value| value.as_str()).unwrap_or("");
                let path = if target.is_empty() {
                    match self.obsidian_vaults.as_slice() {
                        [vault] => vault.path.clone(),
                        [] => {
                            return Err(String::from(
                                "No Obsidian vaults found. Run `aleph obsidian pair <path>`.",
                            ))
                        }
                        _ => {
                            return Ok(std::iter::once(String::from(
                                "Multiple vaults found. Re-run with a number or name:",
                            ))
                            .chain(self.format_obsidian_vault_lines())
                            .collect())
                        }
                    }
                } else {
                    self.resolve_obsidian_vault_target(target)
                        .unwrap_or_else(|| PathBuf::from(Self::expand_home(target)))
                };
                let message = self.pair_obsidian_vault(path)?;
                Ok(vec![
                    message,
                    String::from("Run `aleph obsidian sync` to import notes."),
                ])
            }
            "vaults" | "list" => {
                self.refresh_obsidian_vaults();
                let mut lines = self.format_obsidian_vault_lines();
                if lines.is_empty() {
                    lines.push(String::from(
                        "No Obsidian vaults found. Run `aleph obsidian pair <path>`.",
                    ));
                }
                Ok(lines)
            }
            "sync" => {
                let count = self.sync_obsidian_notes()?;
                Ok(vec![format!("Imported {} Obsidian notes.", count)])
            }
            "status" => Ok(vec![
                format!("Obsidian: {}", self.obsidian_status_label()),
                format!("Detected vaults: {}", self.obsidian_vaults.len()),
                format!("Config: {}", Self::obsidian_config_path().display()),
                format!(
                    "Pairing fallback: {}",
                    Self::obsidian_pairing_path().display()
                ),
            ]),
            "open" => {
                let target = args.get(1..).unwrap_or(&[]).join(" ");
                self.open_obsidian_target(&target)
                    .map(|message| vec![message])
            }
            _ => Err(format!(
                "Unknown obsidian action '{}'. Try pair, vaults, sync, status, or open.",
                action
            )),
        }
    }

    pub fn on_tick(&mut self) {
        self.tick = self.tick.wrapping_add(1);
        if self.thinking_ticks_remaining > 0 {
            self.thinking_ticks_remaining -= 1;
        }

        let mut login_finished = false;
        while !login_finished {
            let result = match self.openrouter_login_rx.as_ref() {
                Some(receiver) => receiver.try_recv(),
                None => break,
            };

            match result {
                Ok(Ok(api_key)) => {
                    self.set_ai_provider(AiProvider::OpenRouter);
                    match self.store_openrouter_api_key(&api_key) {
                        Ok(()) => {
                            self.openrouter_api_key = Some(api_key);
                            self.refresh_connection_state();
                            self.rebuild_chat_render_cache();
                            self.set_result_panel(
                                "OpenRouter provider",
                                vec![
                                    String::from(
                                        "OpenRouter authorization completed successfully.",
                                    ),
                                    String::from(
                                        "The API key has been stored locally as a model provider.",
                                    ),
                                    String::from("AI chat can use OpenRouter now."),
                                ],
                            );
                            self.last_action =
                                String::from("Configured OpenRouter as a model provider.");
                        }
                        Err(error) => {
                            self.openrouter_api_key = None;
                            self.refresh_connection_state();
                            self.set_result_panel("OpenRouter provider failed", vec![error]);
                            self.last_action = String::from("OpenRouter provider setup failed.");
                        }
                    }

                    self.openrouter_login_rx = None;
                    self.openrouter_login_cancel = None;
                    login_finished = true;
                }
                Ok(Err(error)) => {
                    self.refresh_connection_state();
                    self.set_result_panel("OpenRouter provider failed", vec![error]);
                    self.last_action = String::from("OpenRouter provider setup failed.");
                    self.openrouter_login_rx = None;
                    self.openrouter_login_cancel = None;
                    login_finished = true;
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.refresh_connection_state();
                    self.set_result_panel(
                        "OpenRouter provider failed",
                        vec![String::from(
                            "The browser login flow disconnected before completion.",
                        )],
                    );
                    self.last_action = String::from("OpenRouter provider setup disconnected.");
                    self.openrouter_login_rx = None;
                    self.openrouter_login_cancel = None;
                    login_finished = true;
                }
            }
        }

        let mut strix_login_finished = false;
        while !strix_login_finished {
            let result = match self.strix_login_rx.as_ref() {
                Some(receiver) => receiver.try_recv(),
                None => break,
            };

            match result {
                Ok(Ok(access_token)) => {
                    self.set_ai_provider(AiProvider::Strix);
                    match self.store_strix_access_token(&access_token) {
                        Ok(()) => {
                            self.strix_access_token = Some(access_token);
                            self.note_save_target = NoteSaveTarget::Strix;
                            let _ = self.store_note_save_target();
                            self.refresh_connection_state();
                            self.add_strix_log("Browser login completed successfully");
                            self.set_result_panel(
                                "Strix login",
                                vec![
                                    String::from("Strix browser login completed successfully."),
                                    String::from(
                                        "The native app access token has been stored locally.",
                                    ),
                                    String::from(
                                        "Aleph can now call Strix-native APIs as they come online.",
                                    ),
                                ],
                            );
                            self.last_action =
                                String::from("Connected to Strix via browser login.");
                        }
                        Err(error) => {
                            self.strix_access_token = None;
                            self.refresh_connection_state();
                            self.set_result_panel("Strix login failed", vec![error]);
                            self.last_action = String::from("Strix login failed.");
                        }
                    }

                    self.strix_login_rx = None;
                    self.strix_login_cancel = None;
                    strix_login_finished = true;
                }
                Ok(Err(error)) => {
                    self.refresh_connection_state();
                    self.set_result_panel("Strix login failed", vec![error]);
                    self.last_action = String::from("Strix login failed.");
                    self.strix_login_rx = None;
                    self.strix_login_cancel = None;
                    strix_login_finished = true;
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.refresh_connection_state();
                    self.set_result_panel(
                        "Strix login failed",
                        vec![String::from(
                            "The browser login flow disconnected before completion.",
                        )],
                    );
                    self.last_action = String::from("Strix login disconnected.");
                    self.strix_login_rx = None;
                    self.strix_login_cancel = None;
                    strix_login_finished = true;
                }
            }
        }

        let mut stream_finished = false;
        while !stream_finished {
            let result = match self.chat_stream_rx.as_ref() {
                Some(receiver) => receiver.try_recv(),
                None => break,
            };

            match result {
                Ok(ChatStreamUpdate::Delta(chunk)) => {
                    self.streaming_active = true;
                    self.streaming_buffer.push_str(&chunk);
                    if let Some(message) = self
                        .chat_messages
                        .iter_mut()
                        .rev()
                        .find(|message| message.role == "assistant")
                    {
                        message.content.push_str(&chunk);
                    }
                    self.chat_render_dirty = true;
                    self.thinking = true;
                }
                Ok(ChatStreamUpdate::Done) => {
                    if self.streaming_buffer.trim().is_empty() {
                        if let Some(message) = self
                            .chat_messages
                            .iter_mut()
                            .rev()
                            .find(|message| message.role == "assistant")
                        {
                            message.content = String::from("Aleph returned no content.");
                        }
                    }

                    self.streaming_buffer.clear();
                    self.streaming_active = false;
                    self.rebuild_chat_render_cache();
                    self.chat_render_dirty = false;
                    self.thinking = false;
                    self.thinking_ticks_remaining = 0;
                    self.chat_stream_rx = None;
                    self.last_action = String::from("AI response received.");
                    stream_finished = true;
                }
                Ok(ChatStreamUpdate::Error(error)) => {
                    if let Some(message) = self
                        .chat_messages
                        .iter_mut()
                        .rev()
                        .find(|message| message.role == "assistant")
                    {
                        if message.content.trim().is_empty() {
                            message.content = format!("AI chat failed: {}", error);
                        } else {
                            message.content.push_str("\n\n");
                            message.content.push_str(&format!("[AI error: {}]", error));
                        }
                    } else {
                        self.push_chat_message("assistant", format!("AI chat failed: {}", error));
                    }

                    self.streaming_buffer.clear();
                    self.streaming_active = false;
                    self.rebuild_chat_render_cache();
                    self.chat_render_dirty = false;
                    self.thinking = false;
                    self.thinking_ticks_remaining = 0;
                    self.chat_stream_rx = None;
                    self.last_action = String::from("AI request failed.");
                    stream_finished = true;
                }
                Err(TryRecvError::Empty) => {
                    self.thinking = true;
                    break;
                }
                Err(TryRecvError::Disconnected) => {
                    if let Some(message) = self
                        .chat_messages
                        .iter_mut()
                        .rev()
                        .find(|message| message.role == "assistant")
                    {
                        message.content =
                            String::from("AI chat disconnected before a response arrived.");
                    }

                    self.streaming_buffer.clear();
                    self.streaming_active = false;
                    self.rebuild_chat_render_cache();
                    self.chat_render_dirty = false;
                    self.thinking = false;
                    self.thinking_ticks_remaining = 0;
                    self.chat_stream_rx = None;
                    self.last_action = String::from("AI request disconnected.");
                    stream_finished = true;
                }
            }
        }

        if self.chat_render_dirty {
            self.rebuild_chat_render_cache_streaming();
            self.chat_render_dirty = false;
        }

        self.process_ghost_stream();

        if self.ai_overlay_visible && self.ai_overlay_pulse_ticks > 0 {
            self.ai_overlay_pulse_ticks -= 1;
        }
        if self.save_shimmer_ticks > 0 {
            self.save_shimmer_ticks -= 1;
        }
    }

    pub fn request_quit(&mut self) {
        self.quit = true;
    }

    pub fn should_quit(&self) -> bool {
        self.quit
    }

    pub fn tick(&self) -> u64 {
        self.tick
    }

    pub fn spinner(&self) -> &'static str {
        const FRAMES: [&str; 4] = ["◐", "◓", "◑", "◒"];
        FRAMES[(self.tick as usize) % FRAMES.len()]
    }

    pub fn uptime(&self) -> String {
        let seconds = self.started_at.elapsed().as_secs();
        format!("{}s", seconds)
    }

    pub fn prompt(&self) -> &str {
        &self.prompt
    }

    pub fn is_prompt_empty(&self) -> bool {
        self.prompt.is_empty()
    }

    pub fn is_typing_command(&self) -> bool {
        self.prompt.starts_with('/')
    }

    pub fn prompt_before_cursor(&self) -> &str {
        &self.prompt[..self.cursor]
    }

    pub fn prompt_after_cursor(&self) -> &str {
        &self.prompt[self.cursor..]
    }

    pub fn is_thinking(&self) -> bool {
        self.thinking
    }

    pub fn is_full_editor(&self) -> bool {
        self.panel_mode == PanelMode::FullEditor
    }

    pub fn ai_overlay_visible(&self) -> bool {
        self.ai_overlay_visible
    }

    pub fn ai_overlay_pulse_ticks(&self) -> u8 {
        self.ai_overlay_pulse_ticks
    }

    pub fn save_shimmer_ticks(&self) -> u8 {
        self.save_shimmer_ticks
    }

    pub fn ai_input_buffer(&self) -> &str {
        &self.ai_input_buffer
    }

    pub fn ai_input_cursor(&self) -> usize {
        self.ai_input_cursor
    }

    pub fn is_editing_note(&self) -> bool {
        self.panel_mode == PanelMode::NoteEditor
    }

    pub fn panel_mode(&self) -> PanelMode {
        self.panel_mode
    }

    pub fn is_ai_chat(&self) -> bool {
        self.panel_mode == PanelMode::AiChat
    }

    pub fn chat_messages(&self) -> &[ChatMessage] {
        &self.chat_messages
    }

    pub fn chat_input_buffer(&self) -> &str {
        &self.chat_input_buffer
    }

    pub fn chat_input_cursor(&self) -> usize {
        self.chat_input_cursor
    }

    pub fn chat_scroll_offset(&self) -> usize {
        self.chat_scroll_offset
    }

    pub fn chat_render_lines(&self) -> &[Line<'static>] {
        &self.chat_render_cache
    }

    pub fn panel_title(&self) -> &str {
        &self.panel_title
    }

    pub fn panel_lines(&self) -> &[String] {
        &self.panel_lines
    }

    pub fn editor_buffer(&self) -> &str {
        &self.editor_buffer
    }

    pub fn editor_display_buffer(&self) -> &str {
        if let Some(proposal) = self.pending_ai_edit.as_ref() {
            return &proposal.proposed;
        }

        if self.ghost_streaming {
            if let Some(result) = self.ghost_result.as_deref() {
                if !result.trim().is_empty() {
                    return result;
                }
            }
        }

        &self.editor_buffer
    }

    pub fn editor_cursor(&self) -> usize {
        self.editor_cursor
    }

    pub fn editor_display_cursor(&self) -> usize {
        if self.has_live_ai_editor_preview() {
            self.editor_display_buffer().len()
        } else {
            self.editor_cursor
        }
    }

    pub fn has_live_ai_editor_preview(&self) -> bool {
        self.pending_ai_edit.is_some()
            || (self.ghost_streaming
                && self
                    .ghost_result
                    .as_deref()
                    .map(|result| !result.trim().is_empty())
                    .unwrap_or(false))
    }

    pub fn editor_scroll_offset(&self) -> usize {
        self.editor_scroll_offset
    }

    pub fn editor_word_wrap(&self) -> bool {
        self.editor_word_wrap
    }

    pub fn editor_images_enabled(&self) -> bool {
        self.editor_images_enabled
    }

    pub fn editor_image_base_dir(&self) -> Option<PathBuf> {
        self.editor_note_index
            .and_then(|index| self.notes.get(index))
            .and_then(|note| note.obsidian_path.as_ref())
            .and_then(|path| path.parent())
            .map(Path::to_path_buf)
    }

    pub fn editor_cursor_style(&self) -> CursorStyle {
        self.editor_cursor_style
    }

    pub fn editor_selection(&self) -> &Selection {
        &self.editor_selection
    }

    pub fn search_state(&self) -> &SearchState {
        &self.search_state
    }

    pub fn editor_note_title(&self) -> Option<&str> {
        self.editor_note_index
            .and_then(|index| self.notes.get(index))
            .map(|note| note.title.as_str())
    }

    pub fn active_note(&self) -> Option<&Note> {
        self.notes.get(self.selected_note)
    }

    pub fn ai_provider(&self) -> AiProvider {
        self.ai_provider
    }

    pub fn ai_provider_label(&self) -> &'static str {
        self.model_provider_label()
    }

    pub fn model_provider_label(&self) -> &'static str {
        match self.ai_provider {
            AiProvider::OpenRouter => "OpenRouter",
            AiProvider::Strix => "Strix",
        }
    }

    pub fn strix_logs(&self) -> &[String] {
        &self.strix_logs
    }

    pub fn streaming_buffer(&self) -> &str {
        &self.streaming_buffer
    }

    pub fn is_streaming(&self) -> bool {
        self.streaming_active
    }

    pub fn is_agent_mode_enabled(&self) -> bool {
        self.agent_mode_enabled
    }

    pub fn login_picker_selected(&self) -> usize {
        self.login_picker_selected
    }

    pub fn is_login_picker(&self) -> bool {
        self.panel_mode == PanelMode::LoginPicker
    }

    pub fn is_settings(&self) -> bool {
        self.panel_mode == PanelMode::Settings
    }

    pub fn settings_selected(&self) -> usize {
        self.settings_selected
    }

    pub fn is_note_list(&self) -> bool {
        self.panel_mode == PanelMode::NoteList
    }

    pub fn is_vault_picker(&self) -> bool {
        self.panel_mode == PanelMode::VaultPicker
    }

    pub fn is_obsidian_sync_confirm(&self) -> bool {
        self.panel_mode == PanelMode::ObsidianSyncConfirm
    }

    pub fn note_list_selected(&self) -> usize {
        self.note_list_selected
    }

    pub fn note_list_indices(&self) -> &[usize] {
        &self.note_list_indices
    }

    pub fn note_list_delete_is_pending(&self) -> bool {
        self.note_list_indices
            .get(self.note_list_selected)
            .copied()
            .map(|index| self.note_list_pending_delete == Some(index))
            .unwrap_or(false)
    }

    pub fn obsidian_vaults(&self) -> &[ObsidianVault] {
        &self.obsidian_vaults
    }

    pub fn obsidian_vault_selected(&self) -> usize {
        self.obsidian_vault_selected
    }

    pub fn obsidian_vault_path(&self) -> Option<&Path> {
        self.obsidian_vault_path.as_deref()
    }

    pub fn is_obsidian_paired(&self) -> bool {
        self.obsidian_vault_path.is_some()
    }

    pub fn note_save_target_label(&self) -> &'static str {
        Self::note_save_target_name(self.note_save_target)
    }

    pub fn is_editing_title(&self) -> bool {
        self.editing_title
    }

    pub fn title_buffer(&self) -> &str {
        &self.title_buffer
    }

    pub fn title_cursor(&self) -> usize {
        Self::clamp_to_char_boundary(&self.title_buffer, self.title_cursor)
    }

    pub fn is_ghost_streaming(&self) -> bool {
        self.ghost_streaming
    }

    pub fn ghost_result(&self) -> Option<&str> {
        self.ghost_result.as_deref()
    }

    pub fn has_pending_ai_edit(&self) -> bool {
        self.pending_ai_edit.is_some()
    }

    pub fn pending_ai_diff_lines(&self) -> Vec<String> {
        let Some(proposal) = self.pending_ai_edit.as_ref() else {
            return Vec::new();
        };

        proposal.diff_lines.clone()
    }

    pub fn pending_ai_instruction(&self) -> Option<&str> {
        self.pending_ai_edit
            .as_ref()
            .map(|proposal| proposal.instruction.as_str())
    }

    pub fn pending_ai_proposal_label(&self) -> &'static str {
        match self
            .pending_ai_edit
            .as_ref()
            .and_then(|proposal| proposal.note_index)
        {
            Some(_) => "Proposed note edits",
            None => "Proposed new note",
        }
    }

    pub fn is_openrouter_login_pending(&self) -> bool {
        self.openrouter_login_rx.is_some()
    }

    pub fn is_strix_login_pending(&self) -> bool {
        self.strix_login_rx.is_some()
    }

    pub fn thinking_frame(&self) -> &'static str {
        THINKING_FRAMES[(self.tick as usize) % THINKING_FRAMES.len()]
    }

    pub fn command_label(command: &CommandSpec) -> String {
        format!("/{}", command.name)
    }

    pub fn selected_suggestion(&self) -> usize {
        self.selected_suggestion
    }

    pub fn visible_commands_window(
        &self,
        window_size: usize,
    ) -> (Vec<&'static CommandSpec>, usize) {
        // Use suggestion_filter if initialized, otherwise fall back to prompt
        let query = if let Some(ref filter) = self.suggestion_filter {
            filter.clone()
        } else {
            self.normalized_prompt().to_lowercase()
        };

        // Get filtered or full list
        let all: Vec<_> = if !query.is_empty() {
            COMMANDS
                .iter()
                .filter(|cmd| {
                    self.is_command_visible(cmd)
                        && (cmd.name.contains(&query)
                            || cmd.description.to_lowercase().contains(&query))
                })
                .collect()
        } else {
            COMMANDS
                .iter()
                .filter(|cmd| self.is_command_visible(cmd))
                .collect()
        };

        let total = all.len();

        if total == 0 {
            return (Vec::new(), 0);
        }

        let selected = self.selected_suggestion.min(total - 1);

        // Calculate the window start index to keep selection visible
        let mut start = 0;
        if selected >= window_size {
            start = selected.saturating_sub(window_size - 1);
        }

        // Ensure we don't go past the end
        let end = (start + window_size).min(total);
        let window: Vec<_> = all[start..end].iter().copied().collect();

        (window, start)
    }

    pub fn last_action(&self) -> &str {
        &self.last_action
    }

    pub fn visible_commands(&self, limit: usize) -> Vec<&'static CommandSpec> {
        let raw = self.prompt.trim();
        if !raw.starts_with('/') {
            return Vec::new();
        }

        let query = self.normalized_prompt().to_lowercase();

        // Show all commands when query is empty
        if query.is_empty() {
            let mut all: Vec<_> = COMMANDS
                .iter()
                .filter(|cmd| self.is_command_visible(cmd))
                .collect();
            all.truncate(limit);
            return all;
        }

        // Filter commands by query
        let mut matches: Vec<&'static CommandSpec> = COMMANDS
            .iter()
            .filter(|command| {
                self.is_command_visible(command)
                    && (command.name.contains(&query)
                        || command.description.to_lowercase().contains(&query))
            })
            .collect();

        matches.truncate(limit);
        matches
    }

    pub(super) fn is_command_visible(&self, cmd: &CommandSpec) -> bool {
        match cmd.name {
            "config" => false, // Hidden alias for /settings
            "login" => !self.connected,
            "logout" => self.connected,
            _ => true,
        }
    }

    pub fn total_command_matches(&self) -> usize {
        let raw = self.prompt.trim();
        if !raw.starts_with('/') {
            return 0;
        }

        let query = self.normalized_prompt().to_lowercase();

        COMMANDS
            .iter()
            .filter(|command| {
                query.is_empty()
                    || command.name.contains(&query)
                    || command.description.to_lowercase().contains(&query)
            })
            .count()
    }
}
