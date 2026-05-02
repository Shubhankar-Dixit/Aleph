use super::*;

const TEMPORAL_FORKS_CONFIG: &str = "temporal-forks.json";
const FORK_ACTIVITY_CONTEXT_LIMIT: usize = 12;
const FORK_CHAT_CONTEXT_LIMIT: usize = 8;

#[allow(dead_code)]
impl App {
    pub(super) fn handle_fork_command(&mut self, command: &str, args: &str) {
        match command {
            "path save" | "world save" | "fork now" => {
                let label = if args.trim().is_empty() {
                    String::from("decision point")
                } else {
                    args.trim().to_string()
                };
                match self.create_temporal_fork(&label, "manual") {
                    Ok(id) => {
                        self.add_activity(format!("Saved decision point {}.", id));
                        self.set_result_panel(
                            "Saved path",
                            vec![
                                format!("Saved decision point: {}", label),
                                format!("Path ID: {}", id),
                                String::from(
                                    "Aleph captured your ideas, memory, recent context, and read-only code context.",
                                ),
                                String::from("Use /path list, /path show <name>, or /path return <name>."),
                            ],
                        );
                        self.last_action = format!("Saved decision point: {}", label);
                    }
                    Err(error) => {
                        self.set_result_panel("Path save failed", vec![error]);
                        self.last_action = String::from("Path save failed.");
                    }
                }
            }
            "path list" | "world list" | "fork list" => {
                let lines = self.temporal_fork_list_lines();
                self.set_result_panel("Saved paths", lines);
                self.last_action = String::from("Listed saved paths.");
            }
            "path show" | "world show" | "fork read" => {
                let target = args.trim();
                let Some(index) = self.resolve_temporal_fork_index(target) else {
                    self.set_result_panel(
                        "Path not found",
                        vec![String::from("Use /path list, then /path show <name|id>.")],
                    );
                    self.last_action = String::from("Saved path not found.");
                    return;
                };
                let lines = self.temporal_fork_detail_lines(index);
                let title = format!("Path: {}", self.temporal_forks[index].label);
                self.set_result_panel(title, lines);
                self.last_action = String::from("Opened saved path.");
            }
            "path return" | "world return" | "fork checkout" => {
                let target = args.trim();
                let Some(index) = self.resolve_temporal_fork_index(target) else {
                    self.set_result_panel(
                        "Path not found",
                        vec![String::from("Use /path list, then /path return <name|id>.")],
                    );
                    self.last_action = String::from("Saved path not found.");
                    return;
                };
                match self.checkout_temporal_fork(index) {
                    Ok(lines) => {
                        self.set_result_panel("Returned to path", lines);
                        self.last_action = String::from("Returned to saved path.");
                    }
                    Err(error) => {
                        self.set_result_panel("Path return failed", vec![error]);
                        self.last_action = String::from("Path return failed.");
                    }
                }
            }
            _ => {}
        }
    }

    pub(super) fn create_auto_temporal_fork(&mut self, label: &str) {
        let _ = self.create_temporal_fork(label, "auto");
    }

    pub(super) fn create_temporal_fork(
        &mut self,
        label: &str,
        reason: &str,
    ) -> Result<String, String> {
        let id = self.next_temporal_fork_id();
        let previous_current_fork_id = self.current_fork_id.clone();
        let parent_id = previous_current_fork_id.clone();
        let created_at = Self::now_millis().to_string();
        let activity_context = self
            .activity_log
            .iter()
            .rev()
            .take(FORK_ACTIVITY_CONTEXT_LIMIT)
            .cloned()
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>();
        let chat_context = self
            .chat_messages
            .iter()
            .rev()
            .take(FORK_CHAT_CONTEXT_LIMIT)
            .cloned()
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>();

        self.temporal_forks.push(TemporalFork {
            id: id.clone(),
            parent_id,
            label: label.trim().to_string(),
            reason: reason.trim().to_string(),
            created_at,
            notes: self.notes.clone(),
            folders: self.folders.clone(),
            memories: self.memories.clone(),
            selected_note: self.selected_note,
            activity_context,
            chat_context,
            repo_context: Self::capture_repo_context(),
        });
        self.current_fork_id = Some(id.clone());
        if let Err(error) = self.save_temporal_fork_state() {
            self.temporal_forks.pop();
            self.current_fork_id = previous_current_fork_id;
            return Err(error);
        }
        Ok(id)
    }

