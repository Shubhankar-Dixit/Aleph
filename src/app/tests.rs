use super::*;
use crossterm::event::KeyEventState;

fn press(code: KeyCode) -> KeyEvent {
    KeyEvent {
        code,
        modifiers: KeyModifiers::NONE,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    }
}

fn repeat(code: KeyCode) -> KeyEvent {
    KeyEvent {
        code,
        modifiers: KeyModifiers::NONE,
        kind: KeyEventKind::Repeat,
        state: KeyEventState::NONE,
    }
}

fn ctrl(code: KeyCode) -> KeyEvent {
    KeyEvent {
        code,
        modifiers: KeyModifiers::CONTROL,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    }
}

fn env_lock() -> std::sync::MutexGuard<'static, ()> {
    static ENV_LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
    ENV_LOCK
        .get_or_init(|| std::sync::Mutex::new(()))
        .lock()
        .unwrap()
}

fn test_note(id: usize, remote_id: Option<&str>, title: &str, content: &str) -> Note {
    Note {
        id,
        remote_id: remote_id.map(String::from),
        obsidian_path: None,
        title: title.to_string(),
        content: content.to_string(),
        raw_content: String::new(),
        updated_at: String::new(),
        folder_id: None,
    }
}

fn seed_test_notes(app: &mut App) {
    app.notes = vec![
        test_note(1, None, "Strix gateway", "gateway notes"),
        test_note(2, None, "Note editor", "editor notes"),
        test_note(3, None, "MCP server", "server notes"),
        test_note(4, None, "Feature ideas", "feature notes"),
    ];
    app.selected_note = 0;
}

#[test]
fn repeated_character_events_do_not_duplicate_input() {
    let mut app = App::new();

    app.handle_key(press(KeyCode::Char('a')));
    app.handle_key(repeat(KeyCode::Char('a')));

    assert_eq!(app.prompt(), "a");
}

#[test]
fn unpaired_app_starts_with_single_onboarding_note() {
    let app = App::new();

    assert_eq!(app.notes.len(), 1);
    assert!(app.folders.is_empty());
    assert_eq!(app.notes[0].title, "Welcome to Aleph");
    assert_eq!(app.notes[0].folder_id, None);
    assert!(app.notes[0].content.contains("/settings"));
    assert!(app.notes[0].content.contains("/obsidian pair"));
}

#[test]
fn unpaired_note_list_shows_onboarding_note_directly() {
    let mut app = App::new();

    app.open_note_list_panel();

    assert_eq!(app.panel_lines.len(), 1);
    assert!(app.panel_lines[0].contains("Welcome"));
    assert!(!app.panel_lines[0].contains("Projects"));
    assert!(!app.panel_lines[0].contains("Ideas"));
}

#[test]
fn clear_notes_is_hidden_from_command_list() {
    assert!(COMMANDS.iter().all(|command| command.name != "clear-notes"));
}

