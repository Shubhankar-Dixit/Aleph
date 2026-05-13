use super::*;

const ROOMS_CONFIG: &str = "rooms.json";
const GLOBAL_ROOM_NAME: &str = "All";

fn default_room_fallback() -> &'static Room {
    static DEFAULT_ROOM: std::sync::OnceLock<Room> = std::sync::OnceLock::new();
    DEFAULT_ROOM.get_or_init(App::global_room)
}

#[allow(dead_code)]
impl App {
    pub(super) fn global_room() -> Room {
        Room {
            name: String::from(GLOBAL_ROOM_NAME),
            project_paths: Vec::new(),
            tags: Vec::new(),
            filters: Vec::new(),
            accent: [142, 144, 158],
        }
    }

    pub(super) fn is_global_room(room: &Room) -> bool {
        room.name.eq_ignore_ascii_case(GLOBAL_ROOM_NAME)
    }

    pub(super) fn normalize_room_state(
        mut rooms: Vec<Room>,
        active_room_index: usize,
    ) -> (Vec<Room>, usize) {
        if rooms.is_empty() {
            return Self::default_room_state();
        }

        let selected_name = rooms
            .get(active_room_index)
            .map(|room| room.name.clone())
            .unwrap_or_default();

        rooms.retain(|room| !Self::is_global_room(room));
        rooms.insert(0, Self::global_room());

        let active_room_index =
            if selected_name.is_empty() || selected_name.eq_ignore_ascii_case(GLOBAL_ROOM_NAME) {
                0
            } else {
                rooms
                    .iter()
                    .position(|room| room.name.eq_ignore_ascii_case(&selected_name))
                    .unwrap_or(0)
            };

        (rooms, active_room_index)
    }

    pub(super) fn active_room_ref(&self) -> &Room {
        self.rooms
            .get(self.active_room_index)
            .or_else(|| self.rooms.first())
            .unwrap_or_else(|| default_room_fallback())
    }

    pub(super) fn room_note_indices(&self) -> Vec<usize> {
        self.notes
            .iter()
            .enumerate()
            .filter_map(|(index, note)| self.room_matches_note(note).then_some(index))
            .collect()
    }

    pub(super) fn room_matches_note(&self, note: &Note) -> bool {
        let room = self.active_room_ref();
        if room.project_paths.is_empty() && room.tags.is_empty() && room.filters.is_empty() {
            return true;
        }

        let title = note.title.to_lowercase();
        let content = note.content.to_lowercase();
        let path = note
            .obsidian_path
            .as_ref()
            .map(|value| value.display().to_string().to_lowercase())
            .unwrap_or_default();

        if note.obsidian_path.as_ref().is_some_and(|value| {
            room.project_paths.iter().any(|project_path| {
                let normalized = project_path.to_lowercase();
                !normalized.is_empty()
                    && value
                        .display()
                        .to_string()
                        .to_lowercase()
                        .starts_with(&normalized)
            })
        }) {
            return true;
        }

        room.tags.iter().chain(room.filters.iter()).any(|term| {
            let term = term.to_lowercase();
            !term.is_empty()
                && (title.contains(&term) || content.contains(&term) || path.contains(&term))
        })
    }

    pub(super) fn room_matches_memory(&self, memory: &str) -> bool {
        let room = self.active_room_ref();
        if room.tags.is_empty() && room.filters.is_empty() {
            return true;
        }

        let memory = memory.to_lowercase();
        room.tags.iter().chain(room.filters.iter()).any(|term| {
            let term = term.to_lowercase();
            !term.is_empty() && memory.contains(&term)
        })
    }

    pub(super) fn room_matches_session(&self, fork: &TemporalFork) -> bool {
        let room = self.active_room_ref();
        if room.project_paths.is_empty() && room.tags.is_empty() && room.filters.is_empty() {
            return true;
        }

        let mut haystacks = Vec::new();
        haystacks.push(fork.label.to_lowercase());
        haystacks.push(fork.reason.to_lowercase());
        if let Some(repo) = &fork.repo_context {
            haystacks.push(repo.cwd.to_lowercase());
            if let Some(branch) = &repo.branch {
                haystacks.push(branch.to_lowercase());
            }
            if let Some(head) = &repo.head {
                haystacks.push(head.to_lowercase());
            }
            haystacks.extend(repo.dirty_files.iter().map(|value| value.to_lowercase()));
        }

        let combined = haystacks.join(" ");
        if room
            .project_paths
            .iter()
            .map(|path| path.to_lowercase())
            .any(|path| !path.is_empty() && combined.contains(&path))
        {
            return true;
        }

        room.tags.iter().chain(room.filters.iter()).any(|term| {
            let term = term.to_lowercase();
            !term.is_empty() && combined.contains(&term)
        })
    }

