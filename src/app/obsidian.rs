use super::*;

#[allow(dead_code)]
impl App {
    pub(super) fn handle_obsidian_pair(&mut self, target: &str) {
        self.refresh_obsidian_vaults();
        if target.is_empty() {
            if self.obsidian_vaults.len() == 1 {
                let path = self.obsidian_vaults[0].path.clone();
                match self.pair_obsidian_vault(path) {
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
            } else {
                self.open_vault_picker();
            }
            return;
        }

        let target_path = self
            .resolve_obsidian_vault_target(target)
            .unwrap_or_else(|| PathBuf::from(Self::expand_home(target)));

        match self.pair_obsidian_vault(target_path) {
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

    pub(super) fn open_vault_picker(&mut self) {
        self.refresh_obsidian_vaults();
        self.panel_mode = PanelMode::VaultPicker;
        self.panel_title = String::from("Obsidian pairing");
        self.panel_lines.clear();
        self.obsidian_vault_selected = 0;
        self.last_action = if self.obsidian_vaults.is_empty() {
            String::from("No Obsidian vaults found; type /obsidian pair <path>.")
        } else {
            String::from("Choose an Obsidian vault.")
        };
    }

    pub(super) fn pair_obsidian_vault(&mut self, path: PathBuf) -> Result<String, String> {
        let canonical = fs::canonicalize(&path).map_err(|error| {
            format!("failed to open vault path '{}': {}", path.display(), error)
        })?;
        if !canonical.is_dir() {
            return Err(format!("'{}' is not a directory.", canonical.display()));
        }

        self.store_obsidian_vault_path(&canonical)?;
        self.obsidian_vault_path = Some(canonical.clone());
        if self.note_save_target == NoteSaveTarget::Local {
            self.note_save_target = NoteSaveTarget::Obsidian;
            self.store_note_save_target()?;
        }
        if !self
            .obsidian_vaults
            .iter()
            .any(|vault| vault.path == canonical)
        {
            self.obsidian_vaults.push(ObsidianVault {
                id: Self::stable_vault_id(&canonical),
                name: Self::vault_display_name(&canonical),
                path: canonical.clone(),
                source: String::from("manual"),
            });
        }
        Ok(format!("Paired vault: {}", canonical.display()))
    }

    pub(super) fn sync_obsidian_notes(&mut self) -> Result<usize, String> {
        let vault_path = self.obsidian_vault_path.clone().ok_or_else(|| {
            String::from("No Obsidian vault is paired. Run /obsidian pair first.")
        })?;
        let files = Self::collect_markdown_files(&vault_path)?;
        let folder_root_id = self.ensure_folder_path(&[String::from("Obsidian")], None);
        let vault_name = Self::vault_display_name(&vault_path);
        let vault_folder_id = self.ensure_folder_path(&[vault_name], Some(folder_root_id));
        // Ensure the Obsidian root folder is expanded
        if !self.expanded_folders.contains(&folder_root_id) {
            self.expanded_folders.push(folder_root_id);
        }
        // Ensure the vault folder is expanded
        if !self.expanded_folders.contains(&vault_folder_id) {
            self.expanded_folders.push(vault_folder_id);
        }
        let mut imported = 0;

        for file in files {
            let relative = file.strip_prefix(&vault_path).unwrap_or(file.as_path());
            let title = Self::obsidian_note_title(&file, relative);
            let content = fs::read_to_string(&file)
                .map_err(|error| format!("failed to read '{}': {}", file.display(), error))?;
            let folder_id = self.obsidian_folder_id(relative, vault_folder_id);
            let updated_at = fs::metadata(&file)
                .and_then(|metadata| metadata.modified())
                .ok()
                .and_then(|modified| modified.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|duration| format!("obsidian:{}", duration.as_secs()))
                .unwrap_or_else(|| String::from("obsidian"));
            self.upsert_obsidian_note(file, title, content, updated_at, folder_id);
            imported += 1;
        }

        if imported > 0 {
            self.selected_note = self.selected_note.min(self.notes.len().saturating_sub(1));
        }
        Self::save_local_notes(&self.notes)?;
        Ok(imported)
    }

    pub(super) fn rebuild_obsidian_folders_from_cached_notes(&mut self) {
        let Some(vault_path) = self.obsidian_vault_path.clone() else {
            return;
        };
        if !self.folders.is_empty() {
            return;
        }

        let obsidian_notes = self
            .notes
            .iter()
            .enumerate()
            .filter_map(|(index, note)| {
                note.obsidian_path
                    .as_ref()
                    .map(|path| (index, path.clone()))
            })
            .collect::<Vec<_>>();
        if obsidian_notes.is_empty() {
            return;
        }

        let folder_root_id = self.ensure_folder_path(&[String::from("Obsidian")], None);
        let vault_name = Self::vault_display_name(&vault_path);
        let vault_folder_id = self.ensure_folder_path(&[vault_name], Some(folder_root_id));
        if !self.expanded_folders.contains(&folder_root_id) {
            self.expanded_folders.push(folder_root_id);
        }
        if !self.expanded_folders.contains(&vault_folder_id) {
            self.expanded_folders.push(vault_folder_id);
        }

        for (index, path) in obsidian_notes {
            let relative = path.strip_prefix(&vault_path).unwrap_or(path.as_path());
            let folder_id = self.obsidian_folder_id(relative, vault_folder_id);
            if let Some(note) = self.notes.get_mut(index) {
                note.folder_id = folder_id;
            }
        }
    }

    pub(super) fn upsert_obsidian_note(
        &mut self,
        path: PathBuf,
        title: String,
        content: String,
        updated_at: String,
        folder_id: Option<usize>,
    ) {
        if let Some((index, note)) = self
            .notes
            .iter_mut()
            .enumerate()
            .find(|(_, note)| note.obsidian_path.as_deref() == Some(path.as_path()))
        {
            note.title = title;
            note.content = content.clone();
            note.raw_content = content;
            note.updated_at = updated_at;
            note.folder_id = folder_id;
            self.selected_note = index;
            return;
        }

        let id = self.notes.iter().map(|note| note.id).max().unwrap_or(0) + 1;
        self.notes.push(Note {
            id,
            remote_id: None,
            obsidian_path: Some(path),
            title,
            content: content.clone(),
            raw_content: content,
            updated_at,
            folder_id,
        });
        self.selected_note = self.notes.len() - 1;
    }

    pub(super) fn obsidian_folder_id(
        &mut self,
        relative: &Path,
        vault_folder_id: usize,
    ) -> Option<usize> {
        let Some(parent) = relative.parent() else {
            return Some(vault_folder_id);
        };
        let parts = parent
            .components()
            .filter_map(|component| match component {
                std::path::Component::Normal(part) => part.to_str().map(|part| part.to_string()),
                _ => None,
            })
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>();
        if parts.is_empty() {
            Some(vault_folder_id)
        } else {
            Some(self.ensure_folder_path(&parts, Some(vault_folder_id)))
        }
    }

    pub(super) fn ensure_folder_path(
        &mut self,
        parts: &[String],
        parent_id: Option<usize>,
    ) -> usize {
        let mut parent = parent_id;
        let mut last_id = parent_id.unwrap_or(0);
        for part in parts {
            if let Some(existing) = self
                .folders
                .iter()
                .find(|folder| folder.parent_id == parent && folder.name == *part)
                .map(|folder| folder.id)
            {
                last_id = existing;
                parent = Some(existing);
                // Expand existing folders in the path
                if !self.expanded_folders.contains(&existing) {
                    self.expanded_folders.push(existing);
                }
                continue;
            }
            let id = self
                .folders
                .iter()
                .map(|folder| folder.id)
                .max()
                .unwrap_or(0)
                + 1;
            self.folders.push(Folder {
                id,
                name: part.clone(),
                parent_id: parent,
            });
            // Auto-expand newly created folders
            self.expanded_folders.push(id);
            last_id = id;
            parent = Some(id);
        }
        last_id
    }

    pub(super) fn open_obsidian_target(&mut self, target: &str) -> Result<String, String> {
        let vault_path = self.obsidian_vault_path.as_ref().ok_or_else(|| {
            String::from("No Obsidian vault is paired. Run /obsidian pair first.")
        })?;
        let vault_name = Self::vault_display_name(vault_path);
        let target_note = if target.is_empty() {
            self.active_note()
        } else {
            self.resolve_note_index(target)
                .and_then(|index| self.notes.get(index))
        };

        let uri = if let Some(note) = target_note {
            if let Some(path) = note.obsidian_path.as_ref() {
                let file = path
                    .strip_prefix(vault_path)
                    .unwrap_or(path.as_path())
                    .to_string_lossy()
                    .replace('\\', "/");
                format!(
                    "obsidian://open?vault={}&file={}",
                    urlencoding::encode(&vault_name),
                    urlencoding::encode(file.trim_end_matches(".md"))
                )
            } else {
                format!(
                    "obsidian://new?vault={}&name={}&content={}",
                    urlencoding::encode(&vault_name),
                    urlencoding::encode(&note.title),
                    urlencoding::encode(&note.content)
                )
            }
        } else if target.is_empty() {
            format!("obsidian://open?vault={}", urlencoding::encode(&vault_name))
        } else {
            format!(
                "obsidian://open?vault={}&file={}",
                urlencoding::encode(&vault_name),
                urlencoding::encode(target)
            )
        };

        Self::open_browser(&uri)?;
        Ok(format!("Sent Obsidian URI: {}", uri))
    }

    pub(super) fn obsidian_status_label(&self) -> String {
        self.obsidian_vault_path
            .as_ref()
            .map(|path| format!("paired ({})", Self::vault_display_name(path)))
            .unwrap_or_else(|| String::from("not paired"))
    }

    pub(super) fn refresh_obsidian_vaults(&mut self) {
        self.obsidian_vaults = Self::discover_obsidian_vaults();
        if let Some(path) = self.obsidian_vault_path.clone() {
            if !self.obsidian_vaults.iter().any(|vault| vault.path == path) {
                self.obsidian_vaults.push(ObsidianVault {
                    id: Self::stable_vault_id(&path),
                    name: Self::vault_display_name(&path),
                    path,
                    source: String::from("paired"),
                });
            }
        }
    }

    pub(super) fn format_obsidian_vault_lines(&self) -> Vec<String> {
        self.obsidian_vaults
            .iter()
            .enumerate()
            .map(|(index, vault)| {
                let paired = if self.obsidian_vault_path.as_deref() == Some(vault.path.as_path()) {
                    " paired"
                } else {
                    ""
                };
                format!(
                    "{:>2}. {} — {} [{}]{}",
                    index + 1,
                    vault.name,
                    vault.path.display(),
                    vault.source,
                    paired
                )
            })
            .collect()
    }

    pub(super) fn resolve_obsidian_vault_target(&self, target: &str) -> Option<PathBuf> {
        if let Ok(index) = target.parse::<usize>() {
            return self
                .obsidian_vaults
                .get(index.saturating_sub(1))
                .map(|vault| vault.path.clone());
        }

        let lowered = target.to_lowercase();
        self.obsidian_vaults
            .iter()
            .find(|vault| vault.name.to_lowercase() == lowered || vault.id == target)
            .map(|vault| vault.path.clone())
    }

    pub(super) fn collect_markdown_files(root: &Path) -> Result<Vec<PathBuf>, String> {
        let mut files = Vec::new();
        Self::collect_markdown_files_inner(root, &mut files)?;
        files.sort();
        Ok(files)
    }

    pub(super) fn collect_markdown_files_inner(
        dir: &Path,
        files: &mut Vec<PathBuf>,
    ) -> Result<(), String> {
        let entries = fs::read_dir(dir)
            .map_err(|error| format!("failed to read '{}': {}", dir.display(), error))?;
        for entry in entries {
            let entry = entry.map_err(|error| format!("failed to read vault entry: {}", error))?;
            let path = entry.path();
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default();
            if name.starts_with('.') || name == "node_modules" || name == "target" {
                continue;
            }
            if path.is_dir() {
                Self::collect_markdown_files_inner(&path, files)?;
            } else if path
                .extension()
                .and_then(|extension| extension.to_str())
                .map_or(false, |extension| extension.eq_ignore_ascii_case("md"))
            {
                files.push(path);
            }
        }
        Ok(())
    }

    pub(super) fn obsidian_note_title(path: &Path, relative: &Path) -> String {
        fs::read_to_string(path)
            .ok()
            .and_then(|content| {
                let mut in_frontmatter = false;
                let mut is_first_line = true;

                for line in content.lines() {
                    if is_first_line {
                        is_first_line = false;
                        if line.trim() == "---" {
                            in_frontmatter = true;
                            continue;
                        }
                    }

                    if in_frontmatter {
                        if line.trim() == "---" {
                            in_frontmatter = false;
                        }
                        continue;
                    }

                    if let Some(title) = line.strip_prefix("# ") {
                        let trimmed = title.trim();
                        if !trimmed.is_empty() {
                            return Some(trimmed.to_string());
                        }
                    }
                }
                None
            })
            .or_else(|| {
                relative
                    .file_stem()
                    .and_then(|stem| stem.to_str())
                    .map(|stem| stem.to_string())
            })
            .unwrap_or_else(|| String::from("Untitled Obsidian note"))
    }

    pub(super) fn obsidian_note_path_for_title(&self, title: &str) -> Option<PathBuf> {
        let vault_path = self.obsidian_vault_path.as_ref()?;
        let filename = Self::safe_obsidian_filename(title);
        let mut path = vault_path.join(format!("{}.md", filename));
        if !path.exists() {
            return Some(path);
        }

        for suffix in 2..1000 {
            path = vault_path.join(format!("{} {}.md", filename, suffix));
            if !path.exists() {
                return Some(path);
            }
        }
        Some(vault_path.join(format!("{} {}.md", filename, Self::now_millis())))
    }

    pub(super) fn safe_obsidian_filename(title: &str) -> String {
        let cleaned = title
            .chars()
            .map(|character| match character {
                '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '-',
                character if character.is_control() => ' ',
                character => character,
            })
            .collect::<String>()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        let cleaned = cleaned
            .trim_matches(|character| character == '.' || character == ' ')
            .trim();
        if cleaned.is_empty() {
            String::from("Untitled note")
        } else {
            cleaned.chars().take(120).collect()
        }
    }

    pub(super) fn discover_obsidian_vaults() -> Vec<ObsidianVault> {
        let mut vaults = Vec::new();
        if let Some(path) = Self::load_obsidian_vault_path() {
            vaults.push(ObsidianVault {
                id: Self::stable_vault_id(&path),
                name: Self::vault_display_name(&path),
                path,
                source: String::from("paired"),
            });
        }
        if let Ok(config) = fs::read_to_string(Self::obsidian_config_path()) {
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(&config) {
                if let Some(map) = value.get("vaults").and_then(|vaults| vaults.as_object()) {
                    for (id, vault) in map {
                        let Some(path) = vault.get("path").and_then(|path| path.as_str()) else {
                            continue;
                        };
                        let path = PathBuf::from(Self::expand_home(path));
                        if !path.is_dir() {
                            continue;
                        }
                        vaults.push(ObsidianVault {
                            id: id.clone(),
                            name: Self::vault_display_name(&path),
                            path,
                            source: String::from("Obsidian desktop"),
                        });
                    }
                }
            }
        }
        vaults.sort_by(|left, right| left.name.to_lowercase().cmp(&right.name.to_lowercase()));
        vaults.dedup_by(|left, right| left.path == right.path);
        vaults
    }

    pub(super) fn obsidian_config_path() -> PathBuf {
        if let Ok(path) = std::env::var("OBSIDIAN_CONFIG_PATH") {
            return PathBuf::from(Self::expand_home(&path));
        }
        if cfg!(target_os = "windows") {
            if let Ok(appdata) = std::env::var("APPDATA") {
                return PathBuf::from(appdata)
                    .join("obsidian")
                    .join("obsidian.json");
            }
        }
        if cfg!(target_os = "macos") {
            if let Ok(home) = std::env::var("HOME") {
                return PathBuf::from(home)
                    .join("Library")
                    .join("Application Support")
                    .join("obsidian")
                    .join("obsidian.json");
            }
        }
        if let Ok(config_home) = std::env::var("XDG_CONFIG_HOME") {
            return PathBuf::from(config_home)
                .join("obsidian")
                .join("obsidian.json");
        }
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home)
                .join(".config")
                .join("obsidian")
                .join("obsidian.json");
        }
        PathBuf::from("obsidian.json")
    }

    pub(super) fn load_obsidian_vault_path() -> Option<PathBuf> {
        if Self::obsidian_pairing_disabled_path().exists() {
            return None;
        }

        if let Ok(entry) = Self::obsidian_vault_entry() {
            if let Ok(path) = entry.get_password() {
                let path = PathBuf::from(Self::expand_home(path.trim()));
                if path.is_dir() {
                    return Some(path);
                }
            }
        }
        if let Ok(path) = fs::read_to_string(Self::obsidian_pairing_path()) {
            let path = PathBuf::from(Self::expand_home(path.trim()));
            if path.is_dir() {
                return Some(path);
            }
        }
        None
    }

    pub(super) fn store_obsidian_vault_path(&self, path: &Path) -> Result<(), String> {
        let _ = fs::remove_file(Self::obsidian_pairing_disabled_path());

        if let Ok(entry) = Self::obsidian_vault_entry() {
            if entry.set_password(&path.display().to_string()).is_ok() {
                return Ok(());
            }
        }

        let pairing_path = Self::obsidian_pairing_path();
        if let Some(parent) = pairing_path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                format!(
                    "failed to create Obsidian pairing directory '{}': {}",
                    parent.display(),
                    error
                )
            })?;
        }
        fs::write(&pairing_path, path.display().to_string()).map_err(|error| {
            format!(
                "failed to save Obsidian vault pairing '{}': {}",
                pairing_path.display(),
                error
            )
        })
    }

    pub(super) fn clear_obsidian_vault_path(&self) {
        if let Ok(entry) = Self::obsidian_vault_entry() {
            let _ = entry.delete_credential();
        }
        let _ = fs::remove_file(Self::obsidian_pairing_path());
        let disabled_path = Self::obsidian_pairing_disabled_path();
        if let Some(parent) = disabled_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let _ = fs::write(disabled_path, "unpaired");
    }

    pub(super) fn obsidian_vault_entry() -> Result<Entry, String> {
        Entry::new(OBSIDIAN_SERVICE, OBSIDIAN_ACCOUNT)
            .map_err(|error| format!("failed to open Obsidian credential store: {}", error))
    }

    pub(super) fn note_save_target_name(target: NoteSaveTarget) -> &'static str {
        match target {
            NoteSaveTarget::Local => "Local",
            NoteSaveTarget::Obsidian => "Obsidian",
            NoteSaveTarget::Strix => "Strix",
        }
    }

    pub(super) fn note_save_target_config_value(target: NoteSaveTarget) -> &'static str {
        match target {
            NoteSaveTarget::Local => "local",
            NoteSaveTarget::Obsidian => "obsidian",
            NoteSaveTarget::Strix => "strix",
        }
    }

    pub(super) fn parse_note_save_target(value: &str) -> Option<NoteSaveTarget> {
        match value.trim().to_ascii_lowercase().as_str() {
            "local" => Some(NoteSaveTarget::Local),
            "obsidian" => Some(NoteSaveTarget::Obsidian),
            "strix" => Some(NoteSaveTarget::Strix),
            _ => None,
        }
    }

    pub(super) fn note_save_target_is_available(
        target: NoteSaveTarget,
        has_obsidian_vault: bool,
        has_strix_token: bool,
    ) -> bool {
        match target {
            NoteSaveTarget::Local => true,
            NoteSaveTarget::Obsidian => has_obsidian_vault,
            NoteSaveTarget::Strix => has_strix_token,
        }
    }

    pub(super) fn ai_provider_config_value(provider: AiProvider) -> &'static str {
        match provider {
            AiProvider::OpenRouter => "openrouter",
            AiProvider::Strix => "strix",
        }
    }

    pub(super) fn parse_ai_provider(value: &str) -> Option<AiProvider> {
        match value.trim().to_ascii_lowercase().as_str() {
            "openrouter" => Some(AiProvider::OpenRouter),
            "strix" => Some(AiProvider::Strix),
            _ => None,
        }
    }

    pub(super) fn load_ai_provider() -> Option<AiProvider> {
        fs::read_to_string(Self::ai_provider_path())
            .ok()
            .and_then(|value| Self::parse_ai_provider(&value))
    }

    pub(super) fn store_ai_provider(&self) -> Result<(), String> {
        Self::write_config_value(
            Self::ai_provider_path(),
            Self::ai_provider_config_value(self.ai_provider),
            "AI provider setting",
        )
    }

    pub(super) fn load_agent_mode_enabled() -> Option<bool> {
        match fs::read_to_string(Self::agent_mode_path())
            .ok()?
            .trim()
            .to_ascii_lowercase()
            .as_str()
        {
            "true" | "agent" | "enabled" | "1" => Some(true),
            "false" | "chat" | "disabled" | "0" => Some(false),
            _ => None,
        }
    }

    pub(super) fn store_agent_mode_enabled(&self) -> Result<(), String> {
        Self::write_config_value(
            Self::agent_mode_path(),
            if self.agent_mode_enabled {
                "agent"
            } else {
                "chat"
            },
            "agent mode setting",
        )
    }

    pub(super) fn load_editor_images_enabled() -> Option<bool> {
        match fs::read_to_string(Self::editor_images_path())
            .ok()?
            .trim()
            .to_ascii_lowercase()
            .as_str()
        {
            "true" | "enabled" | "on" | "1" => Some(true),
            "false" | "disabled" | "off" | "0" => Some(false),
            _ => None,
        }
    }

    pub(super) fn store_editor_images_enabled(&self) -> Result<(), String> {
        Self::write_config_value(
            Self::editor_images_path(),
            if self.editor_images_enabled {
                "enabled"
            } else {
                "disabled"
            },
            "editor image setting",
        )
    }

    pub(super) fn load_note_save_target() -> Option<NoteSaveTarget> {
        fs::read_to_string(Self::note_save_target_path())
            .ok()
            .and_then(|value| Self::parse_note_save_target(&value))
    }

    pub(super) fn store_note_save_target(&self) -> Result<(), String> {
        Self::write_config_value(
            Self::note_save_target_path(),
            Self::note_save_target_config_value(self.note_save_target),
            "note storage setting",
        )
    }

    pub(super) fn write_config_value(
        config_path: PathBuf,
        value: &'static str,
        setting_name: &str,
    ) -> Result<(), String> {
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                format!(
                    "failed to create settings directory '{}': {}",
                    parent.display(),
                    error
                )
            })?;
        }
        fs::write(&config_path, value).map_err(|error| {
            format!(
                "failed to save {} '{}': {}",
                setting_name,
                config_path.display(),
                error
            )
        })
    }

    pub(super) fn note_save_target_path() -> PathBuf {
        Self::aleph_config_dir().join(NOTE_SAVE_TARGET_CONFIG)
    }

    pub(super) fn ai_provider_path() -> PathBuf {
        Self::aleph_config_dir().join(AI_PROVIDER_CONFIG)
    }

    pub(super) fn agent_mode_path() -> PathBuf {
        Self::aleph_config_dir().join(AGENT_MODE_CONFIG)
    }

    pub(super) fn editor_images_path() -> PathBuf {
        Self::aleph_config_dir().join(EDITOR_IMAGES_CONFIG)
    }

    pub(super) fn aleph_config_dir() -> PathBuf {
        if let Ok(dir) = std::env::var("ALEPH_CONFIG_DIR") {
            return PathBuf::from(dir);
        }
        #[cfg(test)]
        {
            std::env::temp_dir().join(format!("aleph-test-config-{}", std::process::id()))
        }
        #[cfg(not(test))]
        {
            if let Ok(dir) = std::env::var("XDG_CONFIG_HOME") {
                return PathBuf::from(dir).join("aleph");
            }
            if let Ok(dir) = std::env::var("LOCALAPPDATA").or_else(|_| std::env::var("APPDATA")) {
                return PathBuf::from(dir).join("Aleph");
            }
            if let Ok(home) = std::env::var("HOME") {
                return PathBuf::from(home).join(".config").join("aleph");
            }
            std::env::temp_dir().join("aleph")
        }
    }

    pub(super) fn obsidian_pairing_path() -> PathBuf {
        if let Ok(dir) = std::env::var("ALEPH_CONFIG_DIR") {
            return PathBuf::from(dir).join("obsidian-vault");
        }
        if let Ok(dir) = std::env::var("XDG_CONFIG_HOME") {
            return PathBuf::from(dir).join("aleph").join("obsidian-vault");
        }
        if let Ok(dir) = std::env::var("LOCALAPPDATA").or_else(|_| std::env::var("APPDATA")) {
            return PathBuf::from(dir).join("Aleph").join("obsidian-vault");
        }
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home)
                .join(".config")
                .join("aleph")
                .join("obsidian-vault");
        }
        std::env::temp_dir().join("aleph-obsidian-vault")
    }

    pub(super) fn obsidian_pairing_disabled_path() -> PathBuf {
        Self::aleph_config_dir().join(OBSIDIAN_PAIRING_DISABLED_CONFIG)
    }

    pub(super) fn vault_display_name(path: &Path) -> String {
        path.file_name()
            .and_then(|name| name.to_str())
            .map(|name| name.to_string())
            .unwrap_or_else(|| path.display().to_string())
    }

    pub(super) fn stable_vault_id(path: &Path) -> String {
        let mut hasher = Sha256::new();
        hasher.update(path.display().to_string().as_bytes());
        let digest = hasher.finalize();
        URL_SAFE_NO_PAD.encode(&digest[..9])
    }

    pub(super) fn expand_home(path: &str) -> String {
        if path == "~" {
            return std::env::var("HOME").unwrap_or_else(|_| path.to_string());
        }
        if let Some(rest) = path.strip_prefix("~/") {
            if let Ok(home) = std::env::var("HOME") {
                return format!("{}/{}", home, rest);
            }
        }
        path.to_string()
    }
}