#[test]
fn hidden_clear_notes_resets_note_state_and_caches() {
    let _guard = env_lock();
    let root = std::env::temp_dir().join(format!("aleph-clear-notes-test-{}", App::now_millis()));
    let config_dir = root.join("config");
    let cache_dir = root.join("cache");
    fs::create_dir_all(&config_dir).unwrap();
    fs::create_dir_all(&cache_dir).unwrap();
    std::env::set_var("ALEPH_CONFIG_DIR", &config_dir);
    std::env::set_var("ALEPH_CACHE_DIR", &cache_dir);

    App::save_local_notes(&[test_note(1, None, "Draft", "body")]).unwrap();
    fs::write(App::strix_cache_path(), "{\"version\":1,\"notes\":[]}").unwrap();
    fs::write(App::obsidian_pairing_path(), "/tmp").unwrap();

    let mut app = App::new();
    app.notes = vec![test_note(1, None, "Draft", "body")];
    app.folders.push(Folder {
        id: 1,
        name: String::from("Imported"),
        parent_id: None,
    });
    app.obsidian_vault_path = Some(PathBuf::from("/tmp"));
    app.note_save_target = NoteSaveTarget::Obsidian;
    app.prompt = String::from("/clear-notes");
    app.cursor = app.prompt.len();

    app.submit_prompt();

    assert_eq!(app.notes.len(), 1);
    assert_eq!(app.notes[0].title, "Welcome to Aleph");
    assert!(app.folders.is_empty());
    assert!(app.obsidian_vault_path().is_none());
    assert_eq!(app.note_save_target, NoteSaveTarget::Local);
    assert!(!App::local_notes_path().exists());
    assert!(!App::strix_cache_path().exists());
    assert!(App::obsidian_pairing_disabled_path().exists());

    std::env::remove_var("ALEPH_CONFIG_DIR");
    std::env::remove_var("ALEPH_CACHE_DIR");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn repeated_arrow_keys_move_the_cursor() {
    let mut app = App::new();

    app.handle_key(press(KeyCode::Char('a')));
    app.handle_key(press(KeyCode::Char('b')));
    app.handle_key(repeat(KeyCode::Left));

    assert_eq!(app.prompt_before_cursor(), "a");
    assert_eq!(app.prompt_after_cursor(), "b");
}

#[test]
fn up_and_down_cycle_suggestions() {
    let mut app = App::new();

    app.handle_key(press(KeyCode::Char('/')));

    let first = app.visible_commands(16)[0].name;
    let second = app.visible_commands(16)[1].name;

    assert_eq!(first, "login");
    assert_eq!(second, "status");
    assert_eq!(app.prompt(), "/");

    app.handle_key(repeat(KeyCode::Down));
    assert_eq!(app.selected_suggestion(), 1);
    assert_eq!(app.prompt(), "/status");

    app.handle_key(repeat(KeyCode::Up));
    assert_eq!(app.selected_suggestion(), 0);
    assert_eq!(app.prompt(), "/login");
}

#[test]
fn enter_executes_typed_command() {
    let mut app = App::new();

    for character in "/status".chars() {
        app.handle_key(press(KeyCode::Char(character)));
    }
    app.handle_key(press(KeyCode::Enter));

    assert_eq!(app.last_action(), "Refreshed provider status.");
    assert_eq!(app.panel_title(), "Status");
}

#[test]
fn autocomplete_prepends_a_slash_command_prefix() {
    let mut app = App::new();

    app.handle_key(press(KeyCode::Char('/')));
    app.handle_key(press(KeyCode::Tab));

    assert!(app.prompt().starts_with('/'));
}

#[test]
fn obsidian_sync_imports_markdown_tree() {
    let root = std::env::temp_dir().join(format!("aleph-obsidian-test-{}", App::now_millis()));
    let project_dir = root.join("Projects");
    fs::create_dir_all(&project_dir).unwrap();
    fs::write(root.join("Inbox.md"), "# Inbox\n\nTop-level note").unwrap();
    fs::write(project_dir.join("Plan.md"), "# Project Plan\n\nNested note").unwrap();
    fs::write(root.join("ignore.txt"), "not markdown").unwrap();

    let mut app = App::new();
    app.obsidian_vault_path = Some(root.clone());
    app.folders.clear();
    app.notes.clear();

    let count = app.sync_obsidian_notes().unwrap();

    assert_eq!(count, 2);
    assert!(app.notes.iter().any(|note| note.title == "Inbox"));
    assert!(app.notes.iter().any(|note| note.title == "Project Plan"));
    assert!(app.notes.iter().all(|note| note.title != "ignore"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn note_list_handles_unicode_obsidian_folder_and_title() {
    let mut app = App::new();
    app.folders.push(Folder {
        id: 99,
        name: String::from("研究ノート集"),
        parent_id: None,
    });
    app.notes = vec![test_note(1, None, "計画とアイデアの長いノート", "body")];
    app.notes[0].folder_id = Some(99);
    app.notes[0].obsidian_path = Some(PathBuf::from("/tmp/unicode.md"));
    app.expanded_folders.push(99);

    app.open_note_list_panel();

    assert_eq!(app.panel_lines.len(), 2);
    assert!(app.panel_lines[0].contains("研究ノート集"));
    assert!(app.panel_lines[1].contains("計画とアイデアの長いノート"));
}

#[test]
fn note_list_rebuilds_obsidian_tree_from_cached_paths() {
    let root = std::env::temp_dir().join(format!("aleph-cached-tree-test-{}", App::now_millis()));
    let mut app = App::new();
    app.obsidian_vault_path = Some(root.clone());
    app.folders.clear();
    app.expanded_folders.clear();
    app.notes = vec![
        test_note(1, None, "Inbox", "# Inbox"),
        test_note(2, None, "Project Plan", "# Project Plan"),
    ];
    app.notes[0].obsidian_path = Some(root.join("Inbox.md"));
    app.notes[1].obsidian_path = Some(root.join("Projects").join("Plan.md"));

    app.open_note_list_panel();

    assert!(app.panel_title.contains("Space expand/collapse"));
    assert!(app.panel_lines.iter().any(|line| line.contains("Obsidian")));
    assert!(app
        .panel_lines
        .iter()
        .any(|line| line.contains("aleph-cached-tree-test")));
    assert!(app.panel_lines.iter().any(|line| line.contains("Projects")));
    assert!(app.panel_lines.iter().any(|line| line.contains("Inbox")));
    assert!(app
        .panel_lines
        .iter()
        .any(|line| line.contains("Project Plan")));
    assert!(app.notes.iter().all(|note| note.folder_id.is_some()));
}

#[test]
fn settings_round_trip_to_config() {
    let _guard = env_lock();
    let config_dir =
        std::env::temp_dir().join(format!("aleph-settings-test-{}", App::now_millis()));
    std::env::set_var("ALEPH_CONFIG_DIR", &config_dir);

    let mut app = App::new();
    app.note_save_target = NoteSaveTarget::Obsidian;
    app.store_note_save_target().unwrap();
    app.ai_provider = AiProvider::Strix;
    app.store_ai_provider().unwrap();
    app.agent_mode_enabled = false;
    app.store_agent_mode_enabled().unwrap();
    app.editor_images_enabled = true;
    app.store_editor_images_enabled().unwrap();

    assert_eq!(App::load_note_save_target(), Some(NoteSaveTarget::Obsidian));
    assert_eq!(App::load_ai_provider(), Some(AiProvider::Strix));
    assert_eq!(App::load_agent_mode_enabled(), Some(false));
    assert_eq!(App::load_editor_images_enabled(), Some(true));

    std::env::remove_var("ALEPH_CONFIG_DIR");
    let _ = fs::remove_dir_all(config_dir);
}

#[test]
fn obsidian_pair_without_target_opens_picker_for_multiple_vaults() {
    let mut app = App::new();
    app.obsidian_vaults = vec![
        ObsidianVault {
            id: String::from("one"),
            name: String::from("One"),
            path: PathBuf::from("/tmp/one"),
            source: String::from("test"),
        },
        ObsidianVault {
            id: String::from("two"),
            name: String::from("Two"),
            path: PathBuf::from("/tmp/two"),
            source: String::from("test"),
        },
    ];

    app.open_vault_picker();

    assert!(app.is_vault_picker());
    assert_eq!(app.obsidian_vault_selected(), 0);
}

#[test]
fn settings_obsidian_row_opens_pairing_when_unpaired() {
    let mut app = App::new();
    app.obsidian_vault_path = None;

    app.open_settings_panel();
    for _ in 0..4 {
        app.handle_settings_key(press(KeyCode::Down));
    }
    app.handle_settings_key(press(KeyCode::Enter));

    assert!(app.is_vault_picker());
}

#[test]
fn clicking_settings_obsidian_row_opens_pairing_when_unpaired() {
    let mut app = App::new();
    app.obsidian_vault_path = None;

    app.open_settings_panel();
    app.handle_settings_mouse_with_size(
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 4,
            row: 24,
            modifiers: KeyModifiers::NONE,
        },
        40,
        80,
    );

    assert!(app.is_vault_picker());
}

#[test]
fn settings_mouse_hit_test_tracks_rendered_panel_layout() {
    assert_eq!(App::settings_index_for_mouse_row(40, 80, 20), Some(0));
    assert_eq!(App::settings_index_for_mouse_row(40, 80, 24), Some(4));
    assert_eq!(App::settings_index_for_mouse_row(40, 80, 19), None);

    assert_eq!(App::settings_index_for_mouse_row(22, 80, 20), None);
    assert_eq!(App::settings_index_for_mouse_row(22, 80, 24), None);
}

#[test]
fn reset_clears_obsidian_pairing_fallback_file() {
    let _guard = env_lock();
    let config_dir =
        std::env::temp_dir().join(format!("aleph-obsidian-reset-test-{}", App::now_millis()));
    std::env::set_var("ALEPH_CONFIG_DIR", &config_dir);
    fs::create_dir_all(&config_dir).unwrap();
    fs::write(App::obsidian_pairing_path(), "/tmp/aleph-test-vault").unwrap();

    let mut app = App::new();
    app.obsidian_vault_path = Some(PathBuf::from("/tmp/aleph-test-vault"));
    app.note_save_target = NoteSaveTarget::Obsidian;

    app.reset_and_clear_all();

    assert!(app.obsidian_vault_path().is_none());
    assert_eq!(app.note_save_target, NoteSaveTarget::Local);
    assert!(!App::obsidian_pairing_path().exists());

    std::env::remove_var("ALEPH_CONFIG_DIR");
    let _ = fs::remove_dir_all(config_dir);
}

#[test]
fn obsidian_filenames_are_sanitized() {
    assert_eq!(
        App::safe_obsidian_filename("Daily/Plan: Q2?"),
        "Daily-Plan- Q2-"
    );
    assert_eq!(App::safe_obsidian_filename("   ...   "), "Untitled note");
}

#[test]
fn strix_sync_merge_preserves_local_only_notes() {
    let mut app = App::new();
    app.notes = vec![
        test_note(1, None, "Offline draft", "local"),
        test_note(2, Some("remote-1"), "Cached remote", "old"),
    ];

    app.merge_strix_notes(vec![
        test_note(9, Some("remote-1"), "Remote updated", "new"),
        test_note(10, Some("remote-2"), "Remote new", "fresh"),
    ]);

    assert!(app.notes.iter().any(|note| note.title == "Offline draft"));
    assert!(app.notes.iter().any(|note| note.title == "Remote updated"));
    assert!(app.notes.iter().any(|note| note.title == "Remote new"));
    assert_eq!(app.notes.len(), 3);
}

#[test]
fn upsert_existing_synced_note_updates_cache() {
    let _guard = env_lock();
    let cache_path =
        std::env::temp_dir().join(format!("aleph-strix-cache-test-{}.json", App::now_millis()));
    std::env::set_var("ALEPH_STRIX_CACHE", &cache_path);

    let mut app = App::new();
    app.notes = vec![test_note(1, Some("remote-1"), "Old", "old")];
    app.upsert_synced_note(test_note(99, Some("remote-1"), "Updated", "new"));

    let saved = fs::read_to_string(&cache_path).unwrap();
    assert!(saved.contains("Updated"));
    assert!(saved.contains("new"));

    std::env::remove_var("ALEPH_STRIX_CACHE");
    let _ = fs::remove_file(cache_path);
}

#[test]
fn local_notes_round_trip_through_cache() {
    let _guard = env_lock();
    let notes_path =
        std::env::temp_dir().join(format!("aleph-local-notes-test-{}.json", App::now_millis()));
    std::env::set_var("ALEPH_NOTES_PATH", &notes_path);

    let mut note = test_note(7, None, "Local draft", "saved body");
    note.folder_id = Some(3);
    App::save_local_notes(&[note]).unwrap();

    let loaded = App::load_local_notes().unwrap();
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].title, "Local draft");
    assert_eq!(loaded[0].content, "saved body");
    assert_eq!(loaded[0].folder_id, Some(3));

    std::env::remove_var("ALEPH_NOTES_PATH");
    let _ = fs::remove_file(notes_path);
}

#[test]
fn legacy_sample_notes_are_not_loaded_from_local_cache() {
    let _guard = env_lock();
    let notes_path = std::env::temp_dir().join(format!(
        "aleph-legacy-sample-notes-test-{}.json",
        App::now_millis()
    ));
    std::env::set_var("ALEPH_NOTES_PATH", &notes_path);

    let notes = vec![
        test_note(1, None, "Strix gateway", "sample"),
        test_note(2, None, "Real note", "keep"),
    ];
    App::save_local_notes(&notes).unwrap();

    let loaded = App::load_local_notes().unwrap();
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].title, "Real note");

    std::env::remove_var("ALEPH_NOTES_PATH");
    let _ = fs::remove_file(notes_path);
}

