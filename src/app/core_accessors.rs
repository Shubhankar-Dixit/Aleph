use super::*;

#[allow(dead_code)]
impl App {
    pub fn new() -> Self {
        #[cfg(test)]
        let openrouter_api_key = None;
        #[cfg(not(test))]
        let openrouter_api_key = Self::load_openrouter_api_key();

        #[cfg(test)]
        let strix_access_token = None;
        #[cfg(not(test))]
        let strix_access_token = Self::load_strix_access_token();

        #[cfg(test)]
        let obsidian_vault_path = None;
        #[cfg(not(test))]
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

        #[cfg(test)]
        let ai_provider = default_ai_provider;
        #[cfg(not(test))]
        let ai_provider = Self::load_ai_provider().unwrap_or(default_ai_provider);

        let default_note_save_target = if strix_access_token.is_some() {
            NoteSaveTarget::Strix
        } else if obsidian_vault_path.is_some() {
            NoteSaveTarget::Obsidian
        } else {
            NoteSaveTarget::Local
        };

        #[cfg(test)]
        let note_save_target = default_note_save_target;
        #[cfg(not(test))]
        let note_save_target = Self::load_note_save_target()
            .filter(|target| {
                Self::note_save_target_is_available(
                    *target,
                    obsidian_vault_path.is_some(),
                    strix_access_token.is_some(),
                )
            })
            .unwrap_or(default_note_save_target);

        #[cfg(test)]
        let notes = Self::default_local_notes();
        #[cfg(not(test))]
        let notes = Self::load_local_notes().unwrap_or_else(|_| Self::default_local_notes());

        #[cfg(test)]
        let memories = Vec::new();
        #[cfg(not(test))]
        let memories = Self::load_local_memories().unwrap_or_default();

        #[cfg(test)]
        let editor_images_enabled = false;
        #[cfg(not(test))]
        let editor_images_enabled = Self::load_editor_images_enabled().unwrap_or(false);

        #[cfg(test)]
        let agent_mode_enabled = true;
        #[cfg(not(test))]
        let agent_mode_enabled = Self::load_agent_mode_enabled().unwrap_or(true);

        #[cfg(test)]
        let agent_context_scope = AgentContextScope::CurrentFolder;
        #[cfg(not(test))]
        let agent_context_scope =
            Self::load_agent_context_scope().unwrap_or(AgentContextScope::CurrentFolder);

        #[cfg(test)]
        let (temporal_forks, current_fork_id) = (Vec::new(), None);
        #[cfg(not(test))]
        let (temporal_forks, current_fork_id) =
            Self::load_temporal_fork_state().unwrap_or_else(|_| (Vec::new(), None));

        #[cfg(test)]
        let (rooms, active_room_index) = Self::default_room_state();
        #[cfg(not(test))]
        let (rooms, active_room_index) =
            Self::load_room_state().unwrap_or_else(|_| Self::default_room_state());

        let (note_sync_tx, note_sync_rx) = mpsc::channel();
        let (agent_worker_tx, agent_worker_rx) = mpsc::channel();

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
            rooms,
            active_room_index,
            notes,
            memories,
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
            editor_save_status: EditorSaveStatus::Clean,
            ai_input_buffer: String::new(),
            ai_input_cursor: 0,
            suggestion_filter: None,
            editor_scroll_offset: 0,
            editor_word_wrap: true,
            editor_images_enabled,
            editor_cursor_style: CursorStyle::Line,
            editor_selection: Selection::default(),
            editor_drag_anchor: None,
            undo_stack: VecDeque::with_capacity(100),
            redo_stack: VecDeque::with_capacity(100),
            search_state: SearchState {
                query: String::new(),
                matches: Vec::new(),
                current_match: None,
                active: false,
            },
            chat_messages: Vec::new(),
            next_chat_message_id: 1,
            agent_runs: Vec::new(),
            active_run_id: None,
            next_run_id: 1,
            activity_log: VecDeque::with_capacity(80),
            chat_composer: ChatComposerState::default(),
            transcript_viewport: TranscriptViewportState::default(),
            openrouter_api_key,
            strix_access_token,
            chat_stream_rx: None,
            openrouter_login_rx: None,
            openrouter_login_cancel: None,
            strix_login_rx: None,
            strix_login_cancel: None,
            note_sync_tx,
            note_sync_rx,
            note_sync_in_flight: HashSet::new(),
            note_sync_queued: HashMap::new(),
            obsidian_vault_path,
            obsidian_vaults,
            obsidian_vault_selected: 0,
            note_save_target,
            ai_provider,
            strix_logs: Vec::new(),
            streaming_buffer: String::new(),
            streaming_active: false,
            thinking_status: String::new(),
            chat_render_cache: Vec::new(),
            transcript_message_cache: RefCell::new(HashMap::new()),
            chat_render_dirty: false,
            chat_cache_stable_len: 0,
            agent_mode_enabled,
            agent_context_scope,
            login_picker_selected: 0,
            settings_selected: 0,
            pending_agent_query: None,
            pending_agent_decision: None,
            agent_plan_rx: None,
            agent_plan_query: None,
            pending_agent_execution: None,
            agent_execution_generation: 0,
            agent_worker_tx,
            agent_worker_rx,
            chat_turn_started_at: None,
            ghost_stream_rx: None,
            ghost_streaming: false,
            ghost_result: None,
            pending_ai_edit: None,
            ai_draft_create_title: None,
            note_list_selected: 0,
            note_list_indices: Vec::new(),
            note_list_pending_delete: None,
            room_list_selected: 0,
            room_list_pending_delete: None,
            editing_title: false,
            title_buffer: String::new(),
            title_cursor: 0,
            expanded_folders: Vec::new(),
            path_list_selected: 0,
            path_list_pending_delete: None,
            temporal_forks,
            current_fork_id,
        };