    pub(super) fn room_recent_session_lines(&self, limit: usize) -> Vec<String> {
        let mut lines = self
            .temporal_forks
            .iter()
            .rev()
            .filter(|fork| self.room_matches_session(fork))
            .take(limit)
            .map(|fork| {
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
                format!("{}  {}{}", fork.created_at, fork.label, repo)
            })
            .collect::<Vec<_>>();

        if lines.is_empty() {
            if Self::is_global_room(self.active_room_ref()) {
                lines.push(String::from("No recent sessions yet."));
            } else {
                lines.push(format!(
                    "No recent sessions matched room '{}'.",
                    self.active_room_ref().name
                ));
            }
        }

        lines
    }

    pub(super) fn reconcile_room_selection(&mut self) {
        let visible_note_indices = self.room_note_indices();
        if visible_note_indices.is_empty() {
            self.selected_note = 0;
            self.editor_note_index = None;
            self.editor_buffer.clear();
            self.editor_cursor = 0;
            self.editor_selection.clear();
            return;
        }

        if !visible_note_indices.contains(&self.selected_note) {
            self.selected_note = visible_note_indices[0];
        }

        if let Some(editor_index) = self.editor_note_index {
            if !visible_note_indices.contains(&editor_index) {
                self.editor_note_index = None;
                self.editor_buffer.clear();
                self.editor_cursor = 0;
                self.editor_selection.clear();
            }
        }
    }

    pub(super) fn room_list_panel_lines(&self) -> Vec<String> {
        if self.rooms.is_empty() {
            return vec![String::from("No rooms configured.")];
        }

        self.rooms
            .iter()
            .enumerate()
            .map(|(index, room)| {
                let current = if index == self.active_room_index {
                    "*"
                } else {
                    " "
                };
                format!(
                    "{} {}  paths:{}  tags:{}  filters:{}",
                    current,
                    room.name,
                    room.project_paths.len(),
                    room.tags.len(),
                    room.filters.len()
                )
            })
            .collect()
    }

    pub(super) fn open_room_list_panel(&mut self) {
        self.panel_lines = self.room_list_panel_lines();
        self.room_list_selected = self
            .room_list_selected
            .min(self.rooms.len().saturating_sub(1));
        self.room_list_pending_delete = None;
        self.panel_mode = PanelMode::RoomList;
        self.panel_title = String::from("Rooms (Enter switch, Delete delete)");
    }

    pub(super) fn room_list_lines(&self) -> Vec<String> {
        let mut lines = vec![format!(
            "Active scope: {} ({})",
            self.active_room_ref().name,
            self.room_scope_summary()
        )];

        lines.push(String::new());
        lines.extend(self.room_list_panel_lines());
        lines
    }

    pub(super) fn room_detail_lines(&self, index: usize) -> Result<(String, Vec<String>), String> {
        let Some(room) = self.rooms.get(index) else {
            return Err(String::from("Room not found."));
        };

        let mut lines = vec![format!("Project paths: {}", room.project_paths.len())];
        if room.project_paths.is_empty() {
            lines.push(String::from("- none"));
        } else {
            for path in &room.project_paths {
                lines.push(format!("- {}", path));
            }
        }

        lines.push(format!("Tags: {}", room.tags.len()));
        if room.tags.is_empty() {
            lines.push(String::from("- none"));
        } else {
            for tag in &room.tags {
                lines.push(format!("- {}", tag));
            }
        }

        lines.push(format!("Filters: {}", room.filters.len()));
        if room.filters.is_empty() {
            lines.push(String::from("- none"));
        } else {
            for filter in &room.filters {
                lines.push(format!("- {}", filter));
            }
        }

        lines.push(String::new());
        lines.extend(self.room_recent_session_lines(5));
        Ok((room.name.clone(), lines))
    }