#[test]
fn title_edit_cursor_stays_on_utf8_boundaries() {
    let mut app = App::new();
    app.editing_title = true;

    app.handle_title_edit_key(press(KeyCode::Char('é')));
    assert_eq!(app.title_buffer, "é");
    assert_eq!(app.title_cursor, "é".len());

    app.handle_title_edit_key(press(KeyCode::Left));
    assert_eq!(app.title_cursor, 0);

    app.handle_title_edit_key(press(KeyCode::Right));
    assert_eq!(app.title_cursor, "é".len());

    app.handle_title_edit_key(press(KeyCode::Backspace));
    assert!(app.title_buffer.is_empty());
    assert_eq!(app.title_cursor, 0);
}

#[test]
fn note_edit_opens_the_editor_and_saves_changes() {
    let mut app = App::new();
    app.note_save_target = NoteSaveTarget::Local;

    for character in "/note edit".chars() {
        app.handle_key(press(KeyCode::Char(character)));
    }
    app.handle_key(press(KeyCode::Enter));

    assert!(app.is_full_editor());

    for character in "\nAdded from the editor".chars() {
        match character {
            '\n' => app.handle_key(press(KeyCode::Enter)),
            other => app.handle_key(press(KeyCode::Char(other))),
        }
    }

    app.handle_key(KeyEvent {
        code: KeyCode::Char('s'),
        modifiers: KeyModifiers::CONTROL,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    });

    assert!(app.is_full_editor());
    assert!(app.notes[0].content.contains("Added from the editor"));

    app.handle_key(KeyEvent {
        code: KeyCode::Esc,
        modifiers: KeyModifiers::NONE,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    });

    assert!(!app.is_full_editor());
}

