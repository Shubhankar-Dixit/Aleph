use super::*;

#[allow(dead_code)]
impl App {
    pub(super) fn load_strix_notes(&self, query: &str, limit: usize) -> Result<Vec<Note>, String> {
        let path = if query.trim().is_empty() {
            format!("/api/auth/native/notes?limit={}", limit)
        } else {
            format!(
                "/api/auth/native/notes?q={}&limit={}",
                urlencoding::encode(query.trim()),
                limit
            )
        };
        let value = self.strix_json_request("GET", &path, None)?;
        let notes = value
            .get("notes")
            .and_then(|notes| notes.as_array())
            .ok_or_else(|| String::from("Strix notes response did not include notes"))?;

        Ok(notes
            .iter()
            .enumerate()
            .map(|(index, value)| Self::note_from_strix_value(index + 1, value))
            .collect())
    }

    pub(super) fn load_strix_note(
        &self,
        id_or_title: &str,
        hydrate_content: bool,
    ) -> Result<Note, String> {
        let remote_id = self
            .resolve_note_index(id_or_title)
            .and_then(|index| self.notes.get(index))
            .and_then(|note| note.remote_id.clone())
            .unwrap_or_else(|| id_or_title.trim().to_string());
        let path = format!(
            "/api/auth/native/notes/{}",
            urlencoding::encode(remote_id.trim())
        );
        let value = self.strix_json_request("GET", &path, None)?;
        let note = value
            .get("note")
            .ok_or_else(|| String::from("Strix note response did not include note"))?;
        let mut parsed = Self::note_from_strix_value(self.notes.len() + 1, note);
        if !hydrate_content {
            parsed.raw_content.clear();
        }
        Ok(parsed)
    }

    pub(super) fn create_strix_note(&self, title: &str, content: &str) -> Result<Note, String> {
        let payload = serde_json::json!({
            "title": title,
            "content": Self::text_to_strix_html(content),
            "tags": [],
        });
        let value = self.strix_json_request("POST", "/api/auth/native/notes", Some(payload))?;
        let note = value
            .get("note")
            .ok_or_else(|| String::from("Strix create response did not include note"))?;
        Ok(Self::note_from_strix_value(self.notes.len() + 1, note))
    }

    pub(super) fn update_strix_note(&self, note: &Note) -> Result<Note, String> {
        let Some(remote_id) = note.remote_id.as_deref() else {
            return Ok(note.clone());
        };
        let payload = serde_json::json!({
            "title": note.title,
            "content": Self::text_to_strix_html(&note.content),
        });
        let path = format!(
            "/api/auth/native/notes/{}",
            urlencoding::encode(remote_id.trim())
        );
        let value = self.strix_json_request("PATCH", &path, Some(payload))?;
        let note = value
            .get("note")
            .ok_or_else(|| String::from("Strix update response did not include note"))?;
        Ok(Self::note_from_strix_value(0, note))
    }

    pub(super) fn delete_strix_note(&self, note: &Note) -> Result<(), String> {
        let Some(remote_id) = note.remote_id.as_deref() else {
            return Ok(());
        };
        let path = format!(
            "/api/auth/native/notes/{}",
            urlencoding::encode(remote_id.trim())
        );
        let _ = self.strix_json_request("DELETE", &path, None)?;
        Ok(())
    }

    pub(super) fn push_note_to_strix(&mut self, index: usize) -> Result<(), String> {
        if !self.is_strix_connected() {
            return Err(String::from(
                "Strix save target requires a Strix connection. Use /login strix first.",
            ));
        }
        let Some(note) = self.notes.get(index).cloned() else {
            return Ok(());
        };
        let obsidian_path = note.obsidian_path.clone();
        let mut synced = if note.remote_id.is_some() {
            self.update_strix_note(&note)?
        } else {
            self.create_strix_note(&note.title, &note.content)?
        };
        synced.id = note.id;
        synced.folder_id = note.folder_id;
        synced.obsidian_path = obsidian_path;
        if let Some(slot) = self.notes.get_mut(index) {
            *slot = synced;
        }
        Self::save_cached_strix_notes(&self.notes)?;
        self.add_strix_log("Pushed note changes to Strix");
        Ok(())
    }