    pub(super) fn resolve_room_index(&self, target: &str) -> Option<usize> {
        let trimmed = target.trim();
        if trimmed.is_empty() {
            return Some(
                self.active_room_index
                    .min(self.rooms.len().saturating_sub(1)),
            );
        }

        let lower = trimmed.to_lowercase();
        if matches!(lower.as_str(), "all" | "none" | "clear" | "global") {
            return self.rooms.iter().position(Self::is_global_room).or(Some(0));
        }

        self.rooms.iter().enumerate().find_map(|(index, room)| {
            if room.name.eq_ignore_ascii_case(trimmed) || room.name.to_lowercase().contains(&lower)
            {
                return Some(index);
            }

            if room
                .project_paths
                .iter()
                .any(|path| path.to_lowercase().contains(&lower))
            {
                return Some(index);
            }

            if room
                .tags
                .iter()
                .any(|tag| tag.to_lowercase().contains(&lower))
                || room
                    .filters
                    .iter()
                    .any(|filter| filter.to_lowercase().contains(&lower))
            {
                return Some(index);
            }

            None
        })
    }

    pub(super) fn switch_room_by_index(&mut self, index: usize) -> Result<Vec<String>, String> {
        if index >= self.rooms.len() {
            return Err(String::from("Room not found."));
        }

        self.active_room_index = index;
        self.room_list_selected = index;
        self.room_list_pending_delete = None;
        self.reconcile_room_selection();
        self.search_state = SearchState::default();
        self.note_list_selected = 0;
        self.note_list_indices.clear();
        self.path_list_selected = 0;
        self.panel_lines.clear();
        self.save_room_state()?;
        self.room_detail_lines(index).map(|(_, lines)| lines)
    }

    pub(super) fn switch_room_by_name(&mut self, target: &str) -> Result<Vec<String>, String> {
        let Some(index) = self.resolve_room_index(target) else {
            return Err(format!("Room '{}' was not found.", target.trim()));
        };

        self.switch_room_by_index(index)
    }

    pub(super) fn cycle_room(&mut self, direction: isize) -> Result<Vec<String>, String> {
        if self.rooms.is_empty() {
            return Err(String::from("No rooms are configured."));
        }

        let len = self.rooms.len() as isize;
        let mut next = self.active_room_index as isize + direction;
        next = ((next % len) + len) % len;
        self.switch_room_by_index(next as usize)
    }

    pub(super) fn delete_room_at_index(&mut self, index: usize) -> Result<String, String> {
        if self.rooms.get(index).is_some_and(Self::is_global_room) {
            return Err(String::from(
                "All is the default scope and cannot be deleted.",
            ));
        }

        if self.rooms.len() <= 1 {
            return Err(String::from("At least one room must remain."));
        }

        let Some(removed) = self.rooms.get(index).cloned() else {
            return Err(String::from("Room not found."));
        };

        let previous_active_room_index = self.active_room_index;
        let previous_room_list_selected = self.room_list_selected;
        let previous_room_list_pending_delete = self.room_list_pending_delete;

        self.rooms.remove(index);

        if self.active_room_index == index {
            self.active_room_index = index.min(self.rooms.len().saturating_sub(1));
        } else if self.active_room_index > index {
            self.active_room_index -= 1;
        }

        if let Err(error) = self.save_room_state() {
            self.rooms.insert(index, removed.clone());
            self.active_room_index = previous_active_room_index;
            self.room_list_selected = previous_room_list_selected;
            self.room_list_pending_delete = previous_room_list_pending_delete;
            return Err(error);
        }

        self.room_list_selected = if previous_room_list_selected > index {
            previous_room_list_selected - 1
        } else {
            previous_room_list_selected.min(self.rooms.len().saturating_sub(1))
        };
        self.room_list_pending_delete = None;
        self.reconcile_room_selection();
        self.search_state = SearchState::default();
        self.note_list_selected = 0;
        self.note_list_indices.clear();
        self.note_list_pending_delete = None;
        self.path_list_selected = 0;
        self.path_list_pending_delete = None;

        Ok(removed.name)
    }

    pub(super) fn room_state_path() -> PathBuf {
        if let Ok(path) = std::env::var("ALEPH_ROOMS_PATH") {
            return PathBuf::from(path);
        }

        #[cfg(test)]
        {
            if let Ok(dir) = std::env::var("ALEPH_CONFIG_DIR") {
                return PathBuf::from(dir).join(ROOMS_CONFIG);
            }

            return std::env::temp_dir().join(format!(
                "aleph-test-rooms-disabled-{}.json",
                std::process::id()
            ));
        }

        #[cfg(not(test))]
        Self::aleph_config_dir().join(ROOMS_CONFIG)
    }