#[test]
fn ai_edit_proposal_requires_explicit_apply() {
    let mut app = App::new();
    app.note_save_target = NoteSaveTarget::Local;
    app.open_note_editor(0);
    let original = app.editor_buffer.clone();
    let proposed = format!("{}\n\nAdded by AI", original);
    app.pending_ai_edit = Some(AiEditProposal {
        note_index: Some(0),
        title: None,
        instruction: String::from("append a line"),
        proposed: proposed.clone(),
        diff_lines: App::build_line_diff(&original, &proposed),
    });
    app.ai_overlay_visible = true;

    app.handle_key(press(KeyCode::Char('x')));
    assert_eq!(app.editor_buffer, original);
    assert_eq!(app.editor_display_buffer(), proposed);
    assert!(app.has_live_ai_editor_preview());
    assert!(app.has_pending_ai_edit());

    app.handle_key(press(KeyCode::Enter));
    assert_eq!(app.editor_buffer, proposed);
    assert_eq!(app.notes[0].content, proposed);
    assert!(!app.has_pending_ai_edit());
}

#[test]
fn ai_edit_proposal_can_be_rejected() {
    let mut app = App::new();
    app.open_note_editor(0);
    let original = app.editor_buffer.clone();
    let proposed = format!("{}\n\nAdded by AI", original);
    app.pending_ai_edit = Some(AiEditProposal {
        note_index: Some(0),
        title: None,
        instruction: String::from("append a line"),
        proposed,
        diff_lines: App::build_line_diff(&original, "changed"),
    });
    app.ai_overlay_visible = true;

    app.handle_key(ctrl(KeyCode::Char('r')));

    assert_eq!(app.editor_buffer, original);
    assert!(!app.has_pending_ai_edit());
}

#[test]
fn note_create_accepts_initial_body() {
    let mut app = App::new();
    app.note_save_target = NoteSaveTarget::Local;

    for character in "/note create Test note :: first line".chars() {
        app.handle_key(press(KeyCode::Char(character)));
    }
    app.handle_key(press(KeyCode::Enter));

    assert!(app.is_full_editor());
    assert_eq!(app.editor_note_title(), Some("Test note"));
    assert_eq!(app.editor_buffer(), "first line");
}

#[test]
fn note_append_can_target_a_note() {
    let mut app = App::new();
    seed_test_notes(&mut app);
    app.note_save_target = NoteSaveTarget::Local;

    for character in "/note append Feature ideas :: added target text".chars() {
        app.handle_key(press(KeyCode::Char(character)));
    }
    app.handle_key(press(KeyCode::Enter));

    let target = app
        .notes
        .iter()
        .find(|note| note.title == "Feature ideas")
        .unwrap();
    assert!(target.content.contains("added target text"));
}