    pub(super) fn upsert_synced_note(&mut self, mut note: Note) {
        if let Some(remote_id) = note.remote_id.clone() {
            if let Some((index, existing)) = self
                .notes
                .iter_mut()
                .enumerate()
                .find(|(_, existing)| existing.remote_id.as_deref() == Some(remote_id.as_str()))
            {
                note.id = existing.id;
                if note.obsidian_path.is_none() {
                    note.obsidian_path = existing.obsidian_path.clone();
                }
                if note.folder_id.is_none() {
                    note.folder_id = existing.folder_id;
                }
                *existing = note;
                self.selected_note = index;
                let _ = Self::save_cached_strix_notes(&self.notes);
                let _ = Self::save_local_notes(&self.notes);
                return;
            }
        }

        note.id = self.notes.len() + 1;
        self.notes.push(note);
        self.selected_note = self.notes.len() - 1;
        let _ = Self::save_cached_strix_notes(&self.notes);
        let _ = Self::save_local_notes(&self.notes);
    }

    pub(super) fn note_from_strix_value(local_id: usize, value: &serde_json::Value) -> Note {
        let remote_id = value
            .get("id")
            .and_then(|id| id.as_str())
            .map(|id| id.to_string());
        let updated_at = value
            .get("updatedAt")
            .and_then(|updated| {
                if updated.is_number() {
                    updated.as_i64().map(|number| number.to_string())
                } else {
                    updated.as_str().map(|text| text.to_string())
                }
            })
            .unwrap_or_else(|| String::from("strix"));
        let raw_content = value
            .get("content")
            .and_then(|content| content.as_str())
            .unwrap_or("")
            .to_string();

        Note {
            id: local_id,
            remote_id,
            obsidian_path: None,
            title: value
                .get("title")
                .and_then(|title| title.as_str())
                .unwrap_or("Untitled")
                .to_string(),
            content: Self::html_to_terminal_text(&raw_content),
            raw_content,
            updated_at,
            folder_id: None,
        }
    }

    pub(super) fn note_source_label(note: &Note) -> String {
        if let Some(path) = note.obsidian_path.as_ref() {
            return format!("Obsidian: {}", path.display());
        }
        if let Some(remote_id) = note.remote_id.as_deref() {
            return format!("Strix ID: {}", remote_id);
        }
        String::from("Source: local-only")
    }

    pub(super) fn ensure_cached_strix_notes_loaded(&mut self) {
        let has_remote_notes = self.notes.iter().any(|note| note.remote_id.is_some());
        if has_remote_notes {
            return;
        }

        if let Ok(notes) = Self::load_cached_strix_notes() {
            if !notes.is_empty() {
                self.merge_strix_notes(notes);
                self.selected_note = 0;
                self.add_strix_log("Loaded cached Strix notes");
            }
        }
    }