        app.rebuild_obsidian_folders_from_cached_notes();

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

            let pending_syncs = app
                .notes
                .iter()
                .enumerate()
                .filter_map(|(index, note)| note.strix_sync_pending.then_some(index))
                .collect::<Vec<_>>();
            for index in pending_syncs {
                let _ = app.queue_strix_note_sync(index);
            }
        }

        app.add_activity("Ready for input.");
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
            strix_sync_pending: false,
        }]
    }

    pub fn run_cli_command(&mut self, args: &[String]) -> Result<Vec<String>, String> {
        if args.is_empty() {
            return Ok(vec![
                String::from("Usage: aleph <notes|room|obsidian|path|trail|daemon|sync> ..."),
                String::from("Examples:"),
                String::from("  aleph notes search roadmap"),
                String::from("  aleph notes read <id>"),
                String::from("  aleph room list"),
                String::from("  aleph room use <name>"),
                String::from("  aleph room all"),
                String::from("  aleph path list"),
                String::from("  aleph path save <name>"),
                String::from("  aleph trail"),
                String::from("  aleph daemon status"),
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

        if area == "room" || area == "rooms" {
            return self.run_room_cli_command(&args[1..]);
        }

        if area == "path" || area == "world" || area == "fork" {
            return self.run_path_cli_command(&args[1..]);
        }

        if area == "trail" {
            return self.run_trail_cli_command(&args[1..]);
        }

        if area == "daemon" {
            return self.run_daemon_cli_command(&args[1..]);
        }

        if area != "notes" && area != "note" {
            let _ = self.append_trail_event(
                "command_failed",
                format!("Unknown Aleph CLI area: {}.", area),
                vec![area.to_string()],
                TrailImportance::Normal,
            );
            return Err(format!(
                "Unknown Aleph CLI area '{}'. Try 'notes', 'room', 'obsidian', 'path', 'trail', or 'daemon'.",
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
                let _ = self.append_trail_event(
                    "note",
                    format!("Read note from CLI: {}.", note.title),
                    vec![note.id.to_string()],
                    TrailImportance::Low,
                );
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
                let _ = self.append_trail_event(
                    "note",
                    format!("Updated note from CLI: {}.", updated.title),
                    vec![updated.id.to_string()],
                    TrailImportance::High,
                );
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
                let _ = self.append_trail_event(
                    "note",
                    format!("Appended to note from CLI: {}.", updated.title),
                    vec![updated.id.to_string()],
                    TrailImportance::High,
                );
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
                let _ = self.append_trail_event(
                    "note",
                    format!("Created note from CLI: {}.", note.title),
                    vec![note.id.to_string()],
                    TrailImportance::High,
                );
                Ok(vec![format!(
                    "Created {} ({})",
                    note.title,
                    Self::note_source_label(&note)
                )])
            }
            _ => Err(format!("Unknown notes action '{}'.", action)),
        }
    }

    pub(super) fn run_room_cli_command(&mut self, args: &[String]) -> Result<Vec<String>, String> {
        let action = args.first().map(|value| value.as_str()).unwrap_or("list");
        match action {
            "list" => Ok(self.room_list_lines()),
            "show" => {
                let target = args.get(1..).unwrap_or(&[]).join(" ");
                let result = if target.trim().is_empty() {
                    self.room_detail_lines(self.active_room_index)
                } else {
                    self.resolve_room_index(&target)
                        .ok_or_else(|| format!("No room matched '{}'.", target))
                        .and_then(|index| self.room_detail_lines(index))
                };

                result.map(|(title, lines)| {
                    std::iter::once(format!("Room: {}", title))
                        .chain(lines)
                        .collect()
                })
            }
            "use" => {
                let target = args.get(1..).unwrap_or(&[]).join(" ");
                if target.trim().is_empty() {
                    return Err(String::from("Usage: aleph room use <name>"));
                }

                self.switch_room_by_name(&target).map(|lines| {
                    std::iter::once(format!("Room: {}", self.active_room_label()))
                        .chain(lines)
                        .collect()
                })
            }
            "next" => self.cycle_room(1).map(|lines| {
                std::iter::once(format!("Room: {}", self.active_room_label()))
                    .chain(lines)
                    .collect()
            }),
            "prev" | "previous" => self.cycle_room(-1).map(|lines| {
                std::iter::once(format!("Room: {}", self.active_room_label()))
                    .chain(lines)
                    .collect()
            }),
            _ => {
                let target = args.join(" ");
                self.switch_room_by_name(&target).map(|lines| {
                    std::iter::once(format!("Room: {}", self.active_room_label()))
                        .chain(lines)
                        .collect()
                })
            }
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
                            self.add_system_log("OpenRouter login completed; API key stored");
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
                    self.add_system_log(format!("OpenRouter login failed: {}", error));
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
                    let first_chunk = self.streaming_buffer.is_empty();
                    self.streaming_active = true;
                    self.streaming_buffer.push_str(&chunk);
                    let thought_seconds = self
                        .chat_turn_started_at
                        .map(|started| started.elapsed().as_secs_f32());
                    if let Some(message) = self
                        .chat_messages
                        .iter_mut()
                        .rev()
                        .find(|message| message.role == "assistant")
                    {
                        message.content.push_str(&chunk);
                        if first_chunk && message.thought_seconds.is_none() {
                            message.thought_seconds = thought_seconds;
                        }
                    }
                    self.chat_render_dirty = true;
                    self.thinking = true;
                    self.thinking_status = String::from("Streaming response...");
                    if first_chunk {
                        self.add_activity("Receiving model response.");
                    }
                }
                Ok(ChatStreamUpdate::Notice(notice)) => {
                    self.add_system_log(notice.clone());
                    self.add_activity(notice);
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

                    let turn_seconds = self
                        .chat_turn_started_at
                        .take()
                        .map(|started| started.elapsed().as_secs_f32());
                    if let Some(message) = self
                        .chat_messages
                        .iter_mut()
                        .rev()
                        .find(|message| message.role == "assistant")
                    {
                        message.turn_seconds = turn_seconds;
                    }
                    self.streaming_buffer.clear();
                    self.streaming_active = false;
                    self.rebuild_chat_render_cache();
                    self.chat_render_dirty = false;
                    self.thinking = false;
                    self.thinking_status.clear();
                    self.thinking_ticks_remaining = 0;
                    self.chat_stream_rx = None;
                    self.last_action = String::from("AI response received.");
                    match turn_seconds {
                        Some(seconds) => {
                            self.add_activity(format!("Turn completed in {:.1}s.", seconds))
                        }
                        None => self.add_activity("Finished response."),
                    }
                    let _ =
                        self.complete_run("Aleph completed the response. No changes were made.");
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

                    self.chat_turn_started_at = None;
                    self.streaming_buffer.clear();
                    self.streaming_active = false;
                    self.rebuild_chat_render_cache();
                    self.chat_render_dirty = false;
                    self.thinking = false;
                    self.thinking_status.clear();
                    self.thinking_ticks_remaining = 0;
                    self.chat_stream_rx = None;
                    self.last_action = String::from("AI request failed.");
                    self.add_system_log(format!(
                        "{} request failed: {}",
                        self.ai_provider_label(),
                        Self::preview_text(&error, 120)
                    ));
                    self.add_activity(format!(
                        "Request failed: {}",
                        Self::preview_text(&error, 72)
                    ));
                    let _ = self.fail_run(error);
                    stream_finished = true;
                }
                Err(TryRecvError::Empty) => {
                    self.thinking = true;
                    if self.streaming_buffer.is_empty() {
                        if self.thinking_status != "Waiting for response..." {
                            self.add_activity("Waiting for model response.");
                        }
                        self.thinking_status = String::from("Waiting for response...");
                    } else {
                        self.thinking_status = String::from("Streaming response...");
                    }
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

                    self.chat_turn_started_at = None;
                    self.streaming_buffer.clear();
                    self.streaming_active = false;
                    self.rebuild_chat_render_cache();
                    self.chat_render_dirty = false;
                    self.thinking = false;
                    self.thinking_status.clear();
                    self.thinking_ticks_remaining = 0;
                    self.chat_stream_rx = None;
                    self.last_action = String::from("AI request disconnected.");
                    self.add_activity("Request disconnected.");
                    let _ =
                        self.fail_run("The provider disconnected before completing the response.");
                    stream_finished = true;
                }
            }
        }

        if self.chat_render_dirty {
            self.rebuild_chat_render_cache_streaming();
            self.chat_render_dirty = false;
        }

        self.process_ghost_stream();
        self.process_agent_plan();
        self.process_note_sync_updates();

        if self.ai_overlay_visible && self.ai_overlay_pulse_ticks > 0 {
            self.ai_overlay_pulse_ticks -= 1;
        }
        if self.save_shimmer_ticks > 0 {
            self.save_shimmer_ticks -= 1;
        }
    }

    pub fn on_iteration(&mut self) {
        self.process_agent_execution();
    }

    pub fn request_quit(&mut self) {
        self.cancel_foreground_run("Aleph exited before the run completed.");
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

    pub fn editor_save_status(&self) -> &EditorSaveStatus {
        &self.editor_save_status
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

    pub fn agent_runs(&self) -> &[AgentRun] {
        &self.agent_runs
    }

    pub fn active_agent_run(&self) -> Option<&AgentRun> {
        let id = self.active_run_id?;
        self.agent_runs.iter().find(|run| run.id == id)
    }

    pub fn agent_run(&self, id: u64) -> Option<&AgentRun> {
        self.agent_runs.iter().find(|run| run.id == id)
    }

    pub fn has_pending_agent_approval(&self) -> bool {
        self.active_agent_run()
            .is_some_and(|run| run.phase == RunPhase::WaitingApproval && run.approval.is_some())
    }

    pub fn agent_approval_policy(&self) -> AgentApprovalPolicy {
        AgentApprovalPolicy::ExplicitWrites
    }

    pub fn chat_composer(&self) -> &ChatComposerState {
        &self.chat_composer
    }

    pub fn transcript_viewport(&self) -> TranscriptViewportState {
        self.transcript_viewport
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

    pub fn active_room_label(&self) -> &str {
        self.active_room_ref().name.as_str()
    }

    pub fn room_scope_summary(&self) -> String {
        let room = self.active_room_ref();
        if Self::is_global_room(room) {
            return String::from("all notes · no room filter");
        }

        format!(
            "{} paths · {} tags · {} filters",
            room.project_paths.len(),
            room.tags.len(),
            room.filters.len()
        )
    }

    pub fn is_global_scope(&self) -> bool {
        Self::is_global_room(self.active_room_ref())
    }

    pub fn room_accent(&self) -> Color {
        let room = self.active_room_ref();
        Color::Rgb(room.accent[0], room.accent[1], room.accent[2])
    }

    pub fn room_accent_soft(&self) -> Color {
        match self.room_accent() {
            Color::Rgb(red, green, blue) => Color::Rgb(
                ((red as u16 * 3 + 96) / 4) as u8,
                ((green as u16 * 3 + 96) / 4) as u8,
                ((blue as u16 * 3 + 96) / 4) as u8,
            ),
            color => color,
        }
    }

    pub fn room_note_count(&self) -> usize {
        self.room_note_indices().len()
    }

    pub fn temporal_forks(&self) -> &[crate::app::model::TemporalFork] {
        &self.temporal_forks
    }

    pub fn current_fork_id(&self) -> Option<&str> {
        self.current_fork_id.as_deref()
    }

    pub fn room_recent_session_count(&self) -> usize {
        self.temporal_forks
            .iter()
            .filter(|fork| self.room_matches_session(fork))
            .count()
    }

    pub fn ai_provider(&self) -> AiProvider {
        self.ai_provider
    }

    pub fn ai_provider_label(&self) -> &'static str {
        self.model_provider_label()
    }

    pub fn current_repo_context(&self) -> Option<&RepoContext> {
        if let Some(current_fork_id) = self.current_fork_id.as_deref() {
            if let Some(fork) = self
                .temporal_forks
                .iter()
                .find(|fork| fork.id.as_str() == current_fork_id)
            {
                if fork.repo_context.is_some() {
                    return fork.repo_context.as_ref();
                }
            }
        }

        self.temporal_forks
            .iter()
            .rev()
            .find_map(|fork| fork.repo_context.as_ref())
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

    pub fn agent_context_scope(&self) -> AgentContextScope {
        self.agent_context_scope
    }

    pub fn agent_context_scope_label(&self) -> &'static str {
        Self::agent_context_scope_name(self.agent_context_scope)
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

    pub fn is_room_list(&self) -> bool {
        self.panel_mode == PanelMode::RoomList
    }

    pub fn is_path_list(&self) -> bool {
        self.panel_mode == PanelMode::PathList
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

    pub fn room_list_selected(&self) -> usize {
        self.room_list_selected
    }

    pub fn room_list_delete_is_pending(&self) -> bool {
        self.room_list_pending_delete
            .map(|index| index == self.room_list_selected)
            .unwrap_or(false)
    }

    pub fn path_list_selected(&self) -> usize {
        self.path_list_selected
    }

    pub fn path_list_delete_is_pending(&self) -> bool {
        self.path_list_pending_delete == Some(self.path_list_selected)
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

    pub fn thinking_status(&self) -> &str {
        &self.thinking_status
    }

    pub fn thinking_frame(&self) -> &'static str {
        let frame_index = ((self.tick / 4) as usize) % THINKING_FRAMES.len();
        THINKING_FRAMES[frame_index]
    }

    pub fn command_label(command: &CommandSpec) -> String {
        format!("/{}", command.name)
    }

    pub(super) fn command_match_rank(command: &CommandSpec, query: &str) -> Option<usize> {
        if query.is_empty() {
            return Some(0);
        }

        if command.name == query {
            return Some(0);
        }

        if command.name.starts_with(query) {
            return Some(1);
        }

        if query
            .strip_prefix(command.name)
            .is_some_and(|rest| rest.starts_with(char::is_whitespace))
        {
            return Some(2);
        }

        if command.name.contains(query) {
            return Some(3);
        }

        command
            .description
            .to_lowercase()
            .contains(query)
            .then_some(4)
    }

    pub fn selected_suggestion(&self) -> usize {
        self.selected_suggestion
    }

    pub fn visible_commands_window(
        &self,
        window_size: usize,
    ) -> (Vec<&'static CommandSpec>, usize) {
        let query = self.active_command_query();

        let all = self.matching_commands(&query);

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

    pub fn recent_activity(&self, limit: usize) -> Vec<ActivityEntry> {
        self.activity_log
            .iter()
            .rev()
            .take(limit)
            .cloned()
            .collect()
    }

    pub fn activity_headline(&self) -> String {
        self.activity_log
            .back()
            .map(|entry| entry.label.clone())
            .unwrap_or_else(|| self.last_action.clone())
    }

    pub fn visible_commands(&self, limit: usize) -> Vec<&'static CommandSpec> {
        let raw = self.prompt.trim();
        if !raw.starts_with('/') {
            return Vec::new();
        }

        let query = self.active_command_query();

        let mut commands = self.matching_commands(&query);
        commands.truncate(limit);
        commands
    }

    pub(super) fn matching_commands(&self, query: &str) -> Vec<&'static CommandSpec> {
        if query.is_empty() {
            return COMMANDS
                .iter()
                .filter(|cmd| self.is_command_visible(cmd) && Self::is_command_family_entry(cmd))
                .collect();
        }

        // Any exact command is a single choice. In particular, `/note list`
        // must not also rank its `/note` parent as a second suggestion.
        if let Some(exact) = COMMANDS
            .iter()
            .find(|command| command.name == query && self.is_command_visible(command))
        {
            return vec![exact];
        }

        if let Some(base) = query.strip_suffix(' ') {
            if !base.is_empty() && Self::command_has_subcommands(base) {
                let prefix = format!("{} ", base);
                return COMMANDS
                    .iter()
                    .filter(|cmd| self.is_command_visible(cmd) && cmd.name.starts_with(&prefix))
                    .collect();
            }
        }

        // Once input is inside a command family, keep every result inside that
        // family. This applies both to direct typing (`/note li`) and to arrow
        // navigation after Enter expanded the family.
        if let Some((family, _)) = query.split_once(' ') {
            if Self::command_has_subcommands(family) {
                let prefix = format!("{} ", family);
                let mut matches = COMMANDS
                    .iter()
                    .filter(|command| {
                        self.is_command_visible(command) && command.name.starts_with(&prefix)
                    })
                    .filter_map(|command| {
                        Self::command_match_rank(command, query).map(|rank| (rank, command))
                    })
                    .collect::<Vec<_>>();
                matches
                    .sort_by_key(|(rank, command)| (*rank, std::cmp::Reverse(command.name.len())));
                if matches.is_empty() && family == "room" {
                    // Room names are dynamic arguments (`/room project-name`),
                    // not registered leaf commands. Preserve the family row as
                    // the execution affordance when no static room action fits.
                } else {
                    return matches.into_iter().map(|(_, command)| command).collect();
                }
            }
        }

        let mut matches: Vec<(usize, &'static CommandSpec)> = COMMANDS
            .iter()
            .filter_map(|command| {
                self.is_command_visible(command)
                    .then(|| Self::command_match_rank(command, &query).map(|rank| (rank, command)))
                    .flatten()
            })
            .collect();

        matches.sort_by_key(|(rank, command)| (*rank, std::cmp::Reverse(command.name.len())));
        matches.into_iter().map(|(_, command)| command).collect()
    }

    fn is_command_family_entry(command: &CommandSpec) -> bool {
        !command.name.contains(char::is_whitespace)
    }

    pub(super) fn command_has_subcommands(command: &str) -> bool {
        let prefix = format!("{} ", command);
        COMMANDS
            .iter()
            .any(|candidate| candidate.name.starts_with(&prefix))
    }

    pub(super) fn command_family_expands(command: &str) -> bool {
        Self::command_has_subcommands(command) && !matches!(command, "trail" | "daemon")
    }

    pub fn command_subcommand_summary(command: &CommandSpec) -> Option<String> {
        if !Self::command_family_expands(command.name) {
            return None;
        }
        let prefix = format!("{} ", command.name);
        let subcommands = COMMANDS
            .iter()
            .filter_map(|candidate| candidate.name.strip_prefix(&prefix))
            .filter_map(|rest| rest.split_whitespace().next())
            .fold(Vec::<&str>::new(), |mut names, name| {
                if !names.contains(&name) {
                    names.push(name);
                }
                names
            });

        if subcommands.is_empty() {
            None
        } else {
            let noun = if subcommands.len() == 1 {
                "action"
            } else {
                "actions"
            };
            Some(format!("Enter to open {} {}", subcommands.len(), noun))
        }
    }

    pub fn command_browse_family(&self) -> Option<String> {
        let query = self.active_command_query();
        let family = query.split_whitespace().next()?;
        (query.contains(char::is_whitespace) && Self::command_has_subcommands(family))
            .then(|| family.to_string())
    }

    pub fn command_palette_label(&self, command: &CommandSpec) -> String {
        if let Some(family) = self.command_browse_family() {
            if let Some(action) = command.name.strip_prefix(&format!("{} ", family)) {
                return action.to_string();
            }
        }

        Self::command_label(command)
    }

    pub(super) fn command_query(&self) -> String {
        let raw = self.prompt.trim_start();
        if !raw.starts_with('/') {
            return String::new();
        }

        let without_slash = raw.trim_start_matches('/');
        let normalized = Self::normalize_command_input(without_slash.trim());
        if normalized.is_empty() {
            return normalized;
        }

        if without_slash.ends_with(char::is_whitespace) {
            format!("{} ", normalized.to_lowercase())
        } else {
            normalized.to_lowercase()
        }
    }

    pub(super) fn active_command_query(&self) -> String {
        self.suggestion_filter
            .clone()
            .unwrap_or_else(|| self.command_query())
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

        let query = self.active_command_query();

        self.matching_commands(&query).len()
    }
}