    pub(super) fn checkout_temporal_fork(&mut self, index: usize) -> Result<Vec<String>, String> {
        let Some(fork) = self.temporal_forks.get(index).cloned() else {
            return Err(String::from("Path not found."));
        };

        self.notes = fork.notes.clone();
        self.folders = fork.folders.clone();
        self.memories = fork.memories.clone();
        self.selected_note = if self.notes.is_empty() {
            0
        } else {
            fork.selected_note.min(self.notes.len() - 1)
        };
        self.current_fork_id = Some(fork.id.clone());
        self.activity_log = fork.activity_context.iter().cloned().collect();
        self.chat_messages = fork.chat_context.clone();
        self.rebuild_chat_render_cache();
        self.editor_note_index = None;
        self.editor_buffer.clear();
        self.editor_cursor = 0;
        self.editor_selection.clear();
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.search_state = SearchState::default();
        self.note_list_selected = 0;
        self.note_list_indices.clear();
        self.note_list_pending_delete = None;
        self.editing_title = false;
        self.title_buffer.clear();
        self.title_cursor = 0;

        Self::save_local_notes(&self.notes)?;
        self.save_temporal_fork_state()?;
        self.add_activity(format!("Returned to path {}.", fork.id));

        let mut lines = vec![
            format!("Returned to: {}", fork.label),
            format!("Path ID: {}", fork.id),
            format!("Notes restored: {}", self.notes.len()),
            format!("Memories restored: {}", self.memories.len()),
            String::from(
                "This is version control for thinking. Obsidian, Strix, and git were not changed.",
            ),
        ];
        if let Some(repo) = fork.repo_context {
            lines.push(String::new());
            lines.push(format!("Repo: {}", repo.cwd));
            if let Some(branch) = repo.branch {
                lines.push(format!("Branch then: {}", branch));
            }
            if let Some(head) = repo.head {
                lines.push(format!("HEAD then: {}", head));
            }
        }
        Ok(lines)
    }

    pub(super) fn temporal_fork_list_lines(&self) -> Vec<String> {
        if self.temporal_forks.is_empty() {
            return vec![String::from(
                "No paths saved yet. Use /path save <name> at a decision point.",
            )];
        }

        let mut lines = vec![
            String::from(
                "Each path is a saved decision point: ideas, memory, recent context, and read-only code context.",
            ),
            String::from("Return to one with /path return <name|id> to explore another path."),
            String::new(),
        ];
        lines.extend(self.temporal_forks.iter().rev().take(20).map(|fork| {
            let repo = fork
                .repo_context
                .as_ref()
                .and_then(|repo| {
                    repo.branch
                        .as_ref()
                        .map(|branch| format!(" [{}]", branch))
                        .or_else(|| repo.head.as_ref().map(|head| format!(" [{}]", head)))
                })
                .unwrap_or_default();
            let current = if self.current_fork_id.as_deref() == Some(fork.id.as_str()) {
                " *"
            } else {
                ""
            };
            format!(
                "{}{}  {}  {}  notes:{}{}",
                fork.id,
                current,
                fork.created_at,
                fork.label,
                fork.notes.len(),
                repo
            )
        }));
        lines
    }