    pub(super) fn strix_cache_path() -> PathBuf {
        if let Ok(path) = std::env::var("ALEPH_STRIX_CACHE") {
            return PathBuf::from(path);
        }

        if let Ok(dir) = std::env::var("ALEPH_CACHE_DIR") {
            return PathBuf::from(dir).join("strix-notes.json");
        }

        if let Ok(dir) = std::env::var("XDG_CACHE_HOME") {
            return PathBuf::from(dir).join("aleph").join("strix-notes.json");
        }

        if let Ok(dir) = std::env::var("LOCALAPPDATA").or_else(|_| std::env::var("APPDATA")) {
            return PathBuf::from(dir).join("Aleph").join("strix-notes.json");
        }

        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home)
                .join(".cache")
                .join("aleph")
                .join("strix-notes.json");
        }

        std::env::temp_dir().join("aleph-strix-notes.json")
    }

    pub(super) fn local_notes_path() -> PathBuf {
        if let Ok(path) = std::env::var("ALEPH_NOTES_PATH") {
            return PathBuf::from(path);
        }

        #[cfg(test)]
        return std::env::temp_dir().join("aleph-test-notes-disabled.json");

        #[cfg(not(test))]
        Self::aleph_config_dir().join(LOCAL_NOTES_CONFIG)
    }

    pub(super) fn load_local_notes() -> Result<Vec<Note>, String> {
        let path = Self::local_notes_path();
        if !path.exists() {
            return Err(String::from("local note cache does not exist"));
        }

        let body = fs::read_to_string(&path).map_err(|error| {
            format!("failed to read local notes '{}': {}", path.display(), error)
        })?;
        let value: serde_json::Value = serde_json::from_str(&body).map_err(|error| {
            format!(
                "failed to parse local notes '{}': {}",
                path.display(),
                error
            )
        })?;
        let notes = value
            .get("notes")
            .and_then(|notes| notes.as_array())
            .ok_or_else(|| String::from("local note cache did not include notes"))?;

        let loaded = notes
            .iter()
            .enumerate()
            .filter_map(|(index, value)| Self::note_from_local_value(index + 1, value))
            .filter(|note| !Self::is_legacy_sample_note(note))
            .collect::<Vec<_>>();
        if loaded.is_empty() {
            Err(String::from("local note cache was empty"))
        } else {
            Ok(loaded)
        }
    }

    pub(super) fn save_local_notes(notes: &[Note]) -> Result<(), String> {
        let path = Self::local_notes_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                format!(
                    "failed to create local notes directory '{}': {}",
                    parent.display(),
                    error
                )
            })?;
        }

        let cached_notes = notes
            .iter()
            .map(|note| {
                serde_json::json!({
                    "id": note.id,
                    "remoteId": note.remote_id.as_deref(),
                    "obsidianPath": note.obsidian_path.as_ref().map(|path| path.display().to_string()),
                    "title": note.title.as_str(),
                    "content": note.content.as_str(),
                    "rawContent": note.raw_content.as_str(),
                    "updatedAt": note.updated_at.as_str(),
                    "folderId": note.folder_id,
                })
            })
            .collect::<Vec<_>>();
        let payload = serde_json::json!({
            "version": 1,
            "savedAt": Self::now_millis(),
            "notes": cached_notes,
        });

        fs::write(
            &path,
            serde_json::to_string_pretty(&payload)
                .map_err(|error| format!("failed to encode local notes: {}", error))?,
        )
        .map_err(|error| {
            format!(
                "failed to write local notes '{}': {}",
                path.display(),
                error
            )
        })
    }

    pub(super) fn note_from_local_value(
        local_id: usize,
        value: &serde_json::Value,
    ) -> Option<Note> {
        let title = value.get("title").and_then(|title| title.as_str())?.trim();
        let content = value
            .get("content")
            .and_then(|content| content.as_str())
            .unwrap_or("")
            .to_string();
        let raw_content = value
            .get("rawContent")
            .and_then(|content| content.as_str())
            .unwrap_or(&content)
            .to_string();
        let obsidian_path = value
            .get("obsidianPath")
            .and_then(|path| path.as_str())
            .map(PathBuf::from);

        Some(Note {
            id: value
                .get("id")
                .and_then(|id| id.as_u64())
                .map(|id| id as usize)
                .unwrap_or(local_id),
            remote_id: value
                .get("remoteId")
                .and_then(|id| id.as_str())
                .map(str::to_string)
                .filter(|id| !id.trim().is_empty()),
            obsidian_path,
            title: title.to_string(),
            content,
            raw_content,
            updated_at: value
                .get("updatedAt")
                .and_then(|updated| updated.as_str())
                .unwrap_or("local")
                .to_string(),
            folder_id: value
                .get("folderId")
                .and_then(|id| id.as_u64())
                .map(|id| id as usize),
        })
    }

    pub(super) fn is_legacy_sample_note(note: &Note) -> bool {
        note.remote_id.is_none()
            && note.obsidian_path.is_none()
            && matches!(
                note.title.as_str(),
                "Strix gateway" | "Note editor" | "MCP server" | "Feature ideas"
            )
    }

    pub(super) fn load_cached_strix_notes() -> Result<Vec<Note>, String> {
        let path = Self::strix_cache_path();
        if !path.exists() {
            return Ok(Vec::new());
        }

        let body = fs::read_to_string(&path)
            .map_err(|error| format!("failed to read Strix note cache: {}", error))?;
        let value: serde_json::Value = serde_json::from_str(&body)
            .map_err(|error| format!("failed to parse Strix note cache: {}", error))?;
        let notes = value
            .get("notes")
            .and_then(|notes| notes.as_array())
            .ok_or_else(|| String::from("Strix note cache did not include notes"))?;

        Ok(notes
            .iter()
            .enumerate()
            .filter_map(|(index, value)| {
                let note = Self::note_from_strix_value(index + 1, value);
                if note.remote_id.is_some() {
                    Some(note)
                } else {
                    None
                }
            })
            .collect())
    }

    pub(super) fn save_cached_strix_notes(notes: &[Note]) -> Result<(), String> {
        let path = Self::strix_cache_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                format!("failed to create Strix note cache directory: {}", error)
            })?;
        }

        let cached_notes = notes
            .iter()
            .filter(|note| note.remote_id.is_some())
            .map(|note| {
                serde_json::json!({
                    "id": note.remote_id.as_deref().unwrap_or(""),
                    "title": note.title.as_str(),
                    "content": if note.raw_content.trim().is_empty() {
                        Self::text_to_strix_html(&note.content)
                    } else {
                        note.raw_content.clone()
                    },
                    "updatedAt": note.updated_at,
                })
            })
            .collect::<Vec<_>>();
        let payload = serde_json::json!({
            "version": 1,
            "syncedAt": Self::now_millis(),
            "notes": cached_notes,
        });
        fs::write(
            &path,
            serde_json::to_string_pretty(&payload)
                .map_err(|error| format!("failed to encode Strix note cache: {}", error))?,
        )
        .map_err(|error| format!("failed to write Strix note cache: {}", error))
    }

    pub(super) fn now_millis() -> u128 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_millis())
            .unwrap_or(0)
    }

    pub(super) fn html_to_terminal_text(input: &str) -> String {
        if !input.contains('<') {
            return Self::decode_html_entities(input).trim().to_string();
        }

        let mut output = String::new();
        let mut chars = input.chars().peekable();
        while let Some(character) = chars.next() {
            if character != '<' {
                output.push(character);
                continue;
            }

            let mut tag = String::new();
            for next in chars.by_ref() {
                if next == '>' {
                    break;
                }
                tag.push(next);
            }
            let normalized = tag.trim().trim_start_matches('/').to_lowercase();
            let closing = tag.trim_start().starts_with('/');

            if closing {
                if normalized.starts_with('p')
                    || normalized.starts_with("div")
                    || normalized.starts_with("li")
                    || normalized.starts_with("h1")
                    || normalized.starts_with("h2")
                    || normalized.starts_with("h3")
                {
                    Self::push_collapsed_newline(&mut output);
                }
                continue;
            }

            if normalized.starts_with("br") {
                output.push('\n');
            } else if normalized.starts_with("h1") {
                Self::push_block_prefix(&mut output, "# ");
            } else if normalized.starts_with("h2") {
                Self::push_block_prefix(&mut output, "## ");
            } else if normalized.starts_with("h3") {
                Self::push_block_prefix(&mut output, "### ");
            } else if normalized.starts_with("li") {
                if normalized.contains("data-type=\"taskitem\"")
                    || normalized.contains("data-task-item=\"true\"")
                {
                    if normalized.contains("data-checked=\"true\"") {
                        Self::push_block_prefix(&mut output, "- [x] ");
                    } else {
                        Self::push_block_prefix(&mut output, "- [ ] ");
                    }
                } else {
                    Self::push_block_prefix(&mut output, "- ");
                }
            }
        }

        Self::decode_html_entities(&output)
            .lines()
            .map(str::trim_end)
            .collect::<Vec<_>>()
            .join("\n")
            .replace("\n\n\n", "\n\n")
            .trim()
            .to_string()
    }

    pub(super) fn push_block_prefix(output: &mut String, prefix: &str) {
        if !output.trim_end().is_empty() {
            Self::push_collapsed_newline(output);
        }
        output.push_str(prefix);
    }

    pub(super) fn push_collapsed_newline(output: &mut String) {
        if output.ends_with("\n\n") {
            return;
        }
        if output.ends_with('\n') {
            output.push('\n');
        } else {
            output.push_str("\n\n");
        }
    }

    pub(super) fn decode_html_entities(input: &str) -> String {
        input
            .replace("&nbsp;", " ")
            .replace("&amp;", "&")
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&quot;", "\"")
            .replace("&#39;", "'")
    }

    pub(super) fn text_to_strix_html(input: &str) -> String {
        let mut html = String::new();
        let mut task_list_open = false;

        for line in input.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                if task_list_open {
                    html.push_str("</ul>");
                    task_list_open = false;
                }
                continue;
            }

            if let Some(task) = trimmed
                .strip_prefix("- [ ] ")
                .or_else(|| trimmed.strip_prefix("- [x] "))
            {
                if !task_list_open {
                    html.push_str("<ul data-type=\"taskList\">");
                    task_list_open = true;
                }
                let checked = if trimmed.starts_with("- [x] ") {
                    "true"
                } else {
                    "false"
                };
                html.push_str(&format!(
                    "<li data-type=\"taskItem\" data-task-item=\"true\" data-checked=\"{}\"><label><input type=\"checkbox\"><span></span></label><div><p>{}</p></div></li>",
                    checked,
                    Self::escape_html(task)
                ));
                continue;
            }

            if task_list_open {
                html.push_str("</ul>");
                task_list_open = false;
            }

            if let Some(text) = trimmed.strip_prefix("### ") {
                html.push_str(&format!("<h3>{}</h3>", Self::escape_html(text)));
            } else if let Some(text) = trimmed.strip_prefix("## ") {
                html.push_str(&format!("<h2>{}</h2>", Self::escape_html(text)));
            } else if let Some(text) = trimmed.strip_prefix("# ") {
                html.push_str(&format!("<h1>{}</h1>", Self::escape_html(text)));
            } else if let Some(text) = trimmed.strip_prefix("- ") {
                html.push_str(&format!(
                    "<ul><li><p>{}</p></li></ul>",
                    Self::escape_html(text)
                ));
            } else {
                html.push_str(&format!("<p>{}</p>", Self::escape_html(trimmed)));
            }
        }

        if task_list_open {
            html.push_str("</ul>");
        }

        if html.is_empty() {
            String::from("<p></p>")
        } else {
            html
        }
    }

    pub(super) fn escape_html(input: &str) -> String {
        input
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
            .replace('\'', "&#39;")
    }

    pub(super) fn load_openrouter_api_key() -> Option<String> {
        if let Ok(entry) = Self::openrouter_key_entry() {
            if let Ok(password) = entry.get_password() {
                let trimmed = password.trim().to_string();
                if !trimmed.is_empty() {
                    return Some(trimmed);
                }
            }
        }

        std::env::var("OPENROUTER_API_KEY")
            .ok()
            .map(|key| key.trim().to_string())
            .filter(|key| !key.is_empty())
    }

    pub(super) fn store_openrouter_api_key(&self, api_key: &str) -> Result<(), String> {
        let entry = Self::openrouter_key_entry()?;
        entry
            .set_password(api_key.trim())
            .map_err(|error| format!("failed to save OpenRouter API key: {}", error))
    }

    pub(super) fn clear_openrouter_api_key(&self) {
        if let Ok(entry) = Self::openrouter_key_entry() {
            let _ = entry.delete_credential();
        }
    }

    pub(super) fn reset_and_clear_all(&mut self) {
        // Clear API keys from keyring
        self.clear_openrouter_api_key();
        self.clear_strix_access_token();
        self.clear_obsidian_vault_path();

        // Clear all cached data files
        let _ = std::fs::remove_file(Self::strix_cache_path());
        let _ = std::fs::remove_file(Self::local_notes_path());
        let _ = std::fs::remove_file(Self::ai_provider_path());
        let _ = std::fs::remove_file(Self::note_save_target_path());
        let _ = std::fs::remove_file(Self::agent_mode_path());

        // Reset all settings to defaults
        self.ai_provider = AiProvider::OpenRouter;
        self.note_save_target = NoteSaveTarget::Local;
        self.agent_mode_enabled = false;

        // Clear connection state
        self.openrouter_api_key = None;
        self.strix_access_token = None;
        self.obsidian_vault_path = None;
        self.obsidian_vaults.clear();
        self.obsidian_vault_selected = 0;

        // Clear chat and notes data
        self.chat_messages.clear();
        self.chat_input_buffer.clear();
        self.chat_input_cursor = 0;
        self.chat_scroll_offset = 0;
        self.chat_render_cache.clear();
        self.chat_render_dirty = true;
        self.chat_cache_stable_len = 0;
        self.notes = Self::default_local_notes();
        self.folders.clear();
        self.current_folder_id = None;
        self.expanded_folders.clear();
        self.selected_note = 0;
        self.memories.clear();
        self.canvases.clear();

        // Clear editor state
        self.editor_buffer.clear();
        self.editor_cursor = 0;
        self.editor_selection.clear();
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.search_state = SearchState::default();
        self.editor_note_index = None;
        self.editing_title = false;
        self.title_buffer.clear();
        self.title_cursor = 0;

        // Clear AI state
        self.ai_input_buffer.clear();
        self.ai_input_cursor = 0;
        self.pending_ai_edit = None;
        self.ai_draft_create_title = None;
        self.ghost_result = None;
        self.ghost_streaming = false;
        self.strix_logs.clear();
        self.streaming_buffer.clear();
        self.streaming_active = false;

        // Cancel any ongoing operations
        self.chat_stream_rx = None;
        self.openrouter_login_rx = None;
        self.strix_login_rx = None;
        self.ghost_stream_rx = None;
        if let Some(cancel_flag) = &self.openrouter_login_cancel {
            cancel_flag.store(true, Ordering::Relaxed);
        }
        self.openrouter_login_cancel = None;
        if let Some(cancel_flag) = &self.strix_login_cancel {
            cancel_flag.store(true, Ordering::Relaxed);
        }
        self.strix_login_cancel = None;

        // Reset UI state
        self.thinking = false;
        self.thinking_ticks_remaining = 0;
        self.ai_overlay_visible = false;
        self.ai_overlay_pulse_ticks = 0;
        self.save_shimmer_ticks = 0;
        self.panel_mode = PanelMode::Commands;
        self.panel_title = String::from("Commands");
        self.panel_lines.clear();
        self.suggestion_filter = None;
        self.history.clear();
        self.history_index = None;
        self.last_action = String::from("All data cleared and settings reset.");

        // Refresh connection state
        self.refresh_connection_state();
    }

    pub(super) fn openrouter_key_entry() -> Result<Entry, String> {
        Entry::new(OPENROUTER_SERVICE, OPENROUTER_ACCOUNT)
            .map_err(|error| format!("failed to open OpenRouter API key store: {}", error))
    }
}