#[test]
fn temporal_fork_manual_creation_persists_and_renders() {
    let _guard = env_lock();
    let root = std::env::temp_dir().join(format!("aleph-forks-test-{}", App::now_millis()));
    let forks_path = root.join("temporal-forks.json");
    let notes_path = root.join("notes.json");
    fs::create_dir_all(&root).unwrap();
    std::env::set_var("ALEPH_FORKS_PATH", &forks_path);
    std::env::set_var("ALEPH_NOTES_PATH", &notes_path);

    let mut app = App::new();
    seed_test_notes(&mut app);
    app.memories.push(String::from("remember this timeline"));
    app.execute_command("path save", "alpha path");

    assert_eq!(app.temporal_forks.len(), 1);
    assert!(app.panel_title().contains("Saved path"));
    assert!(app
        .panel_lines()
        .iter()
        .any(|line| line.contains("alpha path")));

    let (loaded, current) = App::load_temporal_fork_state().unwrap();
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].label, "alpha path");
    assert_eq!(
        loaded[0].memories,
        vec![String::from("remember this timeline")]
    );
    assert_eq!(current.as_deref(), Some(loaded[0].id.as_str()));

    app.execute_command("path list", "");
    assert!(app
        .panel_lines()
        .iter()
        .any(|line| line.contains("alpha path")));
    app.execute_command("path show", "alpha path");
    assert!(app
        .panel_lines()
        .iter()
        .any(|line| line.contains("Memories: 1")));

    std::env::remove_var("ALEPH_FORKS_PATH");
    std::env::remove_var("ALEPH_NOTES_PATH");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn temporal_fork_auto_snapshots_before_note_writes() {
    let _guard = env_lock();
    let root = std::env::temp_dir().join(format!("aleph-auto-forks-test-{}", App::now_millis()));
    let forks_path = root.join("temporal-forks.json");
    let notes_path = root.join("notes.json");
    fs::create_dir_all(&root).unwrap();
    std::env::set_var("ALEPH_FORKS_PATH", &forks_path);
    std::env::set_var("ALEPH_NOTES_PATH", &notes_path);

    let mut app = App::new();
    seed_test_notes(&mut app);
    app.note_save_target = NoteSaveTarget::Local;

    app.execute_command("note append", "Feature ideas :: added target text");
    app.execute_command("note create", "Forked draft :: first line");
    app.open_note_editor(0);
    app.editor_buffer.push_str("\nSaved from editor");
    app.save_editor();
    app.delete_note_at_index(1).unwrap();

    assert!(app
        .temporal_forks
        .iter()
        .any(|fork| fork.label == "Before note append"));
    assert!(app
        .temporal_forks
        .iter()
        .any(|fork| fork.label == "Before note create"));
    assert!(app
        .temporal_forks
        .iter()
        .any(|fork| fork.label == "Before note save"));
    assert!(app
        .temporal_forks
        .iter()
        .any(|fork| fork.label == "Before note delete"));

    std::env::remove_var("ALEPH_FORKS_PATH");
    std::env::remove_var("ALEPH_NOTES_PATH");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn temporal_fork_auto_snapshots_before_ai_apply() {
    let _guard = env_lock();
    let root = std::env::temp_dir().join(format!("aleph-ai-forks-test-{}", App::now_millis()));
    let forks_path = root.join("temporal-forks.json");
    let notes_path = root.join("notes.json");
    fs::create_dir_all(&root).unwrap();
    std::env::set_var("ALEPH_FORKS_PATH", &forks_path);
    std::env::set_var("ALEPH_NOTES_PATH", &notes_path);

    let mut app = App::new();
    app.note_save_target = NoteSaveTarget::Local;
    app.open_note_editor(0);
    let original = app.editor_buffer.clone();
    let proposed = format!("{}\n\nAI branch", original);
    app.pending_ai_edit = Some(AiEditProposal {
        note_index: Some(0),
        title: None,
        instruction: String::from("append"),
        proposed,
        diff_lines: Vec::new(),
    });

    app.apply_pending_ai_edit();

    assert!(app
        .temporal_forks
        .iter()
        .any(|fork| fork.label == "Before AI edit apply"));
    assert_eq!(
        app.temporal_forks
            .iter()
            .filter(|fork| fork.label == "Before AI edit apply")
            .count(),
        1
    );

    std::env::remove_var("ALEPH_FORKS_PATH");
    std::env::remove_var("ALEPH_NOTES_PATH");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn temporal_fork_checkout_restores_local_state_only() {
    let _guard = env_lock();
    let root =
        std::env::temp_dir().join(format!("aleph-checkout-forks-test-{}", App::now_millis()));
    let forks_path = root.join("temporal-forks.json");
    let notes_path = root.join("notes.json");
    let obsidian_note = root.join("External.md");
    fs::create_dir_all(&root).unwrap();
    fs::write(&obsidian_note, "external file stayed put").unwrap();
    std::env::set_var("ALEPH_FORKS_PATH", &forks_path);
    std::env::set_var("ALEPH_NOTES_PATH", &notes_path);

    let mut app = App::new();
    app.notes = vec![test_note(1, Some("remote-1"), "Timeline", "fork body")];
    app.notes[0].obsidian_path = Some(obsidian_note.clone());
    app.memories = vec![String::from("memory before branch")];
    app.selected_note = 0;
    let fork_id = app.create_temporal_fork("restore point", "manual").unwrap();

    app.note_save_target = NoteSaveTarget::Strix;
    app.notes[0].content = String::from("mutated body");
    app.memories.clear();
    let index = app.resolve_temporal_fork_index(&fork_id).unwrap();
    let result = app.checkout_temporal_fork(index);

    assert!(result.is_ok());
    assert_eq!(app.notes[0].content, "fork body");
    assert_eq!(app.memories, vec![String::from("memory before branch")]);
    assert_eq!(
        fs::read_to_string(&obsidian_note).unwrap(),
        "external file stayed put"
    );

    std::env::remove_var("ALEPH_FORKS_PATH");
    std::env::remove_var("ALEPH_NOTES_PATH");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn temporal_fork_repo_context_degrades_outside_git_repo() {
    let _guard = env_lock();
    let root = std::env::temp_dir().join(format!("aleph-no-git-forks-test-{}", App::now_millis()));
    let forks_path = root.join("temporal-forks.json");
    let notes_path = root.join("notes.json");
    fs::create_dir_all(&root).unwrap();
    let original_cwd = std::env::current_dir().unwrap();
    std::env::set_current_dir(&root).unwrap();
    std::env::set_var("ALEPH_FORKS_PATH", &forks_path);
    std::env::set_var("ALEPH_NOTES_PATH", &notes_path);

    let mut app = App::new();
    app.create_temporal_fork("outside git", "manual").unwrap();

    assert!(app.temporal_forks[0].repo_context.is_none());

    std::env::set_current_dir(original_cwd).unwrap();
    std::env::remove_var("ALEPH_FORKS_PATH");
    std::env::remove_var("ALEPH_NOTES_PATH");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn note_list_delete_requires_second_press() {
    let mut app = App::new();
    let original_count = app.notes.len();

    app.open_note_list_panel();
    app.handle_note_list_key(press(KeyCode::Delete));

    assert_eq!(app.notes.len(), original_count);
    assert!(app.note_list_delete_is_pending());

    app.handle_note_list_key(press(KeyCode::Delete));

    assert_eq!(app.notes.len(), original_count - 1);
    assert!(!app.note_list_delete_is_pending());
}

#[test]
fn note_list_delete_can_be_confirmed_with_enter_or_d() {
    let mut app = App::new();
    seed_test_notes(&mut app);
    let original_count = app.notes.len();

    app.open_note_list_panel();
    app.handle_note_list_key(press(KeyCode::Delete));
    app.handle_note_list_key(press(KeyCode::Enter));

    assert_eq!(app.notes.len(), original_count - 1);
    assert!(!app.note_list_delete_is_pending());

    app.handle_note_list_key(press(KeyCode::Delete));
    app.handle_note_list_key(press(KeyCode::Char('d')));

    assert_eq!(app.notes.len(), original_count - 2);
    assert!(!app.note_list_delete_is_pending());
}

#[test]
fn note_list_delete_repeat_does_not_confirm() {
    let mut app = App::new();
    let original_count = app.notes.len();

    app.open_note_list_panel();
    app.handle_note_list_key(press(KeyCode::Delete));
    app.handle_note_list_key(repeat(KeyCode::Delete));

    assert_eq!(app.notes.len(), original_count);
    assert!(app.note_list_delete_is_pending());
}

#[test]
fn note_list_delete_pending_is_cancelled_by_moving_selection() {
    let mut app = App::new();
    seed_test_notes(&mut app);
    let original_count = app.notes.len();

    app.open_note_list_panel();
    app.handle_note_list_key(press(KeyCode::Delete));
    app.handle_note_list_key(press(KeyCode::Down));

    assert_eq!(app.notes.len(), original_count);
    assert!(!app.note_list_delete_is_pending());
}

#[test]
fn note_list_delete_removes_obsidian_file() {
    let root = std::env::temp_dir().join(format!("aleph-note-delete-test-{}", App::now_millis()));
    fs::create_dir_all(&root).unwrap();
    let note_path = root.join("Delete Me.md");
    fs::write(&note_path, "temporary note").unwrap();

    let mut app = App::new();
    app.notes = vec![test_note(1, None, "Delete Me", "temporary note")];
    app.notes[0].obsidian_path = Some(note_path.clone());

    app.open_note_list_panel();
    app.handle_note_list_key(press(KeyCode::Delete));
    app.handle_note_list_key(press(KeyCode::Delete));

    assert!(app.notes.is_empty());
    assert!(!note_path.exists());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn chat_note_create_request_opens_ai_draft_instead_of_chatting() {
    let mut app = App::new();
    app.openrouter_api_key = None;
    app.strix_access_token = None;
    app.refresh_connection_state();
    app.panel_mode = PanelMode::AiChat;
    app.chat_input_buffer = String::from("write a note about launch planning");
    app.chat_input_cursor = app.chat_input_buffer.len();

    app.handle_chat_key(press(KeyCode::Enter));

    assert!(app.is_ai_chat());
    assert!(app.pending_agent_decision.is_some());
    assert_eq!(app.chat_messages().len(), 2);

    app.handle_chat_key(press(KeyCode::Enter));

    assert!(app.is_full_editor());
    assert_eq!(
        app.ai_draft_create_title.as_deref(),
        Some("Launch Planning")
    );
    assert!(app.pending_agent_decision.is_none());
}

#[test]
fn agent_mode_routes_general_write_prompt_to_note_draft() {
    let mut app = App::new();
    app.openrouter_api_key = None;
    app.strix_access_token = None;
    app.refresh_connection_state();
    app.panel_mode = PanelMode::AiChat;
    app.chat_input_buffer = String::from("write an outline about moat strategy");
    app.chat_input_cursor = app.chat_input_buffer.len();

    app.handle_chat_key(press(KeyCode::Enter));

    assert!(app.is_ai_chat());
    assert!(app.pending_agent_decision.is_some());
    assert_eq!(app.chat_messages().len(), 2);

    app.handle_chat_key(press(KeyCode::Enter));

    assert!(app.is_full_editor());
    assert_eq!(app.ai_draft_create_title.as_deref(), Some("Moat Strategy"));
    assert!(app.pending_agent_decision.is_none());
}

#[test]
fn agent_mode_routes_current_note_edit_without_note_keyword() {
    let mut app = App::new();
    seed_test_notes(&mut app);
    app.openrouter_api_key = None;
    app.strix_access_token = None;
    app.refresh_connection_state();
    app.selected_note = 1;
    app.panel_mode = PanelMode::AiChat;
    app.chat_input_buffer = String::from("make this more concise");
    app.chat_input_cursor = app.chat_input_buffer.len();

    app.handle_chat_key(press(KeyCode::Enter));

    assert!(app.is_ai_chat());
    assert!(app.pending_agent_decision.is_some());
    assert_eq!(app.chat_messages().len(), 2);

    app.handle_chat_key(press(KeyCode::Enter));

    assert!(app.is_full_editor());
    assert_eq!(app.editor_note_index, Some(1));
    assert!(app.pending_agent_decision.is_none());
}

#[test]
fn agent_mode_can_decide_to_work_on_existing_selected_note() {
    let mut app = App::new();
    seed_test_notes(&mut app);
    app.openrouter_api_key = None;
    app.strix_access_token = None;
    app.refresh_connection_state();
    app.selected_note = 2;
    app.panel_mode = PanelMode::AiChat;
    app.chat_input_buffer = String::from("work on the existing note and make progress");
    app.chat_input_cursor = app.chat_input_buffer.len();

    app.handle_chat_key(press(KeyCode::Enter));

    assert!(app.is_ai_chat());
    assert!(app.pending_agent_decision.is_some());
    assert_eq!(app.chat_messages().len(), 2);

    app.handle_chat_key(press(KeyCode::Enter));

    assert!(app.is_full_editor());
    assert_eq!(app.editor_note_index, Some(2));
    assert!(app.pending_agent_decision.is_none());
}

#[test]
fn agent_mode_can_choose_existing_note_by_title() {
    let mut app = App::new();
    seed_test_notes(&mut app);
    app.openrouter_api_key = None;
    app.strix_access_token = None;
    app.refresh_connection_state();
    app.panel_mode = PanelMode::AiChat;
    app.chat_input_buffer = String::from("work on Feature ideas and make it sharper");
    app.chat_input_cursor = app.chat_input_buffer.len();

    app.handle_chat_key(press(KeyCode::Enter));

    assert!(app.is_ai_chat());
    assert!(app.pending_agent_decision.is_some());
    assert_eq!(app.chat_messages().len(), 2);

    app.handle_chat_key(press(KeyCode::Enter));

    assert!(app.is_full_editor());
    assert_eq!(app.editor_note_index, Some(3));
    assert!(app.pending_agent_decision.is_none());
}

#[test]
fn agent_mode_can_read_named_note_without_provider() {
    let mut app = App::new();
    seed_test_notes(&mut app);
    app.openrouter_api_key = None;
    app.strix_access_token = None;
    app.refresh_connection_state();
    app.panel_mode = PanelMode::AiChat;
    app.chat_input_buffer = String::from("read Feature ideas");
    app.chat_input_cursor = app.chat_input_buffer.len();

    app.handle_chat_key(press(KeyCode::Enter));

    assert!(app.is_ai_chat());
    assert!(app.pending_agent_decision.is_none());
    assert_eq!(app.chat_messages().len(), 2);
    assert!(app.chat_messages()[1].content.contains("Feature ideas"));
    assert!(app.chat_messages()[1].content.contains("feature notes"));
}

#[test]
fn agent_mode_can_search_notes_without_provider() {
    let mut app = App::new();
    seed_test_notes(&mut app);
    app.openrouter_api_key = None;
    app.strix_access_token = None;
    app.refresh_connection_state();
    app.panel_mode = PanelMode::AiChat;
    app.chat_input_buffer = String::from("find notes about gateway");
    app.chat_input_cursor = app.chat_input_buffer.len();

    app.handle_chat_key(press(KeyCode::Enter));

    assert!(app.is_ai_chat());
    assert!(app.pending_agent_decision.is_none());
    assert_eq!(app.chat_messages().len(), 2);
    assert!(app.chat_messages()[1].content.contains("Strix gateway"));
}

#[test]
fn agent_search_extracts_subject_from_find_note_request() {
    let mut app = App::new();
    app.notes = vec![
        test_note(
            1,
            None,
            "Founder Advice",
            "Notes on advice from Steve Jobs and Peter Thiel.",
        ),
        test_note(2, None, "Groceries", "milk eggs bread"),
    ];
    app.openrouter_api_key = None;
    app.strix_access_token = None;
    app.refresh_connection_state();
    app.panel_mode = PanelMode::AiChat;
    app.chat_input_buffer =
        String::from("find a note that I have on advice from steve jobs and peter thiel");
    app.chat_input_cursor = app.chat_input_buffer.len();

    app.handle_chat_key(press(KeyCode::Enter));

    assert!(app.is_ai_chat());
    assert!(app.pending_agent_decision.is_none());
    assert_eq!(app.chat_messages().len(), 2);
    assert!(app.chat_messages()[1].content.contains("Founder Advice"));
    assert!(!app.chat_messages()[1]
        .content
        .contains("find a note that I have"));
}

#[test]
fn agent_mode_can_go_through_memories_without_provider() {
    let mut app = App::new();
    app.openrouter_api_key = None;
    app.strix_access_token = None;
    app.memories = vec![
        String::from("Prefer concise notes."),
        String::from("Launch work uses the Aleph vault."),
    ];
    app.refresh_connection_state();
    app.panel_mode = PanelMode::AiChat;
    app.chat_input_buffer = String::from("go through memories");
    app.chat_input_cursor = app.chat_input_buffer.len();

    app.handle_chat_key(press(KeyCode::Enter));

    assert!(app.is_ai_chat());
    assert!(app.pending_agent_decision.is_none());
    assert_eq!(app.chat_messages().len(), 2);
    assert!(app.chat_messages()[1].content.contains("Saved memories"));
    assert!(app.chat_messages()[1]
        .content
        .contains("Prefer concise notes"));
}

#[test]
fn agent_mode_keeps_how_to_writing_questions_as_chat() {
    let mut app = App::new();
    app.openrouter_api_key = None;
    app.strix_access_token = None;
    app.refresh_connection_state();
    app.panel_mode = PanelMode::AiChat;
    app.chat_input_buffer = String::from("how do I write a note about launch planning?");
    app.chat_input_cursor = app.chat_input_buffer.len();

    app.handle_chat_key(press(KeyCode::Enter));

    assert!(app.is_ai_chat());
    assert_eq!(app.chat_messages().len(), 2);
    assert!(app.chat_messages()[0].content.contains("how do I write"));
}

#[test]
fn chat_mode_keeps_note_requests_as_chat() {
    let mut app = App::new();
    app.openrouter_api_key = None;
    app.strix_access_token = None;
    app.refresh_connection_state();
    app.agent_mode_enabled = false;
    app.panel_mode = PanelMode::AiChat;
    app.chat_input_buffer = String::from("write a note about launch planning");
    app.chat_input_cursor = app.chat_input_buffer.len();

    app.handle_chat_key(press(KeyCode::Enter));

    assert!(app.is_ai_chat());
    assert_eq!(app.chat_messages().len(), 2);
    assert!(app.chat_messages()[0].content.contains("write a note"));
}

#[test]
fn mode_commands_switch_agent_routing() {
    let mut app = App::new();

    for character in "/mode chat".chars() {
        app.handle_key(press(KeyCode::Char(character)));
    }
    app.handle_key(press(KeyCode::Enter));
    assert!(!app.is_agent_mode_enabled());

    for character in "/mode agent".chars() {
        app.handle_key(press(KeyCode::Char(character)));
    }
    app.handle_key(press(KeyCode::Enter));
    assert!(app.is_agent_mode_enabled());
}

#[test]
fn ai_create_proposal_creates_note_when_applied() {
    let mut app = App::new();
    app.openrouter_api_key = None;
    app.strix_access_token = None;
    app.obsidian_vault_path = None;
    app.note_save_target = NoteSaveTarget::Local;
    app.refresh_connection_state();
    let original_count = app.notes.len();
    app.panel_mode = PanelMode::FullEditor;
    app.ai_overlay_visible = true;
    app.pending_ai_edit = Some(AiEditProposal {
        note_index: None,
        title: Some(String::from("Launch Planning")),
        instruction: String::from("write a note"),
        proposed: String::from("# Launch Planning\n\nShip the smallest useful path."),
        diff_lines: App::build_line_diff("", "# Launch Planning\n\nShip the smallest useful path."),
    });

    app.handle_key(press(KeyCode::Enter));

    assert_eq!(app.notes.len(), original_count + 1);
    assert_eq!(app.editor_note_title(), Some("Launch Planning"));
    assert!(app
        .editor_buffer()
        .contains("Ship the smallest useful path."));
}

#[test]
fn settings_cycles_note_save_target_through_available_targets() {
    let mut app = App::new();
    app.openrouter_api_key = None;
    app.strix_access_token = Some(String::from("token"));
    app.obsidian_vault_path = Some(PathBuf::from("/tmp"));
    app.note_save_target = NoteSaveTarget::Local;
    app.refresh_connection_state();

    app.cycle_note_save_target();
    assert_eq!(app.note_save_target, NoteSaveTarget::Obsidian);

    app.cycle_note_save_target();
    assert_eq!(app.note_save_target, NoteSaveTarget::Strix);

    app.cycle_note_save_target();
    assert_eq!(app.note_save_target, NoteSaveTarget::Local);
}

#[test]
fn local_save_target_does_not_assign_obsidian_or_strix_source() {
    let mut app = App::new();
    app.openrouter_api_key = None;
    app.strix_access_token = Some(String::from("token"));
    app.obsidian_vault_path = Some(PathBuf::from("/tmp"));
    app.note_save_target = NoteSaveTarget::Local;
    app.refresh_connection_state();

    let index = app.create_note_from_content("Local only", "body").unwrap();

    assert!(app.notes[index].remote_id.is_none());
    assert!(app.notes[index].obsidian_path.is_none());
}

#[test]
fn editor_vertical_navigation_keeps_cursor_on_char_boundary() {
    let mut app = App::new();
    app.editor_buffer = String::from("ééé\nab");
    app.editor_cursor = "éé".len();

    app.editor_move_down();
    assert!(app.editor_buffer.is_char_boundary(app.editor_cursor));
    assert_eq!(app.editor_cursor, app.editor_buffer.len());

    app.editor_move_up();
    assert!(app.editor_buffer.is_char_boundary(app.editor_cursor));
    assert_eq!(app.editor_cursor, "éé".len());
}

#[test]
fn chat_markdown_tables_are_padded_as_blocks() {
    let lines = App::render_chat_markdown_lines_owned(
        "| Name | Count |\n| --- | ---: |\n| Alpha | 2 |\n| Beta project | 14 |",
    );
    let text = lines
        .iter()
        .map(|line| {
            line.spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect::<String>()
        })
        .collect::<Vec<_>>();

    assert_eq!(text[0], "| Name         | Count |");
    assert_eq!(text[1], "| ------------ | ----- |");
    assert_eq!(text[2], "| Alpha        | 2     |");
    assert_eq!(text[3], "| Beta project | 14    |");
}

#[test]
fn agent_note_search_ranks_relevant_notes_with_snippets() {
    let mut app = App::new();
    app.notes = vec![
        test_note(1, None, "Cooking", "Pasta and sauce"),
        test_note(
            2,
            None,
            "Rust terminal UI",
            "Ratatui table rendering and scroll behavior in chat.",
        ),
        test_note(3, None, "Garden", "Seeds and water"),
    ];
    app.selected_note = 0;

    let response = app.agent_search_notes_response(
        &AgentDecision {
            action: AgentAction::SearchNotes,
            note_index: None,
            title: None,
            search_query: Some(String::from("table scroll chat")),
            rationale: String::from("test"),
        },
        "find notes about table scroll chat",
    );

    assert!(response.contains("Rust terminal UI"));
    assert!(response.contains("Ratatui table rendering"));
    assert!(!response.contains("Cooking"));
}

#[test]
fn memory_save_persists_to_local_cache() {
    let _guard = env_lock();
    let root = std::env::temp_dir().join(format!("aleph-memory-test-{}", App::now_millis()));
    fs::create_dir_all(&root).unwrap();
    let previous_config_dir = std::env::var_os("ALEPH_CONFIG_DIR");
    std::env::set_var("ALEPH_CONFIG_DIR", &root);

    let mut app = App::new();
    app.memories.clear();
    let response = app.agent_save_memory_response(
        &AgentDecision {
            action: AgentAction::SaveMemory,
            note_index: None,
            title: None,
            search_query: Some(String::from("remember that tables should stay aligned")),
            rationale: String::from("test"),
        },
        "",
    );

    assert!(response.contains("Saved memory locally"));
    assert_eq!(
        app.memories,
        vec![String::from("tables should stay aligned")]
    );
    assert_eq!(
        App::load_local_memories().unwrap(),
        vec![String::from("tables should stay aligned")]
    );

    if let Some(previous) = previous_config_dir {
        std::env::set_var("ALEPH_CONFIG_DIR", previous);
    } else {
        std::env::remove_var("ALEPH_CONFIG_DIR");
    }
    let _ = fs::remove_dir_all(root);
}
