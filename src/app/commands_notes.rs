use super::*;

pub(super) enum TreeItem {
    Folder {
        id: usize,
        name: String,
        depth: usize,
        expanded: bool,
        note_count: usize,
    },
    Note {
        index: usize,
        depth: usize,
    },
}

#[allow(dead_code)]
impl App {
    pub(super) fn parse_command<'a>(prompt: &'a str) -> Option<(&'static CommandSpec, &'a str)> {
        COMMANDS
            .iter()
            .filter_map(|command| {
                if prompt == command.name {
                    return Some((command, ""));
                }

                let Some(rest) = prompt.strip_prefix(command.name) else {
                    return None;
                };

                if rest.is_empty() || rest.starts_with(char::is_whitespace) {
                    Some((command, rest.trim_start()))
                } else {
                    None
                }
            })
            .max_by_key(|(command, _)| command.name.len())
    }

    pub(super) fn execute_command(&mut self, command: &str, args: &str) {
        match command {
            "clear-notes" => {
                self.clear_notes_state();
            }
            "login" => {
                let args_trimmed = args.trim();

                // No args: show login picker
                if args_trimmed.is_empty() {
                    // Check if already logged in
                    if self.is_openrouter_connected() || self.is_strix_connected() {
                        let provider = if self.is_openrouter_connected() {
                            "OpenRouter"
                        } else {
                            "Strix"
                        };
                        self.set_result_panel(
                            "Already connected",
                            vec![
                                format!("You are already connected through {}.", provider),
                                String::from("Use /logout first if you want to switch credentials or re-authenticate."),
                                String::from("Use /status to see your current connection details."),
                            ],
                        );
                        self.last_action = format!("Already connected to {}.", provider);
                        return;
                    }
                    self.open_login_picker();
                    return;
                }

                // Parse provider and token from args
                // Format: /login [openrouter|strix] [token]
                let (provider, token) = {
                    let parts: Vec<&str> = args_trimmed.splitn(2, ' ').collect();
                    if parts.len() == 2 {
                        let maybe_provider = parts[0].to_lowercase();
                        if maybe_provider == "openrouter" || maybe_provider == "strix" {
                            (maybe_provider, parts[1])
                        } else {
                            // Assume first word is the token for OpenRouter (backward compat)
                            (String::from("openrouter"), args_trimmed)
                        }
                    } else {
                        let maybe_provider = args_trimmed.to_lowercase();
                        if maybe_provider == "openrouter" {
                            // /login openrouter -> browser authorization for an API key
                            if self.start_openrouter_browser_login() {
                                return;
                            }
                            self.set_result_panel(
                                "OpenRouter provider failed",
                                vec![String::from(
                                    "Unable to start the browser-based OpenRouter authorization flow.",
                                )],
                            );
                            self.last_action = String::from("OpenRouter provider setup failed.");
                            return;
                        } else if maybe_provider == "strix" {
                            if self.start_strix_browser_login() {
                                return;
                            }
                            self.set_result_panel(
                                "Strix login failed",
                                vec![String::from(
                                    "Unable to start the browser-based Strix login flow.",
                                )],
                            );
                            self.last_action = String::from("Strix login failed.");
                            return;
                        } else {
                            // Assume it's an OpenRouter API key directly
                            (String::from("openrouter"), args_trimmed)
                        }
                    }
                };

                match provider.as_str() {
                    "strix" => {
                        self.set_ai_provider(AiProvider::Strix);
                        if token.trim().is_empty() {
                            if self.start_strix_browser_login() {
                                return;
                            }
                            self.set_result_panel(
                                "Strix login failed",
                                vec![String::from(
                                    "Unable to start the browser-based Strix login flow.",
                                )],
                            );
                            self.last_action = String::from("Strix login failed.");
                            return;
                        }

                        self.add_strix_log("Saving Strix access token");
                        match self.store_strix_access_token(token) {
                            Ok(()) => {
                                self.strix_access_token = Some(token.to_string());
                                self.note_save_target = NoteSaveTarget::Strix;
                                let _ = self.store_note_save_target();
                                self.refresh_connection_state();
                                self.add_strix_log("Connected to Strix successfully");
                                self.set_result_panel(
                                    "Strix login",
                                    vec![
                                        String::from("Strix authentication configured."),
                                        String::from(
                                            "The native access token has been stored locally.",
                                        ),
                                    ],
                                );
                                self.last_action = String::from("Connected to Strix.");
                            }
                            Err(error) => {
                                self.strix_access_token = None;
                                self.refresh_connection_state();
                                self.set_result_panel("Strix login failed", vec![error]);
                                self.last_action = String::from("Strix login failed.");
                            }
                        }
                    }
                    _ => {
                        self.set_ai_provider(AiProvider::OpenRouter);
                        match self.store_openrouter_api_key(token) {
                            Ok(()) => {
                                self.openrouter_api_key = Some(token.to_string());
                                self.refresh_connection_state();
                                self.rebuild_chat_render_cache();
                                self.set_result_panel(
                                    "OpenRouter provider",
                                    vec![
                                        String::from("OpenRouter API key saved locally."),
                                        String::from(
                                            "AI chat can use OpenRouter as a model provider now.",
                                        ),
                                    ],
                                );
                                self.last_action =
                                    String::from("Configured OpenRouter as a model provider.");
                            }
                            Err(error) => {
                                self.openrouter_api_key = None;
                                self.refresh_connection_state();
                                self.set_result_panel("OpenRouter provider failed", vec![error]);
                                self.last_action =
                                    String::from("OpenRouter provider setup failed.");
                            }
                        }
                    }
                }
            }
            "logout" => {
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
                self.set_result_panel(
                    "Signed out",
                    vec![
                        String::from("Saved model-provider credentials have been cleared."),
                        String::from("Saved Strix account credentials have also been cleared."),
                        String::from("Use /login openrouter or /login strix to connect again."),
                    ],
                );
                self.last_action = String::from("Disconnected from providers.");
            }
            "obsidian pair" => {
                self.handle_obsidian_pair(args.trim());
            }
            "obsidian vaults" => {
                self.refresh_obsidian_vaults();
                let mut lines = self.format_obsidian_vault_lines();
                if lines.is_empty() {
                    lines = vec![
                        String::from("No Obsidian vaults were found in desktop config."),
                        String::from("Pair explicitly with /obsidian pair /path/to/vault."),
                    ];
                }
                self.set_result_panel("Obsidian vaults", lines);
                self.last_action = String::from("Listed Obsidian vaults.");
            }
            "obsidian sync" => match self.sync_obsidian_notes() {
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
            },
            "obsidian status" => {
                self.refresh_obsidian_vaults();
                let paired = self
                    .obsidian_vault_path
                    .as_ref()
                    .map(|path| path.display().to_string())
                    .unwrap_or_else(|| String::from("Not paired"));
                self.set_result_panel(
                    "Obsidian status",
                    vec![
                        format!("Paired vault: {}", paired),
                        format!("Detected vaults: {}", self.obsidian_vaults.len()),
                        format!("Config: {}", Self::obsidian_config_path().display()),
                        format!(
                            "Pairing fallback: {}",
                            Self::obsidian_pairing_path().display()
                        ),
                        String::from("Sync mode: direct Markdown filesystem integration"),
                        String::from("Open mode: obsidian:// URI, no Obsidian CLI required"),
                    ],
                );
                self.last_action = String::from("Refreshed Obsidian status.");
            }
            "obsidian open" => match self.open_obsidian_target(args.trim()) {
                Ok(message) => {
                    self.set_result_panel("Obsidian open", vec![message]);
                    self.last_action = String::from("Opened Obsidian target.");
                }
                Err(error) => {
                    self.set_result_panel("Obsidian open failed", vec![error]);
                    self.last_action = String::from("Obsidian open failed.");
                }
            },
            "status" => {
                self.set_result_panel(
                    "Status",
                    vec![
                        format!(
                            "OpenRouter: {}",
                            if self.is_openrouter_connected() {
                                "connected"
                            } else {
                                "offline"
                            }
                        ),
                        format!(
                            "Strix: {}",
                            if self.is_strix_connected() {
                                "connected"
                            } else {
                                "offline"
                            }
                        ),
                        format!("Obsidian: {}", self.obsidian_status_label()),
                        format!("Note save target: {}", self.note_save_target_label()),
                        format!("Notes: {}", self.notes.len()),
                        format!("Cache: {}", Self::strix_cache_path().display()),
                        format!("Memories: {}", self.memories.len()),
                        format!("Canvases: {}", self.canvases.len()),
                        format!("Uptime: {}", self.uptime()),
                    ],
                );
                self.last_action = String::from("Refreshed provider status.");
            }
            "sync" => match self.sync_strix_notes() {
                Ok(count) => {
                    self.set_result_panel(
                            "Strix sync",
                            vec![
                                format!("Pulled {} notes from Strix.", count),
                                String::from("Use /search, /note list, /note read, and /note edit against the synced notes."),
                            ],
                        );
                    self.last_action = format!("Synced {} Strix notes.", count);
                }
                Err(error) => {
                    self.set_result_panel("Strix sync failed", vec![error]);
                    self.last_action = String::from("Strix sync failed.");
                }
            },
            "doctor" => {
                self.set_result_panel(
                    "Doctor",
                    vec![
                        String::from("Raw input: OK"),
                        String::from("Command palette: OK"),
                        String::from("Note editor: OK"),
                        String::from("Local storage: OK"),
                    ],
                );
                self.last_action = String::from("Ran diagnostics.");
            }
            "config" | "settings" => {
                self.open_settings_panel();
            }
            "mode agent" => {
                self.agent_mode_enabled = true;
                let _ = self.store_agent_mode_enabled();
                self.set_result_panel(
                    "Agent mode",
                    vec![String::from(
                        "Agent mode is enabled. Aleph will route note-writing requests to note tools.",
                    )],
                );
                self.last_action = String::from("Agent mode enabled.");
            }
            "mode chat" => {
                self.agent_mode_enabled = false;
                let _ = self.store_agent_mode_enabled();
                self.set_result_panel(
                    "Chat mode",
                    vec![String::from(
                        "Chat mode is enabled. Aleph will answer normally without taking note actions.",
                    )],
                );
                self.last_action = String::from("Chat mode enabled.");
            }
            "search" => {
                let query = args.trim();
                if self.is_strix_connected() {
                    self.ensure_cached_strix_notes_loaded();
                    if self.notes.is_empty() {
                        if let Err(error) = self.sync_strix_notes() {
                            self.set_result_panel("Strix search failed", vec![error]);
                            self.last_action = String::from("Strix search failed.");
                            return;
                        }
                    }
                    let mut lines = self.search_notes(query);
                    if lines.is_empty() {
                        lines.push(format!(
                            "No cached Strix matches for '{}'. Run /sync to refresh.",
                            query
                        ));
                    }
                    self.set_result_panel(format!("Search: {}", query), lines);
                    self.last_action = format!("Searched cached Strix notes for {}.", query);
                    return;
                }
                let mut lines = self.search_notes(query);
                if lines.is_empty() {
                    lines.push(format!("No local matches for '{}'.", query));
                }
                self.set_result_panel(format!("Search: {}", query), lines);
                self.last_action = format!("Searched for {}.", query);
            }
            "recall" => {
                let mut lines = self
                    .history
                    .iter()
                    .rev()
                    .take(5)
                    .cloned()
                    .collect::<Vec<_>>();

                if lines.is_empty() {
                    lines.push(String::from("No history yet."));
                }

                self.set_result_panel("Recent activity", lines);
                self.last_action = String::from("Showed recent activity.");
            }
            "ask" => {
                let query = args.trim();
                let lines = if query.is_empty() {
                    vec![String::from("Ask the selected AI provider a question after the command, for example: /ask what should ship next?")]
                } else {
                    if self.start_chat_turn(query.to_string()) {
                        self.reset_prompt();
                        return;
                    }
                    return;
                };
                self.set_result_panel("Ask", lines);
                self.last_action = String::from("Prepared an ask response.");
            }
            "agent edit" => {
                let instruction = args.trim();
                if instruction.is_empty() {
                    self.set_result_panel(
                        "AI note edit",
                        vec![String::from(
                            "Describe the note change after /agent edit, for example: /agent edit make the selected note more concise",
                        )],
                    );
                    self.last_action = String::from("AI note edit needs an instruction.");
                    return;
                }
                if let Some(index) = self.current_note_index() {
                    self.open_note_editor(index);
                    self.open_ai_overlay();
                    self.ai_input_buffer = instruction.to_string();
                    self.ai_input_cursor = self.ai_input_buffer.len();
                    self.ghost_submit_instruction();
                    self.last_action = format!(
                        "AI is preparing edits for note: {}",
                        self.notes[index].title
                    );
                } else {
                    self.set_result_panel(
                        "AI note edit",
                        vec![String::from("No note is selected right now.")],
                    );
                    self.last_action = String::from("AI note edit needs a note target.");
                }
            }
            "note list" => {
                self.open_note_list_panel();
                self.last_action = String::from("Listed notes. Use arrow keys to navigate.");
            }
            "note read" => {
                if self.is_strix_connected() {
                    self.ensure_cached_strix_notes_loaded();
                }
                if self.is_strix_connected()
                    && !args.trim().is_empty()
                    && self.resolve_note_index(args.trim()).is_none()
                {
                    if let Ok(note) = self.load_strix_note(args.trim(), true) {
                        self.upsert_synced_note(note);
                    }
                }
                let Some(index) = self.resolve_note_index(args.trim()) else {
                    self.set_result_panel(
                        "Note not found",
                        vec![String::from(
                            "Try /note read 1 or /note read Strix gateway.",
                        )],
                    );
                    self.last_action = String::from("Note not found.");
                    return;
                };

                self.selected_note = index;
                let note = &self.notes[index];
                let note_title = note.title.clone();
                let note_id = note.id;
                let note_updated = note.updated_at.clone();
                let note_content = note.content.clone();
                let source_info = Self::note_source_label(note);
                let folder_info = if let Some(fid) = note.folder_id {
                    format!("Folder: {}", self.get_folder_path(fid))
                } else {
                    String::from("Folder: Uncategorized")
                };

                let mut lines = vec![
                    format!("ID: {}", note_id),
                    source_info,
                    format!("Updated: {}", note_updated),
                    folder_info,
                    String::new(),
                ];
                lines.extend(note_content.lines().map(|line| line.to_string()));
                self.set_result_panel(format!("Note: {}", note_title), lines);
                self.last_action = format!("Opened note: {}", note_title);
            }
            "note create" => {
                let (title_arg, initial_content) = Self::split_note_body_args(args.trim());
                let title = if title_arg.trim().is_empty() {
                    String::from("Untitled note")
                } else {
                    title_arg.trim().to_string()
                };
                match self.create_note_from_content(&title, initial_content) {
                    Ok(index) => {
                        self.open_note_editor(index);
                        self.last_action = format!(
                            "Created note in {}: {}",
                            self.note_save_target_label(),
                            title
                        );
                    }
                    Err(error) => {
                        self.set_result_panel("Note create failed", vec![error]);
                        self.last_action = String::from("Note create failed.");
                    }
                }
            }
            "note append" => {
                let (target_arg, append_arg) = Self::split_note_body_args(args.trim());
                let resolved_target = if append_arg.is_empty() {
                    self.current_note_index()
                } else {
                    self.resolve_note_index(target_arg)
                };

                let Some(index) = resolved_target else {
                    self.set_result_panel(
                        "Append failed",
                        vec![if append_arg.is_empty() {
                            String::from("No note is selected right now.")
                        } else {
                            format!("Note '{}' was not found.", target_arg.trim())
                        }],
                    );
                    self.last_action = String::from("No target note to append to.");
                    return;
                };

                let append_text = if append_arg.is_empty() {
                    target_arg.trim()
                } else {
                    append_arg.trim()
                };
                if append_text.is_empty() {
                    self.set_result_panel(
                        "Append failed",
                        vec![
                            String::from("Provide text after /note append."),
                            String::from("Usage: /note append <text>"),
                            String::from("   or: /note append <note> :: <text>"),
                        ],
                    );
                    self.last_action = String::from("Append text was empty.");
                    return;
                }

                let updated_at = self.uptime();
                let (note_title, note_content) = {
                    let note = &mut self.notes[index];
                    if !note.content.is_empty() {
                        note.content.push('\n');
                    }
                    note.content.push_str(append_text);
                    note.raw_content = note.content.clone();
                    note.updated_at = updated_at;
                    (note.title.clone(), note.content.clone())
                };
                if let Err(error) = self.persist_note(index) {
                    self.set_result_panel("Append save failed", vec![error]);
                    self.last_action = String::from("Note append save failed.");
                    return;
                }
                self.selected_note = index;
                self.set_result_panel(
                    format!("Note: {}", note_title),
                    vec![
                        String::from("Appended text to the note."),
                        String::new(),
                        note_content,
                    ],
                );
                self.last_action = format!("Appended to note: {}", note_title);
            }
            "note edit" => {
                if self.notes.is_empty() {
                    self.set_result_panel(
                        "Edit failed",
                        vec![String::from("No notes are available yet.")],
                    );
                    self.last_action = String::from("No note available to edit.");
                    return;
                }

                let resolved_index = if args.trim().is_empty() {
                    self.current_note_index()
                } else {
                    self.resolve_note_index(args.trim())
                };

                let Some(index) = resolved_index else {
                    self.set_result_panel("Edit failed", vec![String::from("Note not found.")]);
                    self.last_action = String::from("Note not found.");
                    return;
                };

                if self.is_strix_connected() {
                    if let Some(remote_id) = self.notes[index].remote_id.clone() {
                        if let Ok(note) = self.load_strix_note(&remote_id, true) {
                            let mut refreshed = note;
                            refreshed.id = self.notes[index].id;
                            refreshed.folder_id = self.notes[index].folder_id;
                            refreshed.obsidian_path = self.notes[index].obsidian_path.clone();
                            self.notes[index] = refreshed;
                            let _ = Self::save_cached_strix_notes(&self.notes);
                            let _ = Self::save_local_notes(&self.notes);
                        }
                    }
                }

                self.open_note_editor(index);
                self.last_action = format!("Editing note: {}", self.notes[index].title);
            }
            "note move" => {
                let args = args.trim();
                let parts: Vec<&str> = args.splitn(2, " to ").collect();
                if parts.len() != 2 {
                    self.set_result_panel(
                        "Move failed",
                        vec![String::from("Usage: /note move <note> to <folder>")],
                    );
                    self.last_action = String::from("Invalid note move syntax.");
                    return;
                }

                let note_ref = parts[0].trim();
                let folder_ref = parts[1].trim();

                let Some(note_index) = self.resolve_note_index(note_ref) else {
                    self.set_result_panel(
                        "Move failed",
                        vec![format!("Note '{}' not found.", note_ref)],
                    );
                    self.last_action = String::from("Note not found for move.");
                    return;
                };

                let Some(folder_id) = self.resolve_folder_id(folder_ref) else {
                    self.set_result_panel(
                        "Move failed",
                        vec![format!("Folder '{}' not found.", folder_ref)],
                    );
                    self.last_action = String::from("Folder not found for move.");
                    return;
                };

                let note_title = self.notes[note_index].title.clone();
                let folder_name = self.get_folder_name(folder_id).unwrap_or_default();
                self.notes[note_index].folder_id = Some(folder_id);
                self.notes[note_index].updated_at = self.uptime();

                self.set_result_panel(
                    "Note moved",
                    vec![format!(
                        "Moved '{}' to folder '{}'.",
                        note_title, folder_name
                    )],
                );
                self.last_action = format!("Moved note to folder: {}", folder_name);
            }
            "folder list" => {
                let lines = self.list_folders();
                self.set_result_panel("Folders", lines);
                self.last_action = String::from("Listed folders.");
            }
            "folder create" => {
                let name = args.trim();
                if name.is_empty() {
                    self.set_result_panel(
                        "Create failed",
                        vec![String::from("Provide a folder name after /folder create.")],
                    );
                    self.last_action = String::from("Folder name was empty.");
                    return;
                }

                let new_id = self.folders.iter().map(|f| f.id).max().unwrap_or(0) + 1;
                self.folders.push(Folder {
                    id: new_id,
                    name: name.to_string(),
                    parent_id: self.current_folder_id,
                });
                self.set_result_panel(
                    "Folder created",
                    vec![format!("Created folder '{}' with ID #{}.", name, new_id)],
                );
                self.last_action = format!("Created folder: {}", name);
            }
            "folder delete" => {
                let folder_ref = args.trim();
                if folder_ref.is_empty() {
                    self.set_result_panel(
                        "Delete failed",
                        vec![String::from(
                            "Provide a folder ID or name after /folder delete.",
                        )],
                    );
                    self.last_action = String::from("Folder reference was empty.");
                    return;
                }

                let Some(folder_id) = self.resolve_folder_id(folder_ref) else {
                    self.set_result_panel(
                        "Delete failed",
                        vec![format!("Folder '{}' not found.", folder_ref)],
                    );
                    self.last_action = String::from("Folder not found for deletion.");
                    return;
                };

                let folder_name = self.get_folder_name(folder_id).unwrap_or_default();

                // Move notes to parent or make them uncategorized
                for note in &mut self.notes {
                    if note.folder_id == Some(folder_id) {
                        note.folder_id = None;
                    }
                }

                // Remove subfolders by making them root folders
                for folder in &mut self.folders {
                    if folder.parent_id == Some(folder_id) {
                        folder.parent_id = None;
                    }
                }

                self.folders.retain(|f| f.id != folder_id);
                if self.current_folder_id == Some(folder_id) {
                    self.current_folder_id = None;
                }

                self.set_result_panel(
                    "Folder deleted",
                    vec![format!("Deleted folder '{}'.", folder_name)],
                );
                self.last_action = format!("Deleted folder: {}", folder_name);
            }
            "folder notes" => {
                let folder_ref = args.trim();
                let folder_id = if folder_ref.is_empty() {
                    self.current_folder_id
                } else {
                    self.resolve_folder_id(folder_ref)
                };

                let folder_name = folder_id
                    .and_then(|id| self.get_folder_name(id))
                    .unwrap_or_else(|| String::from("Uncategorized"));

                let lines: Vec<String> = self
                    .notes
                    .iter()
                    .enumerate()
                    .filter(|(_, note)| note.folder_id == folder_id)
                    .map(|(index, note)| {
                        format!(
                            "{:>2}. #{} {:<18} {}",
                            index + 1,
                            note.id,
                            note.title,
                            Self::preview_text(&note.content, 42)
                        )
                    })
                    .collect();

                if lines.is_empty() {
                    self.set_result_panel(
                        format!("Notes in: {}", folder_name),
                        vec![String::from("No notes in this folder.")],
                    );
                } else {
                    self.set_result_panel(format!("Notes in: {}", folder_name), lines);
                }
                self.last_action = format!("Listed notes in folder: {}", folder_name);
            }
            "folder tree" => {
                let lines = self.build_folder_tree_display();
                self.set_result_panel("Folder tree", lines);
                self.last_action = String::from("Displayed folder tree.");
            }
            "memory list" => {
                let lines = self
                    .memories
                    .iter()
                    .enumerate()
                    .map(|(index, memory)| format!("{:>2}. {}", index + 1, memory))
                    .collect::<Vec<_>>();
                self.set_result_panel("Memories", lines);
                self.last_action = String::from("Listed memories.");
            }
            "memory save" => {
                let memory = args.trim();
                if memory.is_empty() {
                    self.set_result_panel(
                        "Memory save failed",
                        vec![String::from("Provide text after /memory save.")],
                    );
                    self.last_action = String::from("Memory text was empty.");
                    return;
                }

                self.memories.push(memory.to_string());
                self.set_result_panel(
                    "Memory saved",
                    vec![
                        memory.to_string(),
                        String::from("Stored in the local demo memory list."),
                    ],
                );
                self.last_action = String::from("Saved a memory.");
            }
            "memory search" => {
                let query = args.trim().to_lowercase();
                let mut lines = self
                    .memories
                    .iter()
                    .filter(|memory| query.is_empty() || memory.to_lowercase().contains(&query))
                    .cloned()
                    .collect::<Vec<_>>();

                if lines.is_empty() {
                    lines.push(format!("No memory matched '{}'.", args.trim()));
                }

                self.set_result_panel(format!("Memory search: {}", args.trim()), lines);
                self.last_action = String::from("Searched memories.");
            }
            "serve mcp" => {
                self.set_result_panel(
                    "MCP server",
                    vec![
                        String::from("The MCP server entrypoint is still a stub in this sample build."),
                        String::from(
                            "Use this command to wire the gateway layer once the transport is ready.",
                        ),
                    ],
                );
                self.last_action = String::from("Prepared MCP server output.");
            }
            _ => {
                self.set_result_panel(
                    "Unknown command",
                    vec![format!("No local handler exists for /{}.", command)],
                );
                self.last_action = format!("Unknown: /{}", command);
            }
        }
    }

    pub(super) fn clear_notes_state(&mut self) {
        let _ = fs::remove_file(Self::local_notes_path());
        let _ = fs::remove_file(Self::strix_cache_path());
        self.clear_obsidian_vault_path();

        self.notes = Self::default_local_notes();
        self.folders.clear();
        self.current_folder_id = None;
        self.expanded_folders.clear();
        self.selected_note = 0;
        self.note_list_selected = 0;
        self.note_list_indices.clear();
        self.note_list_pending_delete = None;

        self.editor_note_index = None;
        self.editor_buffer.clear();
        self.editor_cursor = 0;
        self.editor_selection.clear();
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.search_state = SearchState::default();
        self.editing_title = false;
        self.title_buffer.clear();
        self.title_cursor = 0;

        self.obsidian_vault_path = None;
        self.obsidian_vaults = Self::discover_obsidian_vaults();
        self.obsidian_vault_selected = 0;
        self.note_save_target = NoteSaveTarget::Local;
        let _ = self.store_note_save_target();

        self.set_result_panel(
            "Notes cleared",
            vec![
                String::from("Cleared Aleph note caches and imported note state."),
                String::from("Obsidian is unpaired and note saving is set to Local."),
                String::from("Your Obsidian Markdown files were not deleted."),
            ],
        );
        self.last_action = String::from("Cleared notes and disabled Obsidian pairing.");
    }

    pub(super) fn split_note_body_args(args: &str) -> (&str, &str) {
        args.split_once("::").unwrap_or((args, ""))
    }

    pub(super) fn open_note_list_panel(&mut self) {
        if self.is_strix_connected() {
            self.ensure_cached_strix_notes_loaded();
        }

        if self.folders.is_empty() {
            self.note_list_indices = (0..self.notes.len()).collect();
            self.panel_lines = self
                .note_list_indices
                .iter()
                .enumerate()
                .map(|(list_index, &note_index)| self.note_list_line(list_index, note_index))
                .collect();
            self.note_list_selected = self
                .note_list_selected
                .min(self.note_list_indices.len().saturating_sub(1));
            self.panel_mode = PanelMode::NoteList;
            self.panel_title = String::from("Notes (Enter open, Delete delete)");
            return;
        }

        // Build hierarchical tree structure
        let mut tree_items: Vec<TreeItem> = Vec::new();

        // Get root folders (no parent)
        let root_folders: Vec<&Folder> = self
            .folders
            .iter()
            .filter(|f| f.parent_id.is_none())
            .collect();

        // Add uncategorized notes first
        let uncategorized_notes: Vec<usize> = self
            .notes
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

        // Recursively add folders and their notes
        for folder in &root_folders {
            self.build_folder_tree_items(&mut tree_items, folder.id, 0);
        }

        // Convert tree items to display lines and indices
        self.note_list_indices.clear();
        self.panel_lines.clear();

        for (_list_index, item) in tree_items.iter().enumerate() {
            match item {
                TreeItem::Folder {
                    id: _,
                    name,
                    depth,
                    expanded,
                    note_count,
                } => {
                    let indent = "  ".repeat(*depth);
                    let icon = if *expanded { "▼" } else { "▶" };
                    self.panel_lines
                        .push(format!("{}{} {} ({})", indent, icon, name, note_count));
                    self.note_list_indices.push(usize::MAX); // Marker for folder
                }
                TreeItem::Note { index, depth } => {
                    let indent = "  ".repeat(*depth);
                    let note = &self.notes[*index];
                    let line = format!("{}  • {}", indent, Self::truncate_chars(&note.title, 30));
                    self.panel_lines.push(line);
                    self.note_list_indices.push(*index);
                }
            }
        }

        self.note_list_selected = self
            .note_list_selected
            .min(self.panel_lines.len().saturating_sub(1));
        self.panel_mode = PanelMode::NoteList;
        self.panel_title = String::from("Notes (Enter open, Space expand/collapse, Delete delete)");
    }

    pub(super) fn build_folder_tree_items(
        &self,
        tree_items: &mut Vec<TreeItem>,
        folder_id: usize,
        depth: usize,
    ) {
        let folder_name = self
            .get_folder_name(folder_id)
            .unwrap_or_else(|| String::from("Unknown"));
        let note_count = self
            .notes
            .iter()
            .filter(|n| n.folder_id == Some(folder_id))
            .count();
        let expanded = self.expanded_folders.contains(&folder_id);

        tree_items.push(TreeItem::Folder {
            id: folder_id,
            name: folder_name,
            depth,
            expanded,
            note_count,
        });

        if expanded {
            // Add notes in this folder
            for (note_index, note) in self.notes.iter().enumerate() {
                if note.folder_id == Some(folder_id) {
                    tree_items.push(TreeItem::Note {
                        index: note_index,
                        depth: depth + 1,
                    });
                }
            }

            // Add subfolders
            let subfolders: Vec<&Folder> = self
                .folders
                .iter()
                .filter(|f| f.parent_id == Some(folder_id))
                .collect();

            for subfolder in subfolders {
                self.build_folder_tree_items(tree_items, subfolder.id, depth + 1);
            }
        }
    }

    pub(super) fn note_list_line(&self, list_index: usize, note_index: usize) -> String {
        let note = &self.notes[note_index];
        let folder_indicator = if let Some(fid) = note.folder_id {
            let fname = self.get_folder_name(fid).unwrap_or_default();
            format!("[{}] ", Self::truncate_chars(&fname, 8))
        } else {
            String::from("[-] ")
        };
        let source_indicator = if note.obsidian_path.is_some() {
            String::from(" [obsidian]")
        } else {
            note.remote_id
                .as_deref()
                .map(|id| format!(" [{}]", id))
                .unwrap_or_default()
        };
        format!(
            "{:>2}. #{} {:<14} {}{}{}",
            list_index + 1,
            note.id,
            if note.title.chars().count() > 14 {
                format!("{}...", Self::truncate_chars(&note.title, 11))
            } else {
                note.title.clone()
            },
            folder_indicator,
            Self::preview_text(&note.content, 32),
            source_indicator
        )
    }

    pub(super) fn truncate_chars(value: &str, max_chars: usize) -> String {
        value.chars().take(max_chars).collect()
    }

    pub(super) fn create_note_from_content(
        &mut self,
        title: &str,
        content: &str,
    ) -> Result<usize, String> {
        let title = if title.trim().is_empty() {
            String::from("Untitled note")
        } else {
            title.trim().to_string()
        };
        let note_id = self.notes.iter().map(|n| n.id).max().unwrap_or(0) + 1;
        let note = Note {
            id: note_id,
            remote_id: None,
            obsidian_path: None,
            title: title.clone(),
            content: content.to_string(),
            raw_content: content.to_string(),
            updated_at: self.uptime(),
            folder_id: self.current_folder_id,
        };

        self.notes.push(note);
        let index = self.notes.len() - 1;
        if let Err(error) = self.persist_note(index) {
            self.notes.pop();
            return Err(error);
        }
        Ok(index)
    }

    pub(super) fn delete_note_at_index(&mut self, index: usize) -> Result<String, String> {
        let Some(note) = self.notes.get(index).cloned() else {
            return Err(String::from("Note not found."));
        };

        if let Some(path) = note.obsidian_path.as_ref() {
            match fs::remove_file(path) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    return Err(format!("failed to delete '{}': {}", path.display(), error));
                }
            }
        }

        if note.remote_id.is_some() && self.is_strix_connected() {
            self.delete_strix_note(&note)?;
        }

        self.notes.remove(index);
        self.note_list_pending_delete = None;
        self.selected_note = if self.notes.is_empty() {
            0
        } else if self.selected_note > index {
            self.selected_note - 1
        } else {
            self.selected_note.min(self.notes.len() - 1)
        };
        self.note_list_indices = self
            .note_list_indices
            .iter()
            .filter_map(|&note_index| {
                if note_index == index {
                    None
                } else if note_index > index {
                    Some(note_index - 1)
                } else {
                    Some(note_index)
                }
            })
            .collect();
        self.note_list_selected = self
            .note_list_selected
            .min(self.note_list_indices.len().saturating_sub(1));
        if self.is_strix_connected() {
            let _ = Self::save_cached_strix_notes(&self.notes);
        }
        let _ = Self::save_local_notes(&self.notes);

        Ok(note.title)
    }

    pub(super) fn set_result_panel(&mut self, title: impl Into<String>, lines: Vec<String>) {
        self.panel_mode = PanelMode::Commands;
        self.panel_title = title.into();
        self.panel_lines = lines;
        self.editor_note_index = None;
    }

    pub(super) fn push_chat_message(
        &mut self,
        role: impl Into<String>,
        content: impl Into<String>,
    ) {
        self.chat_messages.push(ChatMessage {
            role: role.into(),
            content: content.into(),
            timestamp: self.uptime(),
        });

        if self.chat_messages.len() > MAX_CHAT_MESSAGES {
            let overflow = self.chat_messages.len() - MAX_CHAT_MESSAGES;
            self.chat_messages.drain(0..overflow);
        }

        self.rebuild_chat_render_cache();
    }

    pub(super) fn scroll_chat_up(&mut self, lines: usize) {
        self.chat_scroll_offset = self.chat_scroll_offset.saturating_add(lines);
    }

    pub(super) fn scroll_chat_down(&mut self, lines: usize) {
        self.chat_scroll_offset = self.chat_scroll_offset.saturating_sub(lines);
    }

    pub(super) fn add_strix_log(&mut self, message: impl Into<String>) {
        let timestamp = self.uptime();
        self.strix_logs
            .push(format!("[{}] {}", timestamp, message.into()));
        // Keep only last 50 log entries
        if self.strix_logs.len() > 50 {
            self.strix_logs.drain(0..self.strix_logs.len() - 50);
        }
    }

    pub(super) fn set_ai_provider(&mut self, provider: AiProvider) {
        self.ai_provider = provider;
        let _ = self.store_ai_provider();
        self.add_strix_log(format!("Switched to {:?} provider", provider));
    }

    pub(super) fn clear_strix_logs(&mut self) {
        self.strix_logs.clear();
    }

    pub(super) fn refresh_connection_state(&mut self) {
        self.connected = self.openrouter_api_key.is_some() || self.strix_access_token.is_some();
    }
}