    pub(super) fn temporal_fork_detail_lines(&self, index: usize) -> Vec<String> {
        let Some(fork) = self.temporal_forks.get(index) else {
            return vec![String::from("Path not found.")];
        };

        let mut lines = vec![
            format!("Path: {}", fork.label),
            format!("ID: {}", fork.id),
            format!("Reason: {}", fork.reason),
            format!("Saved at: {}", fork.created_at),
            format!(
                "Previous path: {}",
                fork.parent_id.as_deref().unwrap_or("none")
            ),
            format!("Notes: {}", fork.notes.len()),
            format!("Folders: {}", fork.folders.len()),
            format!("Memories: {}", fork.memories.len()),
            String::from("Use /path return <name|id> to branch back into this context."),
        ];

        if let Some(repo) = &fork.repo_context {
            lines.push(String::new());
            lines.push(format!("Repo: {}", repo.cwd));
            lines.push(format!(
                "Branch: {}",
                repo.branch.as_deref().unwrap_or("unknown")
            ));
            lines.push(format!(
                "HEAD: {}",
                repo.head.as_deref().unwrap_or("unknown")
            ));
            if repo.dirty_files.is_empty() {
                lines.push(String::from("Dirty files: none"));
            } else {
                lines.push(format!("Dirty files: {}", repo.dirty_files.len()));
                lines.extend(
                    repo.dirty_files
                        .iter()
                        .take(8)
                        .map(|file| format!("  {}", file)),
                );
            }
        }

        if !fork.activity_context.is_empty() {
            lines.push(String::new());
            lines.push(String::from("Recent activity:"));
            lines.extend(
                fork.activity_context
                    .iter()
                    .rev()
                    .take(5)
                    .map(|entry| format!("[{}] {}", entry.timestamp, entry.label)),
            );
        }

        lines
    }

    pub(super) fn resolve_temporal_fork_index(&self, target: &str) -> Option<usize> {
        let trimmed = target.trim();
        if trimmed.is_empty() {
            return self.temporal_forks.len().checked_sub(1);
        }

        self.temporal_forks
            .iter()
            .position(|fork| fork.id == trimmed || fork.label.eq_ignore_ascii_case(trimmed))
            .or_else(|| {
                let lower = trimmed.to_lowercase();
                self.temporal_forks
                    .iter()
                    .position(|fork| fork.label.to_lowercase().contains(&lower))
            })
    }

    pub(super) fn temporal_fork_cache_path() -> PathBuf {
        if let Ok(path) = std::env::var("ALEPH_FORKS_PATH") {
            return PathBuf::from(path);
        }

        #[cfg(test)]
        {
            if let Ok(dir) = std::env::var("ALEPH_CONFIG_DIR") {
                return PathBuf::from(dir).join(TEMPORAL_FORKS_CONFIG);
            }

            return std::env::temp_dir().join(format!(
                "aleph-test-forks-disabled-{}.json",
                std::process::id()
            ));
        }

        #[cfg(not(test))]
        Self::aleph_config_dir().join(TEMPORAL_FORKS_CONFIG)
    }

    pub(super) fn load_temporal_fork_state() -> Result<(Vec<TemporalFork>, Option<String>), String>
    {
        #[cfg(test)]
        if std::env::var("ALEPH_FORKS_PATH").is_err() && std::env::var("ALEPH_CONFIG_DIR").is_err()
        {
            return Err(String::from(
                "temporal fork cache is disabled for this test",
            ));
        }

        let path = Self::temporal_fork_cache_path();
        if !path.exists() {
            return Ok((Vec::new(), None));
        }

        let body = fs::read_to_string(&path).map_err(|error| {
            format!(
                "failed to read temporal forks '{}': {}",
                path.display(),
                error
            )
        })?;
        let value: serde_json::Value = serde_json::from_str(&body).map_err(|error| {
            format!(
                "failed to parse temporal forks '{}': {}",
                path.display(),
                error
            )
        })?;
        let forks = value
            .get("forks")
            .and_then(|forks| forks.as_array())
            .map(|forks| {
                forks
                    .iter()
                    .filter_map(Self::temporal_fork_from_value)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let current_fork_id = value
            .get("currentForkId")
            .and_then(|id| id.as_str())
            .map(str::to_string)
            .filter(|id| forks.iter().any(|fork| fork.id == id.as_str()));
        Ok((forks, current_fork_id))
    }

    pub(super) fn save_temporal_fork_state(&self) -> Result<(), String> {
        let path = Self::temporal_fork_cache_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                format!(
                    "failed to create temporal fork directory '{}': {}",
                    parent.display(),
                    error
                )
            })?;
        }