    pub(super) fn default_room_state() -> (Vec<Room>, usize) {
        let workspace = std::env::current_dir()
            .ok()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| String::from("."));

        let rooms = vec![
            Self::global_room(),
            Room {
                name: String::from("aleph-dev"),
                project_paths: vec![workspace.clone()],
                tags: vec![String::from("aleph"), String::from("terminal")],
                filters: vec![String::from("agent"), String::from("obsidian")],
                accent: [136, 129, 176],
            },
            Room {
                name: String::from("strix-core"),
                project_paths: vec![workspace],
                tags: vec![String::from("strix"), String::from("gateway")],
                filters: vec![String::from("memory"), String::from("session")],
                accent: [106, 163, 178],
            },
        ];

        (rooms, 0)
    }

    pub(super) fn load_room_state() -> Result<(Vec<Room>, usize), String> {
        let path = Self::room_state_path();
        if !path.exists() {
            return Ok(Self::default_room_state());
        }

        let body = fs::read_to_string(&path)
            .map_err(|error| format!("failed to read rooms '{}': {}", path.display(), error))?;
        let value: serde_json::Value = serde_json::from_str(&body)
            .map_err(|error| format!("failed to parse rooms '{}': {}", path.display(), error))?;

        let rooms = value
            .get("rooms")
            .and_then(|rooms| rooms.as_array())
            .map(|rooms| {
                rooms
                    .iter()
                    .filter_map(Self::room_from_value)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        if rooms.is_empty() {
            return Ok(Self::default_room_state());
        }

        let selected_name = value
            .get("activeRoom")
            .and_then(|entry| entry.as_str())
            .unwrap_or_default();
        let active_room_index = if selected_name.is_empty() {
            usize::MAX
        } else {
            rooms
                .iter()
                .position(|room| room.name.eq_ignore_ascii_case(selected_name))
                .unwrap_or(usize::MAX)
        };

        Ok(Self::normalize_room_state(rooms, active_room_index))
    }

    pub(super) fn save_room_state(&self) -> Result<(), String> {
        let path = Self::room_state_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                format!(
                    "failed to create room directory '{}': {}",
                    parent.display(),
                    error
                )
            })?;
        }

        let payload = serde_json::json!({
            "version": 1,
            "activeRoom": self.active_room_ref().name,
            "rooms": self.rooms.iter().map(Self::room_to_value).collect::<Vec<_>>(),
        });

        fs::write(
            &path,
            serde_json::to_string_pretty(&payload)
                .map_err(|error| format!("failed to encode rooms: {}", error))?,
        )
        .map_err(|error| format!("failed to write rooms '{}': {}", path.display(), error))
    }

    fn room_to_value(room: &Room) -> serde_json::Value {
        serde_json::json!({
            "name": room.name,
            "projectPaths": room.project_paths,
            "tags": room.tags,
            "filters": room.filters,
            "accent": room.accent,
        })
    }

    fn room_from_value(value: &serde_json::Value) -> Option<Room> {
        let name = value.get("name")?.as_str()?.trim();
        if name.is_empty() {
            return None;
        }

        let project_paths = value
            .get("projectPaths")
            .and_then(|paths| paths.as_array())
            .map(|paths| {
                paths
                    .iter()
                    .filter_map(|entry| entry.as_str().map(str::to_string))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let tags = value
            .get("tags")
            .and_then(|entries| entries.as_array())
            .map(|entries| {
                entries
                    .iter()
                    .filter_map(|entry| entry.as_str().map(str::to_string))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let filters = value
            .get("filters")
            .and_then(|entries| entries.as_array())
            .map(|entries| {
                entries
                    .iter()
                    .filter_map(|entry| entry.as_str().map(str::to_string))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let accent = value
            .get("accent")
            .and_then(|accent| accent.as_array())
            .and_then(|accent| {
                if accent.len() != 3 {
                    return None;
                }

                let red = accent[0].as_u64()?.min(255) as u8;
                let green = accent[1].as_u64()?.min(255) as u8;
                let blue = accent[2].as_u64()?.min(255) as u8;
                Some([red, green, blue])
            })
            .unwrap_or([136, 129, 176]);

        Some(Room {
            name: name.to_string(),
            project_paths,
            tags,
            filters,
            accent,
        })
    }
}