        let forks = self
            .temporal_forks
            .iter()
            .map(Self::temporal_fork_to_value)
            .collect::<Vec<_>>();
        let payload = serde_json::json!({
            "version": 1,
            "savedAt": Self::now_millis(),
            "currentForkId": self.current_fork_id.as_deref(),
            "forks": forks,
        });

        fs::write(
            &path,
            serde_json::to_string_pretty(&payload)
                .map_err(|error| format!("failed to encode temporal forks: {}", error))?,
        )
        .map_err(|error| {
            format!(
                "failed to write temporal forks '{}': {}",
                path.display(),
                error
            )
        })
    }

    fn next_temporal_fork_id(&self) -> String {
        let base = Self::now_millis();
        let mut suffix = self.temporal_forks.len() + 1;
        loop {
            let id = format!("fork-{}-{}", base, suffix);
            if self.temporal_forks.iter().all(|fork| fork.id != id) {
                return id;
            }
            suffix += 1;
        }
    }

    fn capture_repo_context() -> Option<RepoContext> {
        let cwd = std::env::current_dir().ok()?;
        let cwd_label = cwd.display().to_string();
        let branch = Self::git_output(&["branch", "--show-current"]);
        let head = Self::git_output(&["rev-parse", "--short", "HEAD"]);
        let dirty_files = Self::git_output(&["status", "--short"])
            .map(|output| output.lines().map(str::to_string).collect::<Vec<_>>())
            .unwrap_or_default();

        if branch.is_none() && head.is_none() && dirty_files.is_empty() {
            return None;
        }

        Some(RepoContext {
            cwd: cwd_label,
            branch,
            head,
            dirty_files,
        })
    }

    fn git_output(args: &[&str]) -> Option<String> {
        let output = Command::new("git").args(args).output().ok()?;
        if !output.status.success() {
            return None;
        }
        let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
        (!value.is_empty()).then_some(value)
    }

    fn temporal_fork_to_value(fork: &TemporalFork) -> serde_json::Value {
        serde_json::json!({
            "id": fork.id,
            "parentId": fork.parent_id,
            "label": fork.label,
            "reason": fork.reason,
            "createdAt": fork.created_at,
            "notes": fork.notes.iter().map(Self::note_to_fork_value).collect::<Vec<_>>(),
            "folders": fork.folders.iter().map(Self::folder_to_fork_value).collect::<Vec<_>>(),
            "memories": fork.memories,
            "selectedNote": fork.selected_note,
            "activityContext": fork.activity_context.iter().map(Self::activity_to_fork_value).collect::<Vec<_>>(),
            "chatContext": fork.chat_context.iter().map(Self::chat_to_fork_value).collect::<Vec<_>>(),
            "repoContext": fork.repo_context.as_ref().map(Self::repo_to_fork_value),
        })
    }

    fn temporal_fork_from_value(value: &serde_json::Value) -> Option<TemporalFork> {
        let notes = value
            .get("notes")
            .and_then(|notes| notes.as_array())
            .map(|notes| {
                notes
                    .iter()
                    .enumerate()
                    .filter_map(|(index, note)| Self::note_from_local_value(index + 1, note))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        Some(TemporalFork {
            id: value.get("id")?.as_str()?.to_string(),
            parent_id: value
                .get("parentId")
                .and_then(|id| id.as_str())
                .map(str::to_string),
            label: value
                .get("label")
                .and_then(|label| label.as_str())
                .unwrap_or("untitled fork")
                .to_string(),
            reason: value
                .get("reason")
                .and_then(|reason| reason.as_str())
                .unwrap_or("manual")
                .to_string(),
            created_at: value
                .get("createdAt")
                .and_then(|created_at| created_at.as_str())
                .unwrap_or("unknown")
                .to_string(),
            notes,
            folders: value
                .get("folders")
                .and_then(|folders| folders.as_array())
                .map(|folders| {
                    folders
                        .iter()
                        .filter_map(Self::folder_from_fork_value)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default(),
            memories: value
                .get("memories")
                .and_then(|memories| memories.as_array())
                .map(|memories| {
                    memories
                        .iter()
                        .filter_map(|memory| memory.as_str().map(str::to_string))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default(),
            selected_note: value
                .get("selectedNote")
                .and_then(|selected| selected.as_u64())
                .map(|selected| selected as usize)
                .unwrap_or(0),
            activity_context: value
                .get("activityContext")
                .and_then(|entries| entries.as_array())
                .map(|entries| {
                    entries
                        .iter()
                        .filter_map(Self::activity_from_fork_value)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default(),
            chat_context: value
                .get("chatContext")
                .and_then(|messages| messages.as_array())
                .map(|messages| {
                    messages
                        .iter()
                        .filter_map(Self::chat_from_fork_value)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default(),
            repo_context: value
                .get("repoContext")
                .and_then(Self::repo_from_fork_value),
        })
    }

    fn note_to_fork_value(note: &Note) -> serde_json::Value {
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
    }

    fn folder_to_fork_value(folder: &Folder) -> serde_json::Value {
        serde_json::json!({
            "id": folder.id,
            "name": folder.name,
            "parentId": folder.parent_id,
        })
    }

    fn folder_from_fork_value(value: &serde_json::Value) -> Option<Folder> {
        Some(Folder {
            id: value.get("id")?.as_u64()? as usize,
            name: value.get("name")?.as_str()?.to_string(),
            parent_id: value
                .get("parentId")
                .and_then(|id| id.as_u64())
                .map(|id| id as usize),
        })
    }

    fn activity_to_fork_value(entry: &ActivityEntry) -> serde_json::Value {
        serde_json::json!({
            "timestamp": entry.timestamp,
            "label": entry.label,
        })
    }

    fn activity_from_fork_value(value: &serde_json::Value) -> Option<ActivityEntry> {
        Some(ActivityEntry {
            timestamp: value.get("timestamp")?.as_str()?.to_string(),
            label: value.get("label")?.as_str()?.to_string(),
        })
    }

    fn chat_to_fork_value(message: &ChatMessage) -> serde_json::Value {
        serde_json::json!({
            "role": message.role,
            "content": message.content,
            "timestamp": message.timestamp,
        })
    }

    fn chat_from_fork_value(value: &serde_json::Value) -> Option<ChatMessage> {
        Some(ChatMessage {
            role: value.get("role")?.as_str()?.to_string(),
            content: value.get("content")?.as_str()?.to_string(),
            timestamp: value.get("timestamp")?.as_str()?.to_string(),
        })
    }

    fn repo_to_fork_value(repo: &RepoContext) -> serde_json::Value {
        serde_json::json!({
            "cwd": repo.cwd,
            "branch": repo.branch,
            "head": repo.head,
            "dirtyFiles": repo.dirty_files,
        })
    }

    fn repo_from_fork_value(value: &serde_json::Value) -> Option<RepoContext> {
        Some(RepoContext {
            cwd: value.get("cwd")?.as_str()?.to_string(),
            branch: value
                .get("branch")
                .and_then(|branch| branch.as_str())
                .map(str::to_string),
            head: value
                .get("head")
                .and_then(|head| head.as_str())
                .map(str::to_string),
            dirty_files: value
                .get("dirtyFiles")
                .and_then(|files| files.as_array())
                .map(|files| {
                    files
                        .iter()
                        .filter_map(|file| file.as_str().map(str::to_string))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default(),
        })
    }
}
