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

fn modified(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
    KeyEvent {
        code,
        modifiers,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    }
}

fn env_lock() -> std::sync::MutexGuard<'static, ()> {
    static ENV_LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
    ENV_LOCK
        .get_or_init(|| std::sync::Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[test]
fn openrouter_callback_url_matches_the_ipv4_loopback_listener() {
    assert_eq!(
        App::openrouter_callback_url(43_880, "/aleph/openrouter/callback/test"),
        "http://127.0.0.1:43880/aleph/openrouter/callback/test"
    );
}

#[test]
fn openrouter_key_fallback_path_uses_aleph_config_directory() {
    let _guard = env_lock();
    let original = std::env::var_os("ALEPH_CONFIG_DIR");
    let config_dir =
        std::env::temp_dir().join(format!("aleph-openrouter-key-path-{}", App::now_millis()));
    std::env::set_var("ALEPH_CONFIG_DIR", &config_dir);

    assert_eq!(
        App::openrouter_api_key_path(),
        config_dir.join("openrouter-api-key")
    );

    match original {
        Some(value) => std::env::set_var("ALEPH_CONFIG_DIR", value),
        None => std::env::remove_var("ALEPH_CONFIG_DIR"),
    }
}

#[test]
fn openrouter_key_fallback_persists_with_private_permissions() {
    let _guard = env_lock();
    let original = std::env::var_os("ALEPH_CONFIG_DIR");
    let config_dir = std::env::temp_dir().join(format!(
        "aleph-openrouter-key-fallback-{}",
        App::now_millis()
    ));
    std::env::set_var("ALEPH_CONFIG_DIR", &config_dir);

    App::store_openrouter_api_key_fallback("sk-or-v1-test").unwrap();
    let key_path = App::openrouter_api_key_path();
    assert_eq!(fs::read_to_string(&key_path).unwrap(), "sk-or-v1-test");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(&key_path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }

    let _ = fs::remove_dir_all(&config_dir);
    match original {
        Some(value) => std::env::set_var("ALEPH_CONFIG_DIR", value),
        None => std::env::remove_var("ALEPH_CONFIG_DIR"),
    }
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
        strix_sync_pending: false,
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

fn seed_test_rooms(app: &mut App) {
    app.rooms = vec![
        Room {
            name: String::from("aleph-dev"),
            project_paths: vec![String::from("/workspace/aleph")],
            tags: vec![String::from("aleph")],
            filters: vec![String::from("note")],
            accent: [136, 129, 176],
        },
        Room {
            name: String::from("strix-core"),
            project_paths: vec![String::from("/workspace/strix")],
            tags: vec![String::from("strix")],
            filters: vec![String::from("gateway")],
            accent: [106, 163, 178],
        },
    ];
    app.active_room_index = 0;
}

#[test]
fn strix_auth_base_defaults_to_production() {
    assert_eq!(
        App::normalized_strix_auth_base_url(None, None),
        "https://strix.page"
    );
}

#[test]
fn strix_auth_base_ignores_legacy_localhost_override() {
    assert_eq!(
        App::normalized_strix_auth_base_url(Some("http://localhost:3000"), None),
        "https://strix.page"
    );
    assert_eq!(
        App::normalized_strix_auth_base_url(Some("http://127.0.0.1:3000/"), None),
        "https://strix.page"
    );
}

#[test]
fn strix_auth_base_allows_explicit_local_dev_override() {
    assert_eq!(
        App::normalized_strix_auth_base_url(
            Some("https://strix.page"),
            Some("http://localhost:3000/")
        ),
        "http://localhost:3000"
    );
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
    assert_eq!(app.active_room_label(), "All");
    assert_eq!(app.room_scope_summary(), "all notes · no room filter");
}

#[test]
fn unpaired_note_list_uses_tree_panel_for_local_notes() {
    let mut app = App::new();

    app.open_note_list_panel();

    assert_eq!(app.panel_lines.len(), 2);
    assert!(app.panel_title.contains("Space expand/collapse"));
    assert!(app.panel_lines[0].contains("Uncategorized"));
    assert!(app.panel_lines[1].contains("Welcome"));
    assert_eq!(app.note_list_indices()[0], usize::MAX);
    assert_eq!(app.note_list_indices()[1], 0);
}

#[test]
fn room_list_command_opens_interactive_panel() {
    let mut app = App::new();
    seed_test_rooms(&mut app);

    for character in "/room list".chars() {
        app.handle_key(press(KeyCode::Char(character)));
    }
    app.handle_key(press(KeyCode::Enter));

    assert!(app.is_room_list());
    assert_eq!(app.panel_lines.len(), 2);
}

#[test]
fn room_list_enter_switches_selected_room() {
    let mut app = App::new();
    seed_test_rooms(&mut app);

    app.open_room_list_panel();
    app.handle_room_list_key(press(KeyCode::Down));
    app.handle_room_list_key(press(KeyCode::Enter));

    assert_eq!(app.active_room_label(), "strix-core");
    assert!(app.is_room_list());
    assert_eq!(app.room_list_selected(), 1);
}

#[test]
fn room_list_delete_requires_second_press() {
    let mut app = App::new();
    seed_test_rooms(&mut app);

    app.open_room_list_panel();
    app.handle_room_list_key(press(KeyCode::Delete));

    assert_eq!(app.rooms.len(), 2);
    assert!(app.room_list_delete_is_pending());

    app.handle_room_list_key(press(KeyCode::Delete));

    assert_eq!(app.rooms.len(), 1);
    assert_eq!(app.active_room_label(), "strix-core");
    assert!(!app.room_list_delete_is_pending());
}

#[test]
fn room_list_cannot_delete_all_scope() {
    let mut app = App::new();
    app.open_room_list_panel();

    app.handle_room_list_key(press(KeyCode::Delete));

    assert_eq!(app.active_room_label(), "All");
    assert!(app.rooms.iter().any(|room| room.name == "All"));
    assert_eq!(
        app.last_action(),
        "All is the default scope and cannot be deleted."
    );
}

#[test]
fn clear_notes_is_hidden_from_command_list() {
    assert!(COMMANDS.iter().all(|command| command.name != "clear-notes"));
}

#[test]
fn rooms_is_listed_as_its_own_command() {
    assert!(COMMANDS.iter().any(|command| command.name == "rooms"));
    assert!(COMMANDS.iter().any(|command| command.name == "room"));
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
fn slash_only_command_list_includes_rooms() {
    let mut app = App::new();

    app.handle_key(press(KeyCode::Char('/')));

    let suggestions = app.visible_commands(16);
    assert!(suggestions.iter().any(|command| command.name == "rooms"));
    assert!(suggestions.iter().any(|command| command.name == "room"));
}

#[test]
fn slash_command_window_shows_room_on_first_page() {
    let mut app = App::new();

    app.handle_key(press(KeyCode::Char('/')));

    let (suggestions, offset) = app.visible_commands_window(8);
    assert_eq!(offset, 0);
    assert!(suggestions.iter().any(|command| command.name == "room"));
}

#[test]
fn typing_a_fresh_slash_resets_command_selection() {
    let mut app = App::new();

    app.selected_suggestion = 12;
    assert!(app.selected_suggestion() > 0);

    app.handle_key(press(KeyCode::Char('/')));

    assert_eq!(app.selected_suggestion(), 0);
    let suggestions = app.visible_commands(16);
    assert_eq!(suggestions[0].name, "login");
    assert!(suggestions.iter().any(|command| command.name == "rooms"));
}

#[test]
fn autocomplete_shows_rooms_command_separately() {
    let mut app = App::new();

    for character in "/rooms".chars() {
        app.handle_key(press(KeyCode::Char(character)));
    }

    let suggestions = app.visible_commands(16);
    assert!(!suggestions.is_empty());
    assert_eq!(suggestions[0].name, "rooms");
}

#[test]
fn typing_room_prefers_room_over_rooms_alias() {
    let mut app = App::new();

    for character in "/room".chars() {
        app.handle_key(press(KeyCode::Char(character)));
    }

    let suggestions = app.visible_commands(16);
    assert!(!suggestions.is_empty());
    assert_eq!(suggestions[0].name, "room");
}

#[test]
fn typing_room_target_keeps_room_command_visible() {
    let mut app = App::new();

    for character in "/room strix-core".chars() {
        app.handle_key(press(KeyCode::Char(character)));
    }

    let suggestions = app.visible_commands(16);
    assert!(!suggestions.is_empty());
    assert_eq!(suggestions[0].name, "room");
}

#[test]
fn enter_on_command_family_drills_into_subcommands() {
    let mut app = App::new();

    for character in "/note".chars() {
        app.handle_key(press(KeyCode::Char(character)));
    }
    app.handle_key(press(KeyCode::Enter));

    assert_eq!(app.prompt(), "/note ");
    let suggestions = app.visible_commands(16);
    assert!(!suggestions.is_empty());
    assert!(suggestions
        .iter()
        .all(|command| command.name.starts_with("note ")));
    assert!(suggestions
        .iter()
        .any(|command| command.name == "note list"));
    assert!(suggestions
        .iter()
        .any(|command| command.name == "note create"));
}

#[test]
fn typing_command_family_space_shows_subcommands_only() {
    let mut app = App::new();

    for character in "/note ".chars() {
        app.handle_key(press(KeyCode::Char(character)));
    }

    let suggestions = app.visible_commands(16);
    assert!(!suggestions.is_empty());
    assert!(suggestions
        .iter()
        .all(|command| command.name.starts_with("note ")));
    assert!(!suggestions.iter().any(|command| command.name == "note"));
}

#[test]
fn exact_command_family_is_one_focused_navigation_choice() {
    let mut app = App::new();

    for character in "/note".chars() {
        app.handle_key(press(KeyCode::Char(character)));
    }

    let suggestions = app.visible_commands(16);
    assert_eq!(
        suggestions
            .iter()
            .map(|command| command.name)
            .collect::<Vec<_>>(),
        vec!["note"]
    );
    assert_eq!(
        App::command_subcommand_summary(suggestions[0]).as_deref(),
        Some("Enter to open 6 actions")
    );
}

#[test]
fn command_family_browser_uses_short_action_labels() {
    let mut app = App::new();

    for character in "/note ".chars() {
        app.handle_key(press(KeyCode::Char(character)));
    }

    assert_eq!(app.command_browse_family().as_deref(), Some("note"));
    let suggestions = app.visible_commands(16);
    assert_eq!(app.command_palette_label(suggestions[0]), "list");
}

#[test]
fn every_command_family_expands_to_only_its_own_actions() {
    let family_names = COMMANDS
        .iter()
        .filter(|command| {
            !command.name.contains(char::is_whitespace) && App::command_family_expands(command.name)
        })
        .map(|command| command.name)
        .collect::<Vec<_>>();

    for family in family_names {
        let mut app = App::new();
        for character in format!("/{}", family).chars() {
            app.handle_key(press(KeyCode::Char(character)));
        }
        assert_eq!(app.visible_commands(16).len(), 1, "family: {family}");

        app.handle_key(press(KeyCode::Enter));
        let suggestions = app.visible_commands(32);
        assert!(!suggestions.is_empty(), "family: {family}");
        assert!(
            suggestions
                .iter()
                .all(|command| command.name.starts_with(&format!("{} ", family))),
            "family: {family}"
        );
    }
}

#[test]
fn exact_leaf_command_does_not_show_its_parent() {
    let mut app = App::new();
    for character in "/note list".chars() {
        app.handle_key(press(KeyCode::Char(character)));
    }

    let suggestions = app.visible_commands(16);
    assert_eq!(
        suggestions
            .iter()
            .map(|command| command.name)
            .collect::<Vec<_>>(),
        vec!["note list"]
    );
    assert_eq!(app.command_browse_family().as_deref(), Some("note"));
    assert_eq!(app.command_palette_label(suggestions[0]), "list");
}

#[test]
fn arrow_navigation_keeps_the_expanded_family_context() {
    let mut app = App::new();
    for character in "/note".chars() {
        app.handle_key(press(KeyCode::Char(character)));
    }
    app.handle_key(press(KeyCode::Enter));
    app.handle_key(press(KeyCode::Down));

    assert_eq!(app.command_browse_family().as_deref(), Some("note"));
    let suggestions = app.visible_commands(16);
    assert!(suggestions
        .iter()
        .all(|command| command.name.starts_with("note ")));
    assert!(suggestions
        .iter()
        .all(|command| !app.command_palette_label(command).starts_with('/')));
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
fn enter_executes_room_name_as_switch() {
    let mut app = App::new();
    seed_test_rooms(&mut app);

    for character in "/room strix-core".chars() {
        app.handle_key(press(KeyCode::Char(character)));
    }
    app.handle_key(press(KeyCode::Enter));

    assert_eq!(app.active_room_label(), "strix-core");
    assert_eq!(app.last_action(), "Switched to room strix-core.");
    assert_eq!(app.panel_title(), "Room: strix-core");
}

#[test]
fn cli_room_name_switches_room_directly() {
    let mut app = App::new();
    seed_test_rooms(&mut app);

    let lines = app
        .run_cli_command(&[String::from("room"), String::from("strix-core")])
        .unwrap();

    assert_eq!(app.active_room_label(), "strix-core");
    assert!(lines.iter().any(|line| line.contains("Room: strix-core")));
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
    app.ai_provider = AiProvider::Strix;
    app.store_ai_provider().unwrap();
    app.agent_mode_enabled = false;
    app.store_agent_mode_enabled().unwrap();
    app.agent_context_scope = AgentContextScope::Global;
    app.store_agent_context_scope().unwrap();
    app.editor_images_enabled = true;
    app.store_editor_images_enabled().unwrap();

    assert_eq!(App::load_ai_provider(), Some(AiProvider::Strix));
    assert_eq!(App::load_agent_mode_enabled(), Some(false));
    assert_eq!(
        App::load_agent_context_scope(),
        Some(AgentContextScope::Global)
    );
    assert_eq!(App::load_editor_images_enabled(), Some(true));

    std::env::remove_var("ALEPH_CONFIG_DIR");
    let _ = fs::remove_dir_all(config_dir);
}

#[test]
fn rooms_round_trip_to_config() {
    let _guard = env_lock();
    let config_dir = std::env::temp_dir().join(format!("aleph-rooms-test-{}", App::now_millis()));
    std::env::set_var("ALEPH_CONFIG_DIR", &config_dir);

    let mut app = App::new();
    seed_test_rooms(&mut app);
    app.active_room_index = 1;
    app.save_room_state().unwrap();

    let (rooms, active_room_index) = App::load_room_state().unwrap();
    assert_eq!(rooms.len(), 3);
    assert_eq!(rooms[0].name, "All");
    assert_eq!(rooms[2].name, "strix-core");
    assert_eq!(active_room_index, 2);

    std::env::remove_var("ALEPH_CONFIG_DIR");
    let _ = fs::remove_dir_all(config_dir);
}

#[test]
fn room_scope_filters_notes_and_memories() {
    let mut app = App::new();
    seed_test_rooms(&mut app);
    app.notes = vec![
        test_note(1, None, "Aleph overview", "terminal notes"),
        test_note(2, None, "Strix gateway", "gateway notes"),
    ];
    app.memories = vec![
        String::from("Aleph workspace memory"),
        String::from("Strix gateway memory"),
    ];

    app.switch_room_by_name("strix-core").unwrap();

    let search_results = app.search_notes("gateway");
    assert_eq!(search_results.len(), 1);
    assert!(search_results[0].contains("Strix gateway"));

    let memory_results = app
        .memories
        .iter()
        .filter(|memory| app.room_matches_memory(memory))
        .collect::<Vec<_>>();
    assert_eq!(memory_results.len(), 1);
    assert!(memory_results[0].contains("Strix gateway"));
}

#[test]
fn all_scope_does_not_filter_notes_or_memories() {
    let mut app = App::new();
    seed_test_rooms(&mut app);
    app.rooms.insert(0, App::global_room());
    app.active_room_index = 0;
    app.notes = vec![
        test_note(1, None, "Aleph overview", "terminal notes"),
        test_note(2, None, "Strix gateway", "gateway notes"),
    ];
    app.memories = vec![
        String::from("Aleph workspace memory"),
        String::from("Strix gateway memory"),
    ];

    assert_eq!(app.search_notes("").len(), 2);
    assert_eq!(
        app.memories
            .iter()
            .filter(|memory| app.room_matches_memory(memory))
            .count(),
        2
    );
}

#[test]
fn room_all_returns_to_unfiltered_scope() {
    let mut app = App::new();
    seed_test_rooms(&mut app);
    app.rooms.insert(0, App::global_room());
    app.active_room_index = 2;

    for character in "/room all".chars() {
        app.handle_key(press(KeyCode::Char(character)));
    }
    app.handle_key(press(KeyCode::Enter));

    assert_eq!(app.active_room_label(), "All");
    assert_eq!(app.room_scope_summary(), "all notes · no room filter");
}

#[test]
fn switching_rooms_reconciles_hidden_note_selection() {
    let mut app = App::new();
    seed_test_rooms(&mut app);
    app.notes = vec![
        test_note(1, None, "Aleph overview", "terminal notes"),
        test_note(2, None, "Strix gateway", "gateway notes"),
    ];
    app.selected_note = 0;

    app.switch_room_by_name("strix-core").unwrap();

    assert_eq!(app.selected_note, 1);
    assert!(app.resolve_note_index("Aleph overview").is_none());
    assert!(app.resolve_note_index("Strix gateway").is_some());
}

#[test]
fn empty_room_set_uses_safe_fallback_room() {
    let mut app = App::new();
    app.rooms.clear();
    app.active_room_index = 99;

    assert!(!app.active_room_label().is_empty());
    assert!(!app.room_scope_summary().is_empty());
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
    for _ in 0..6 {
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
            row: 26,
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
    assert_eq!(App::settings_index_for_mouse_row(40, 80, 25), Some(5));
    assert_eq!(App::settings_index_for_mouse_row(40, 80, 26), Some(6));
    assert_eq!(App::settings_index_for_mouse_row(40, 80, 19), None);

    assert_eq!(App::settings_index_for_mouse_row(22, 80, 20), None);
    assert_eq!(App::settings_index_for_mouse_row(22, 80, 25), None);
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
    assert!(!app
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
fn cli_path_commands_use_temporal_forks() {
    let _guard = env_lock();
    let root = std::env::temp_dir().join(format!("aleph-forks-cli-test-{}", App::now_millis()));
    let forks_path = root.join("temporal-forks.json");
    let notes_path = root.join("notes.json");
    let trail_path = root.join("trail.jsonl");
    fs::create_dir_all(&root).unwrap();
    std::env::set_var("ALEPH_FORKS_PATH", &forks_path);
    std::env::set_var("ALEPH_NOTES_PATH", &notes_path);
    std::env::set_var("ALEPH_TRAIL_PATH", &trail_path);

    let mut app = App::new();
    seed_test_notes(&mut app);

    let saved = app
        .run_cli_command(&[
            String::from("path"),
            String::from("save"),
            String::from("cli"),
            String::from("path"),
        ])
        .unwrap();
    assert!(saved.iter().any(|line| line.contains("cli path")));

    let listed = app
        .run_cli_command(&[String::from("path"), String::from("list")])
        .unwrap();
    assert!(listed.iter().any(|line| line.contains("cli path")));

    let shown = app
        .run_cli_command(&[
            String::from("path"),
            String::from("show"),
            String::from("cli"),
            String::from("path"),
        ])
        .unwrap();
    assert!(shown.iter().any(|line| line.contains("Path: cli path")));

    std::env::remove_var("ALEPH_FORKS_PATH");
    std::env::remove_var("ALEPH_NOTES_PATH");
    std::env::remove_var("ALEPH_TRAIL_PATH");
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
fn note_list_delete_preserves_folder_markers_when_reindexing() {
    let mut app = App::new();
    seed_test_notes(&mut app);

    app.open_note_list_panel();
    assert_eq!(app.note_list_indices()[0], usize::MAX);

    app.delete_note_at_index(0).unwrap();

    assert_eq!(app.note_list_indices()[0], usize::MAX);
    assert_eq!(app.note_list_indices()[1], 0);
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
    app.chat_composer
        .set_text("write a note about launch planning");

    app.handle_chat_key(press(KeyCode::Enter));
    advance_execution(&mut app, 20);

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
fn agent_mode_does_not_route_general_write_prompt_to_note_draft() {
    let mut app = App::new();
    app.openrouter_api_key = None;
    app.strix_access_token = None;
    app.refresh_connection_state();
    app.panel_mode = PanelMode::AiChat;
    app.chat_composer
        .set_text("write an outline about moat strategy");

    app.handle_chat_key(press(KeyCode::Enter));
    advance_execution(&mut app, 20);

    assert!(app.is_ai_chat());
    assert!(app.pending_agent_decision.is_none());
    assert_eq!(app.chat_messages().len(), 2);
    assert!(!app.is_full_editor());
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
    app.chat_composer.set_text("make this more concise");

    app.handle_chat_key(press(KeyCode::Enter));
    advance_execution(&mut app, 20);

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
    app.chat_composer
        .set_text("work on the existing note and make progress");

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
    app.chat_composer
        .set_text("work on Feature ideas and make it sharper");

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
    app.chat_composer.set_text("read Feature ideas");

    app.handle_chat_key(press(KeyCode::Enter));
    advance_execution(&mut app, 20);

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
    app.chat_composer.set_text("find notes about gateway");

    app.handle_chat_key(press(KeyCode::Enter));
    advance_execution(&mut app, 20);

    assert!(app.is_ai_chat());
    assert!(app.pending_agent_decision.is_none());
    assert_eq!(app.chat_messages().len(), 2);
    assert!(app.chat_messages()[1].content.contains("Strix gateway"));
    assert!(!app.chat_messages()[1].content.contains("Result."));
    assert!(!app.chat_messages()[1].content.contains("Path:"));
    assert!(!app.chat_messages()[1].content.contains("Steps run:"));
}

#[test]
fn agent_mode_followup_search_uses_recent_chat_context() {
    let mut app = App::new();
    app.notes = vec![
        test_note(1, None, "Strix gateway", "gateway notes"),
        test_note(2, None, "Garden", "seed notes"),
    ];
    app.openrouter_api_key = None;
    app.strix_access_token = None;
    app.refresh_connection_state();
    app.panel_mode = PanelMode::AiChat;
    app.push_chat_message("user", "I was looking for notes about gateway");
    app.chat_composer.set_text("find that");

    app.handle_chat_key(press(KeyCode::Enter));
    advance_execution(&mut app, 20);

    assert!(app.is_ai_chat());
    assert!(app.pending_agent_decision.is_none());
    assert!(app
        .chat_messages()
        .last()
        .unwrap()
        .content
        .contains("Strix gateway"));
}

#[test]
fn greeting_before_note_search_does_not_force_smalltalk() {
    let app = App::new();
    let decision = app.plan_agent_action_locally("hey find notes about gateway");

    assert!(matches!(decision.action, AgentAction::SearchNotes));
    assert_eq!(decision.search_query.as_deref(), Some("gateway"));
}

#[test]
fn workspace_request_uses_local_agent_steps_without_provider() {
    let mut app = App::new();
    app.openrouter_api_key = None;
    app.strix_access_token = None;
    app.refresh_connection_state();
    app.panel_mode = PanelMode::AiChat;
    app.chat_composer
        .set_text("inspect the current workspace status");

    app.handle_chat_key(press(KeyCode::Enter));
    advance_execution(&mut app, 30);

    let answer = &app.chat_messages().last().unwrap().content;
    assert!(answer.contains("- agent context: Current folder"));
    assert!(answer.contains("Daemon"));
    assert!(!answer.contains("Running step"));
    assert!(!answer.contains("Next useful move"));
}

#[test]
fn note_write_permission_prompt_stays_explicit_but_natural() {
    let mut app = App::new();
    app.openrouter_api_key = None;
    app.strix_access_token = None;
    app.refresh_connection_state();
    app.panel_mode = PanelMode::AiChat;
    app.chat_composer
        .set_text("write a note about launch planning");

    app.handle_chat_key(press(KeyCode::Enter));

    let prompt = &app.chat_messages()[1].content;
    assert!(prompt.contains("Press Enter to approve"));
    assert!(prompt.contains("type `no`"));
    assert!(!prompt.contains("using the note-writing agent"));
}

#[test]
fn agent_context_scope_can_skip_current_folder_context() {
    let mut app = App::new();
    app.agent_context_scope = AgentContextScope::Global;

    let lines = app.workspace_context_lines().join("\n");

    assert!(lines.contains("- agent context: Global"));
    assert!(lines.contains("- git: skipped"));
    assert!(!lines.contains("- cwd:"));
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
    app.chat_composer
        .set_text("find a note that I have on advice from steve jobs and peter thiel");

    app.handle_chat_key(press(KeyCode::Enter));
    advance_execution(&mut app, 20);

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
    app.chat_composer.set_text("go through memories");

    app.handle_chat_key(press(KeyCode::Enter));
    advance_execution(&mut app, 20);

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
    app.chat_composer
        .set_text("how do I write a note about launch planning?");

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
    app.chat_composer
        .set_text("write a note about launch planning");

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
fn editor_search_handles_unicode_matches_on_character_boundaries() {
    let mut app = App::new();
    app.editor_buffer = String::from("éé É");
    app.search_state.query = String::from("é");

    app.update_search();

    assert_eq!(app.search_state.matches, vec![0, "é".len(), "éé ".len()]);
    assert!(app
        .search_state
        .matches
        .iter()
        .all(|&index| app.editor_buffer.is_char_boundary(index)));
}

#[test]
fn hash_note_reference_uses_persistent_id() {
    let mut app = App::new();
    app.notes = vec![
        test_note(2, None, "First", "one"),
        test_note(7, None, "Second", "two"),
    ];

    assert_eq!(app.resolve_note_index("#7"), Some(1));
    assert_eq!(app.resolve_note_index("#2"), Some(0));
    assert_eq!(app.resolve_note_index("2"), Some(1));
}

#[test]
fn pending_ai_edit_blocks_editor_mutations() {
    let mut app = App::new();
    app.open_note_editor(0);
    app.ai_overlay_visible = true;
    app.insert_editor_character('!');
    let edited = app.editor_buffer.clone();
    app.pending_ai_edit = Some(AiEditProposal {
        note_index: Some(0),
        title: None,
        instruction: String::from("replace"),
        proposed: String::from("replacement"),
        diff_lines: Vec::new(),
    });

    app.handle_key(ctrl(KeyCode::Char('z')));

    assert_eq!(app.editor_buffer, edited);
    assert!(app.has_pending_ai_edit());
    assert_eq!(
        app.last_action,
        "Apply or reject the pending AI edits first."
    );
}

#[test]
fn failed_save_keeps_editor_open() {
    let root = std::env::temp_dir().join(format!("aleph-save-failure-{}", App::now_millis()));
    fs::create_dir_all(&root).unwrap();

    let mut app = App::new();
    seed_test_notes(&mut app);
    app.note_save_target = NoteSaveTarget::Local;
    app.notes[0].obsidian_path = Some(root.clone());
    app.open_note_editor(0);
    app.insert_editor_character('!');
    app.exit_editor();

    assert!(matches!(
        app.editor_save_status,
        EditorSaveStatus::Failed(_)
    ));
    assert_eq!(app.editor_note_index, Some(0));
    assert!(app.panel_mode == PanelMode::FullEditor);
    assert!(app.last_action.starts_with("Note save failed:"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn editor_save_feedback_moves_from_saving_to_saved() {
    let _guard = env_lock();
    let root = std::env::temp_dir().join(format!("aleph-save-feedback-{}", App::now_millis()));
    let notes_path = root.join("notes.json");
    fs::create_dir_all(&root).unwrap();
    std::env::set_var("ALEPH_NOTES_PATH", &notes_path);

    let mut app = App::new();
    seed_test_notes(&mut app);
    app.note_save_target = NoteSaveTarget::Local;
    app.open_note_editor(0);
    app.insert_editor_character('!');
    assert_eq!(app.editor_save_status, EditorSaveStatus::Unsaved);

    app.save_editor();
    assert_eq!(app.editor_save_status, EditorSaveStatus::Saved);
    assert_eq!(app.save_shimmer_ticks, 18);
    for _ in 0..18 {
        app.on_tick();
    }
    assert_eq!(app.save_shimmer_ticks, 0);

    std::env::remove_var("ALEPH_NOTES_PATH");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn exiting_an_unchanged_editor_skips_persistence() {
    let _guard = env_lock();
    let root = std::env::temp_dir().join(format!("aleph-clean-exit-{}", App::now_millis()));
    let forks_path = root.join("temporal-forks.json");
    let notes_path = root.join("notes.json");
    fs::create_dir_all(&root).unwrap();
    std::env::set_var("ALEPH_FORKS_PATH", &forks_path);
    std::env::set_var("ALEPH_NOTES_PATH", &notes_path);

    let mut app = App::new();
    seed_test_notes(&mut app);
    app.note_save_target = NoteSaveTarget::Local;
    app.open_note_editor(0);
    app.exit_editor();

    assert!(!app.is_full_editor());
    assert!(app.temporal_forks.is_empty());
    assert!(!forks_path.exists());
    assert_eq!(app.editor_save_status, EditorSaveStatus::Clean);
    assert_eq!(app.save_shimmer_ticks, 0);

    std::env::remove_var("ALEPH_FORKS_PATH");
    std::env::remove_var("ALEPH_NOTES_PATH");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn exiting_a_large_note_builds_only_a_compact_preview() {
    let _guard = env_lock();
    let root = std::env::temp_dir().join(format!("aleph-fast-exit-{}", App::now_millis()));
    fs::create_dir_all(&root).unwrap();
    std::env::set_var("ALEPH_NOTES_PATH", root.join("notes.json"));

    let mut app = App::new();
    app.note_save_target = NoteSaveTarget::Local;
    app.notes[0].content = (0..500)
        .map(|line| format!("line {line}"))
        .collect::<Vec<_>>()
        .join("\n");
    app.open_note_editor(0);
    app.exit_editor();

    assert!(app.panel_lines().len() <= 25);
    assert!(app
        .panel_lines()
        .last()
        .is_some_and(|line| line.contains("more lines")));

    std::env::remove_var("ALEPH_NOTES_PATH");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn edited_note_drops_stale_duplicate_raw_content() {
    let _guard = env_lock();
    let root = std::env::temp_dir().join(format!("aleph-raw-content-{}", App::now_millis()));
    fs::create_dir_all(&root).unwrap();
    std::env::set_var("ALEPH_NOTES_PATH", root.join("notes.json"));

    let mut app = App::new();
    app.note_save_target = NoteSaveTarget::Local;
    app.notes[0].raw_content = String::from("<p>old remote HTML</p>");
    app.open_note_editor(0);
    app.insert_editor_text(" updated");
    app.save_editor();

    assert!(app.notes[0].raw_content.is_empty());
    assert!(app.temporal_forks.is_empty());

    std::env::remove_var("ALEPH_NOTES_PATH");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn repeated_strix_saves_coalesce_to_the_latest_note() {
    let mut app = App::new();
    app.strix_access_token = Some(String::from("test-token"));
    app.refresh_connection_state();
    let note_id = app.notes[0].id;
    app.note_sync_in_flight.insert(note_id);
    app.notes[0].content = String::from("latest local content");

    app.queue_strix_note_sync(0).unwrap();

    assert_eq!(app.note_sync_queued.len(), 1);
    assert_eq!(
        app.note_sync_queued
            .get(&note_id)
            .map(|note| note.content.as_str()),
        Some("latest local content")
    );
}

#[test]
fn background_strix_result_never_overwrites_newer_local_content() {
    let _guard = env_lock();
    let root = std::env::temp_dir().join(format!("aleph-sync-result-{}", App::now_millis()));
    fs::create_dir_all(&root).unwrap();
    std::env::set_var("ALEPH_NOTES_PATH", root.join("notes.json"));

    let mut app = App::new();
    let note_id = app.notes[0].id;
    app.notes[0].content = String::from("newer local content");
    app.notes[0].strix_sync_pending = true;
    app.note_sync_in_flight.insert(note_id);
    let mut synced = app.notes[0].clone();
    synced.remote_id = Some(String::from("remote-note"));
    synced.content = String::from("older sent content");
    synced.raw_content = String::from("<p>older sent content</p>");

    app.note_sync_tx
        .send(NoteSyncUpdate {
            local_id: note_id,
            sent_content: String::from("older sent content"),
            result: Ok(synced),
        })
        .unwrap();
    app.process_note_sync_updates();

    assert_eq!(app.notes[0].content, "newer local content");
    assert_eq!(app.notes[0].remote_id.as_deref(), Some("remote-note"));
    assert_ne!(app.notes[0].raw_content, "<p>older sent content</p>");
    assert!(app.notes[0].strix_sync_pending);
    assert!(!app.note_sync_in_flight.contains(&note_id));

    std::env::remove_var("ALEPH_NOTES_PATH");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn strix_refresh_preserves_a_durable_pending_local_edit() {
    let mut app = App::new();
    let mut local = test_note(1, Some("remote-1"), "Local title", "unsynced local body");
    local.strix_sync_pending = true;
    app.notes = vec![local];

    app.merge_strix_notes(vec![test_note(
        9,
        Some("remote-1"),
        "Older remote title",
        "older remote body",
    )]);

    assert_eq!(app.notes[0].title, "Local title");
    assert_eq!(app.notes[0].content, "unsynced local body");
    assert!(app.notes[0].strix_sync_pending);
}

#[test]
fn typing_replaces_utf8_selection_and_remains_undoable() {
    let mut app = App::new();
    app.editor_buffer = String::from("AéBC");
    app.editor_cursor = "Aé".len();
    app.editor_selection = Selection {
        start: 1,
        end: "Aé".len(),
        active: true,
    };

    app.insert_editor_text("x");
    assert_eq!(app.editor_buffer, "AxBC");
    assert_eq!(app.editor_cursor, 2);
    assert_eq!(app.editor_save_status, EditorSaveStatus::Unsaved);

    app.undo();
    assert_eq!(app.editor_buffer, "AéBC");
}

#[test]
fn editor_click_position_maps_terminal_cells_to_utf8_offsets() {
    let mut app = App::new();
    app.editor_buffer = String::from("éclair\nsecond");
    app.editor_cursor = 0;
    let area = ratatui::prelude::Rect::new(0, 0, 100, 30);

    assert_eq!(crate::ui::editor_position_at(&app, area, 12, 3), Some(2));
    assert_eq!(crate::ui::editor_position_at(&app, area, 14, 4), Some(11));
    assert_eq!(crate::ui::editor_position_at(&app, area, 0, 0), None);
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

    assert_eq!(text[0], "## Name");
    assert_eq!(text[1], "Alpha");
    assert_eq!(text[2], "## Count");
    assert_eq!(text[3], "2");
    assert_eq!(text[4], "");
    assert_eq!(text[5], "## Name");
    assert_eq!(text[6], "Beta project");
    assert_eq!(text[7], "## Count");
    assert_eq!(text[8], "14");
}

#[test]
fn smalltalk_routes_directly_to_chat() {
    let app = App::new();
    let decision = app.plan_agent_action_locally("How are you?");

    if let AgentAction::Chat = decision.action {
    } else {
        panic!("smalltalk should route to chat");
    }
    assert_eq!(decision.rationale, "smalltalk");
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

#[test]
fn agent_memory_write_waits_for_explicit_approval() {
    let mut app = App::new();
    app.openrouter_api_key = None;
    app.strix_access_token = None;
    app.refresh_connection_state();
    app.panel_mode = PanelMode::AiChat;
    app.chat_composer
        .set_text("remember that command families should stay compact");

    app.handle_chat_key(press(KeyCode::Enter));

    assert!(app.pending_agent_decision.is_some());
    assert!(app.memories.is_empty());
    assert!(app
        .chat_messages()
        .last()
        .is_some_and(|message| message.content.contains("Press Enter to approve")));
}

#[test]
fn trail_event_persists_schema_workspace_signals_and_importance() {
    let _guard = env_lock();
    let root = std::env::temp_dir().join(format!("aleph-trail-test-{}", App::now_millis()));
    fs::create_dir_all(&root).unwrap();
    let previous_config_dir = std::env::var_os("ALEPH_CONFIG_DIR");
    std::env::set_var("ALEPH_CONFIG_DIR", &root);

    let app = App::new();
    app.append_trail_event(
        "path",
        "Saved decision point: before MCP Strix memory refactor.",
        vec![String::from("fork-123")],
        TrailImportance::High,
    )
    .unwrap();

    let body = fs::read_to_string(App::trail_path()).unwrap();
    let event: serde_json::Value = serde_json::from_str(body.lines().next().unwrap()).unwrap();
    assert_eq!(event["schema_version"], 1);
    assert!(event["workspace_id"].as_str().unwrap().len() >= 12);
    assert_eq!(event["kind"], "path");
    assert_eq!(event["importance"], "high");
    assert!(event["signals"]
        .as_array()
        .unwrap()
        .iter()
        .any(|signal| signal == "mcp"));
    assert!(event["signals"]
        .as_array()
        .unwrap()
        .iter()
        .any(|signal| signal == "strix"));

    if let Some(previous) = previous_config_dir {
        std::env::set_var("ALEPH_CONFIG_DIR", previous);
    } else {
        std::env::remove_var("ALEPH_CONFIG_DIR");
    }
    let _ = fs::remove_dir_all(root);
}

#[test]
fn trail_panel_surfaces_recurring_signals() {
    let _guard = env_lock();
    let root = std::env::temp_dir().join(format!("aleph-trail-panel-test-{}", App::now_millis()));
    fs::create_dir_all(&root).unwrap();
    let previous_config_dir = std::env::var_os("ALEPH_CONFIG_DIR");
    std::env::set_var("ALEPH_CONFIG_DIR", &root);

    let app = App::new();
    app.append_trail_event(
        "agent",
        "Clarified MCP and Strix memory boundaries.",
        Vec::new(),
        TrailImportance::Normal,
    )
    .unwrap();
    app.append_trail_event(
        "path",
        "Saved path for MCP and Darwin secrecy thinking.",
        Vec::new(),
        TrailImportance::High,
    )
    .unwrap();

    let lines = app.trail_lines(None);
    assert!(lines.iter().any(|line| line == "Recurring signals:"));
    assert!(lines.iter().any(|line| line == "- mcp"));
    assert!(lines.iter().any(|line| line.contains("Recent turns:")));

    if let Some(previous) = previous_config_dir {
        std::env::set_var("ALEPH_CONFIG_DIR", previous);
    } else {
        std::env::remove_var("ALEPH_CONFIG_DIR");
    }
    let _ = fs::remove_dir_all(root);
}

#[test]
fn trail_command_opens_panel_without_expanding_subcommands() {
    let _guard = env_lock();
    let root = std::env::temp_dir().join(format!("aleph-trail-command-test-{}", App::now_millis()));
    fs::create_dir_all(&root).unwrap();
    let previous_config_dir = std::env::var_os("ALEPH_CONFIG_DIR");
    std::env::set_var("ALEPH_CONFIG_DIR", &root);

    let mut app = App::new();
    for character in "/trail".chars() {
        app.handle_key(press(KeyCode::Char(character)));
    }
    app.handle_key(press(KeyCode::Enter));

    assert_eq!(app.panel_title(), "Trail");
    assert_eq!(app.last_action(), "Opened Aleph Trail.");
    assert_eq!(app.prompt(), "");

    if let Some(previous) = previous_config_dir {
        std::env::set_var("ALEPH_CONFIG_DIR", previous);
    } else {
        std::env::remove_var("ALEPH_CONFIG_DIR");
    }
    let _ = fs::remove_dir_all(root);
}

#[test]
fn daemon_status_reports_missing_state() {
    let _guard = env_lock();
    let root = std::env::temp_dir().join(format!("aleph-daemon-status-test-{}", App::now_millis()));
    fs::create_dir_all(&root).unwrap();
    let previous_config_dir = std::env::var_os("ALEPH_CONFIG_DIR");
    std::env::set_var("ALEPH_CONFIG_DIR", &root);

    let app = App::new();
    let lines = app.daemon_status_lines();

    assert!(lines.iter().any(|line| line == "Daemon: not running"));
    assert!(lines
        .iter()
        .any(|line| line == "Status detail: state file not found"));

    if let Some(previous) = previous_config_dir {
        std::env::set_var("ALEPH_CONFIG_DIR", previous);
    } else {
        std::env::remove_var("ALEPH_CONFIG_DIR");
    }
    let _ = fs::remove_dir_all(root);
}

fn rendered_chat(app: &App, width: u16) -> String {
    use ratatui::{backend::TestBackend, Terminal};

    let backend = TestBackend::new(width, 64);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| crate::ui::draw(frame, app)).unwrap();
    let buffer = terminal.backend().buffer();
    (0..buffer.area.height)
        .map(|y| {
            (0..buffer.area.width)
                .filter_map(|x| buffer.cell((x, y)))
                .map(|cell| cell.symbol())
                .collect::<String>()
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn fill_scrollable_transcript(app: &mut App) {
    app.panel_mode = PanelMode::AiChat;
    for index in 0..18 {
        app.push_chat_message(
            if index % 2 == 0 { "user" } else { "assistant" },
            format!(
                "message {index} with enough wrapped content to occupy several visual rows in a narrow transcript"
            ),
        );
    }
}

#[test]
fn follow_tail_tracks_progressive_updates_and_anchored_browsing_does_not_move() {
    use ratatui::prelude::Rect;

    let mut app = App::new();
    fill_scrollable_transcript(&mut app);
    app.begin_run("inspect", RunPhase::Planning);
    app.push_chat_message("user", "inspect");
    app.queue_run_steps([
        (String::from("First pending step"), None),
        (String::from("Second pending step"), None),
    ])
    .unwrap();
    assert_eq!(
        app.transcript_viewport().mode(),
        TranscriptViewportMode::FollowTail
    );
    app.start_queued_step(0).unwrap();
    app.complete_step(0, "done").unwrap();
    assert_eq!(
        app.transcript_viewport().mode(),
        TranscriptViewportMode::FollowTail
    );

    let area = Rect::new(0, 0, 60, 20);
    app.scroll_chat_by_in_area(-8, area);
    let anchored = app.transcript_viewport().mode();
    let TranscriptViewportMode::Anchored(anchor) = anchored else {
        panic!("scrolling upward should establish a semantic anchor");
    };
    app.start_queued_step(1).unwrap();
    app.complete_step(1, "another completed step").unwrap();
    app.push_chat_message("assistant", "stream start");
    app.chat_messages
        .last_mut()
        .unwrap()
        .content
        .push_str(" and more streamed output");
    app.note_transcript_activity();
    app.chat_composer
        .set_text("one\ntwo\nthree\nfour\nfive\nsix");

    assert_eq!(app.transcript_viewport().mode(), anchored);
    assert!(app.transcript_viewport().has_new_activity());
    let narrow = crate::ui::chat_transcript_layout(&app, area);
    let wide = crate::ui::chat_transcript_layout(&app, Rect::new(0, 0, 100, 28));
    assert_eq!(
        narrow.anchor_at(narrow.resolve_anchor(anchor)).block,
        anchor.block
    );
    assert_eq!(
        wide.anchor_at(wide.resolve_anchor(anchor)).block,
        anchor.block
    );
}

#[test]
fn end_and_page_down_return_to_follow_tail_and_clear_new_activity() {
    use ratatui::prelude::Rect;

    let mut app = App::new();
    fill_scrollable_transcript(&mut app);
    let area = Rect::new(0, 0, 60, 20);
    app.scroll_chat_by_in_area(-12, area);
    app.note_transcript_activity();
    assert!(app.transcript_viewport().has_new_activity());

    app.chat_composer
        .set_interaction(ComposerInteraction::Transcript);
    app.handle_chat_key(press(KeyCode::End));
    assert_eq!(
        app.transcript_viewport().mode(),
        TranscriptViewportMode::FollowTail
    );
    assert!(!app.transcript_viewport().has_new_activity());

    app.scroll_chat_by_in_area(-12, area);
    for _ in 0..100 {
        app.scroll_chat_by_in_area(10, area);
        if matches!(
            app.transcript_viewport().mode(),
            TranscriptViewportMode::FollowTail
        ) {
            break;
        }
    }
    assert_eq!(
        app.transcript_viewport().mode(),
        TranscriptViewportMode::FollowTail
    );
}

#[test]
fn anchored_transcript_renders_new_activity_and_tiny_layouts_do_not_panic() {
    use ratatui::{backend::TestBackend, Terminal};

    let mut app = App::new();
    fill_scrollable_transcript(&mut app);
    app.scroll_chat_by_in_area(-5, ratatui::prelude::Rect::new(0, 0, 40, 12));
    app.push_chat_message("assistant", "new tail output");

    let screen = rendered_chat(&app, 60);
    assert!(screen.contains("new activity"), "{screen}");
    for (width, height) in [(1, 1), (10, 2), (20, 3), (30, 6)] {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| crate::ui::draw(frame, &app)).unwrap();
    }
}

#[test]
fn mouse_and_keyboard_scrolling_choose_consistent_viewport_modes() {
    use crossterm::event::{MouseEvent, MouseEventKind};

    let mut keyboard = App::new();
    fill_scrollable_transcript(&mut keyboard);
    let mut mouse = App::new();
    fill_scrollable_transcript(&mut mouse);

    keyboard.scroll_chat_up(1);
    mouse.handle_mouse(MouseEvent {
        kind: MouseEventKind::ScrollUp,
        column: 1,
        row: 1,
        modifiers: KeyModifiers::NONE,
    });
    assert!(matches!(
        keyboard.transcript_viewport().mode(),
        TranscriptViewportMode::Anchored(_)
    ));
    assert!(matches!(
        mouse.transcript_viewport().mode(),
        TranscriptViewportMode::Anchored(_)
    ));

    keyboard.scroll_chat_down(usize::MAX);
    mouse.handle_mouse(MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: 1,
        row: 1,
        modifiers: KeyModifiers::NONE,
    });
    assert_eq!(
        keyboard.transcript_viewport().mode(),
        TranscriptViewportMode::FollowTail
    );
}

#[test]
fn run_phase_transitions_are_guarded() {
    assert!(RunPhase::Planning.can_transition_to(RunPhase::Acting));
    assert!(RunPhase::Acting.can_transition_to(RunPhase::WaitingApproval));
    assert!(RunPhase::WaitingApproval.can_transition_to(RunPhase::Acting));
    assert!(RunPhase::Streaming.can_transition_to(RunPhase::Completed));
    assert!(!RunPhase::Completed.can_transition_to(RunPhase::Acting));

    let mut app = App::new();
    app.begin_run("inspect", RunPhase::Planning);
    app.complete_run("done").unwrap();
    assert!(app.transition_run(RunPhase::Acting).is_err());
}

#[test]
fn one_agent_request_owns_one_run_id_and_schedules_read_only_progressively() {
    let mut app = App::new();
    app.openrouter_api_key = None;
    app.strix_access_token = None;
    app.refresh_connection_state();

    assert!(app.try_start_agent_action("inspect the current workspace status"));

    assert_eq!(app.agent_runs.len(), 1);
    let run = &app.agent_runs[0];
    assert_eq!(run.phase, RunPhase::Planning);
    assert!(run.changes.is_empty());
    assert!(!run.steps.is_empty());
    assert!(run
        .steps
        .iter()
        .all(|step| step.status == StepStatus::Pending));
    assert!(app
        .chat_messages
        .iter()
        .all(|message| message.run_id == Some(run.id)));
}

fn advance_execution(app: &mut App, iterations: usize) {
    for _ in 0..iterations {
        app.on_iteration();
        if app.pending_agent_execution.is_none() {
            break;
        }
        if app
            .pending_agent_execution
            .as_ref()
            .is_some_and(|execution| execution.phase == AgentExecutionPhase::WaitingWorker)
        {
            std::thread::sleep(std::time::Duration::from_millis(1));
        } else {
            std::thread::yield_now();
        }
    }
}

#[test]
fn local_steps_expose_pending_running_and_completed_across_iterations() {
    let mut app = App::new();
    app.openrouter_api_key = None;
    app.strix_access_token = None;
    app.refresh_connection_state();

    assert!(app.try_start_agent_action("show workspace status"));
    assert!(app
        .active_agent_run()
        .unwrap()
        .steps
        .iter()
        .all(|step| step.status == StepStatus::Pending));

    app.on_iteration();
    assert_eq!(
        app.active_agent_run().unwrap().steps[0].status,
        StepStatus::Running
    );
    assert_eq!(
        app.pending_agent_execution.as_ref().unwrap().phase,
        AgentExecutionPhase::ExecuteStep
    );

    app.on_iteration();
    assert_eq!(
        app.active_agent_run().unwrap().steps[0].status,
        StepStatus::Running,
        "starting worker work must not also complete the visible step"
    );
    advance_execution(&mut app, 200);
    assert_eq!(app.active_agent_run().unwrap().phase, RunPhase::Completed);
    assert!(app.pending_agent_execution.is_none());
}

#[test]
fn approval_suspends_and_resumes_the_same_progressive_run() {
    let mut app = App::new();
    app.openrouter_api_key = None;
    app.strix_access_token = None;
    app.refresh_connection_state();
    assert!(app.try_start_agent_action("search notes for Aleph context"));
    let run_id = app.active_run_id.unwrap();
    app.on_iteration();
    app.on_iteration();
    assert_eq!(
        app.active_agent_run().unwrap().steps[0].status,
        StepStatus::Completed
    );

    app.request_approval(
        ApprovalRequest {
            operation: String::from("Continue"),
            target: String::from("local context"),
            effect: String::from("Continue the remaining read-only steps."),
        },
        RunChange {
            target: String::from("local context"),
            summary: String::from("Continue."),
            status: ChangeStatus::Proposed,
        },
    )
    .unwrap();
    let next_step = app.pending_agent_execution.as_ref().unwrap().current_step;
    app.on_iteration();
    assert_eq!(
        app.pending_agent_execution.as_ref().unwrap().current_step,
        next_step
    );
    assert_eq!(app.active_run_id, Some(run_id));

    app.approve_request().unwrap();
    app.on_iteration();
    assert_eq!(app.active_run_id, Some(run_id));
    assert_eq!(
        app.active_agent_run().unwrap().steps[next_step].status,
        StepStatus::Running
    );
}

#[test]
fn rejection_and_cancellation_discard_remaining_progressive_steps() {
    let mut rejected = App::new();
    rejected.openrouter_api_key = None;
    rejected.strix_access_token = None;
    rejected.refresh_connection_state();
    assert!(rejected.try_start_agent_action("search notes for broad workspace context"));
    rejected.on_iteration();
    rejected
        .request_approval(
            ApprovalRequest {
                operation: String::from("Continue"),
                target: String::from("context"),
                effect: String::from("Continue."),
            },
            RunChange {
                target: String::from("context"),
                summary: String::from("Continue."),
                status: ChangeStatus::Proposed,
            },
        )
        .unwrap();
    rejected.reject_request("rejected").unwrap();
    assert!(rejected.pending_agent_execution.is_none());
    assert_eq!(
        rejected.active_agent_run().unwrap().phase,
        RunPhase::Cancelled
    );

    let mut cancelled = App::new();
    cancelled.openrouter_api_key = None;
    cancelled.strix_access_token = None;
    cancelled.refresh_connection_state();
    assert!(cancelled.try_start_agent_action("search notes for broad workspace context"));
    cancelled.on_iteration();
    cancelled.cancel_run("cancelled").unwrap();
    advance_execution(&mut cancelled, 5);
    let run = cancelled.active_agent_run().unwrap();
    assert_eq!(run.phase, RunPhase::Cancelled);
    assert!(run
        .steps
        .iter()
        .skip(1)
        .all(|step| step.status == StepStatus::Pending));
}

#[test]
fn stale_worker_result_cannot_mutate_cancelled_or_newer_run() {
    let mut app = App::new();
    app.openrouter_api_key = None;
    app.strix_access_token = None;
    app.refresh_connection_state();
    assert!(app.try_start_agent_action("show workspace status"));
    app.on_iteration();
    let stale = app.pending_agent_execution.as_ref().unwrap();
    let stale_run = stale.run_id;
    let stale_generation = stale.generation;
    app.cancel_run("cancel old run").unwrap();

    assert!(app.try_start_agent_action("search notes for release"));
    let newer_run = app.active_run_id.unwrap();
    app.agent_worker_tx
        .send(AgentWorkerResult {
            run_id: stale_run,
            generation: stale_generation,
            step_index: 0,
            result: Ok(None),
        })
        .unwrap();
    app.on_iteration();
    assert_eq!(app.active_run_id, Some(newer_run));
    assert_ne!(newer_run, stale_run);
    assert_ne!(app.active_agent_run().unwrap().phase, RunPhase::Completed);
}

#[test]
fn provider_streaming_waits_for_local_steps_and_failure_is_terminal() {
    let mut app = App::new();
    app.ai_provider = AiProvider::OpenRouter;
    app.openrouter_api_key = Some(String::from("test-key"));
    app.refresh_connection_state();
    assert!(app.try_start_agent_action("search notes for release context"));
    assert!(
        app.pending_agent_execution
            .as_ref()
            .unwrap()
            .pending_provider
    );
    while app
        .pending_agent_execution
        .as_ref()
        .is_some_and(|execution| execution.phase != AgentExecutionPhase::Finish)
    {
        app.on_iteration();
    }
    assert!(app.chat_stream_rx.is_none());
    assert_eq!(app.active_agent_run().unwrap().phase, RunPhase::Acting);

    let run_id = app.active_run_id.unwrap();
    let (sender, receiver) = mpsc::channel();
    app.pending_agent_execution = None;
    app.chat_stream_rx = Some(receiver);
    app.transition_run(RunPhase::Streaming).unwrap();
    sender
        .send(ChatStreamUpdate::Error(String::from("provider failed")))
        .unwrap();
    app.on_tick();
    assert_eq!(app.agent_run(run_id).unwrap().phase, RunPhase::Failed);
    assert!(app.pending_agent_execution.is_none());
    assert!(app.chat_stream_rx.is_none());
}

#[test]
fn authoritative_run_approval_supports_approve_reject_cancel_and_fail() {
    let mut app = App::new();
    app.begin_run("write", RunPhase::Planning);
    app.request_approval(
        ApprovalRequest {
            operation: String::from("Create note"),
            target: String::from("Launch"),
            effect: String::from("Create the Launch note."),
        },
        RunChange {
            target: String::from("Launch"),
            summary: String::from("Create the Launch note."),
            status: ChangeStatus::Proposed,
        },
    )
    .unwrap();
    assert!(app.has_pending_agent_approval());
    assert_eq!(
        app.chat_composer.interaction(),
        ComposerInteraction::Approval
    );
    app.approve_request().unwrap();
    assert!(!app.has_pending_agent_approval());
    app.complete_run("proposal prepared").unwrap();

    app.begin_run("reject", RunPhase::Planning);
    app.request_approval(
        ApprovalRequest {
            operation: String::from("Save memory"),
            target: String::from("local memories"),
            effect: String::from("Save one memory."),
        },
        RunChange {
            target: String::from("local memories"),
            summary: String::from("Save one memory."),
            status: ChangeStatus::Proposed,
        },
    )
    .unwrap();
    app.reject_request("rejected").unwrap();
    assert_eq!(app.active_agent_run().unwrap().phase, RunPhase::Cancelled);
    assert_eq!(
        app.active_agent_run().unwrap().changes[0].status,
        ChangeStatus::Rejected
    );

    app.begin_run("cancel", RunPhase::Planning);
    app.cancel_run("cancelled").unwrap();
    assert_eq!(app.active_agent_run().unwrap().phase, RunPhase::Cancelled);

    app.begin_run("fail", RunPhase::Planning);
    let step = app.start_step("Read workspace", None).unwrap();
    app.fail_step(step, "unavailable").unwrap();
    let run = app.active_agent_run().unwrap();
    assert_eq!(run.phase, RunPhase::Failed);
    assert_eq!(run.steps[0].status, StepStatus::Failed);
    assert_eq!(run.steps[0].error.as_deref(), Some("unavailable"));
}

#[test]
fn failed_later_step_preserves_completed_earlier_steps() {
    let mut app = App::new();
    app.begin_run("two steps", RunPhase::Planning);
    app.queue_run_steps([
        (String::from("First"), None),
        (String::from("Second"), None),
    ])
    .unwrap();
    app.start_queued_step(0).unwrap();
    app.complete_step(0, "first completed").unwrap();
    app.start_queued_step(1).unwrap();
    app.fail_step(1, "second failed").unwrap();

    let run = app.active_agent_run().unwrap();
    assert_eq!(run.phase, RunPhase::Failed);
    assert_eq!(run.steps[0].status, StepStatus::Completed);
    assert_eq!(run.steps[1].status, StepStatus::Failed);
    assert!(app.pending_agent_execution.is_none());
}

#[test]
fn run_context_distinguishes_live_and_cached_repository_state() {
    let mut app = App::new();
    app.agent_context_scope = AgentContextScope::CurrentFolder;
    app.begin_run("live", RunPhase::Planning);
    assert_eq!(
        app.active_agent_run().unwrap().context.repository_source,
        RepositoryContextSource::Live
    );
    app.cancel_run("done").unwrap();

    app.agent_context_scope = AgentContextScope::Global;
    app.temporal_forks.push(TemporalFork {
        id: String::from("snapshot"),
        parent_id: None,
        label: String::from("Snapshot"),
        reason: String::from("test"),
        created_at: String::new(),
        notes: Vec::new(),
        folders: Vec::new(),
        memories: Vec::new(),
        selected_note: 0,
        activity_context: Vec::new(),
        chat_context: Vec::new(),
        repo_context: Some(RepoContext {
            cwd: String::from("/snapshot"),
            branch: Some(String::from("saved")),
            head: None,
            dirty_files: vec![String::from("old.rs")],
        }),
    });
    app.current_fork_id = Some(String::from("snapshot"));
    app.begin_run("snapshot", RunPhase::Planning);
    assert_eq!(
        app.active_agent_run().unwrap().context.repository_source,
        RepositoryContextSource::Snapshot
    );
}

#[test]
fn inline_workspace_orders_run_state_and_survives_all_supported_widths() {
    let mut app = App::new();
    app.panel_mode = PanelMode::AiChat;
    app.openrouter_api_key = None;
    app.strix_access_token = None;
    app.refresh_connection_state();
    app.begin_run("write a launch note", RunPhase::Planning);
    app.push_chat_message("user", "write a launch note");
    let step = app.start_step("Choose the target", None).unwrap();
    app.complete_step(step, "New note: Launch").unwrap();
    app.request_approval(
        ApprovalRequest {
            operation: String::from("Create note"),
            target: String::from("Launch"),
            effect: String::from("Draft and open the Launch note."),
        },
        RunChange {
            target: String::from("Launch"),
            summary: String::from("Draft and open the Launch note."),
            status: ChangeStatus::Proposed,
        },
    )
    .unwrap();
    app.push_chat_message("assistant", "I need permission before writing.");

    for width in [60, 80, 107, 108, 140] {
        let screen = rendered_chat(&app, width);
        for expected in [
            "write a launch note",
            "Context used",
            "Run · waiting for approval",
            "Permission required",
            "Create note · Launch",
            "Nothing has changed yet",
            "Changes",
            "proposed · Launch",
            "I need permission before writing",
            "offline",
            "local notes",
        ] {
            assert!(
                screen.contains(expected),
                "width {width} did not render {expected:?}\n{screen}"
            );
        }
        let positions = [
            "write a launch note",
            "Context used",
            "Run · waiting for approval",
            "Permission required",
            "Changes",
            "I need permission before writing",
        ]
        .map(|needle| screen.find(needle).unwrap());
        assert!(positions.windows(2).all(|pair| pair[0] < pair[1]));
    }
}

#[test]
fn completed_read_only_run_renders_no_changes_and_outcome_after_response() {
    let mut app = App::new();
    app.panel_mode = PanelMode::AiChat;
    app.begin_run("inspect", RunPhase::Planning);
    app.push_chat_message("user", "inspect");
    let step = app.start_step("Inspect workspace", None).unwrap();
    app.complete_step(step, "Workspace inspected").unwrap();
    app.push_chat_message("assistant", "The workspace is clean.");
    app.complete_run("Inspection completed.").unwrap();

    let screen = rendered_chat(&app, 60);
    let response = screen.find("The workspace is clean").unwrap();
    let outcome = screen.find("Outcome").unwrap();
    assert!(response < outcome);
    assert!(screen.contains("No changes were made"));
}

#[test]
fn composer_enter_submits_and_modified_enter_inserts_newlines() {
    let mut app = App::new();
    app.panel_mode = PanelMode::AiChat;
    app.openrouter_api_key = None;
    app.strix_access_token = None;
    app.refresh_connection_state();

    app.chat_composer.set_text("inspect workspace");
    app.handle_chat_key(modified(KeyCode::Enter, KeyModifiers::ALT));
    app.handle_chat_key(modified(KeyCode::Enter, KeyModifiers::SHIFT));
    assert_eq!(app.chat_composer.buffer(), "inspect workspace\n\n");

    app.handle_chat_key(press(KeyCode::Enter));
    assert!(app.chat_composer.buffer().is_empty());
    assert!(app
        .chat_messages()
        .iter()
        .any(|message| message.role == "user" && message.content == "inspect workspace"));
}

#[test]
fn composer_paste_preserves_newlines_and_ctrl_c_clears_before_quitting() {
    let mut app = App::new();
    app.panel_mode = PanelMode::AiChat;
    app.handle_paste("first\n界🙂\nthird");
    assert_eq!(app.chat_composer.buffer(), "first\n界🙂\nthird");

    app.handle_chat_key(ctrl(KeyCode::Char('c')));
    assert!(app.chat_composer.buffer().is_empty());
    assert!(!app.should_quit());
    app.handle_chat_key(ctrl(KeyCode::Char('c')));
    assert!(app.should_quit());
}

#[test]
fn composer_escape_dismisses_transcript_then_approval_before_chat() {
    let mut app = App::new();
    app.panel_mode = PanelMode::AiChat;
    app.chat_composer
        .set_interaction(ComposerInteraction::Transcript);
    app.handle_chat_key(press(KeyCode::Esc));
    assert!(app.is_ai_chat());
    assert_eq!(
        app.chat_composer.interaction(),
        ComposerInteraction::Editing
    );
    app.handle_chat_key(press(KeyCode::Esc));
    assert!(!app.is_ai_chat());

    app.panel_mode = PanelMode::AiChat;
    app.begin_run("write", RunPhase::Planning);
    app.request_approval(
        ApprovalRequest {
            operation: String::from("Create note"),
            target: String::from("Draft"),
            effect: String::from("Create the draft."),
        },
        RunChange {
            target: String::from("Draft"),
            summary: String::from("Create the draft."),
            status: ChangeStatus::Proposed,
        },
    )
    .unwrap();
    app.handle_chat_key(press(KeyCode::Esc));
    assert!(app.is_ai_chat());
    assert_eq!(app.active_agent_run().unwrap().phase, RunPhase::Cancelled);
}

#[test]
fn active_run_submission_preserves_the_next_instruction() {
    let mut app = App::new();
    app.panel_mode = PanelMode::AiChat;
    app.begin_run("current instruction", RunPhase::Planning);
    app.chat_composer.set_text("next instruction");

    app.handle_chat_key(press(KeyCode::Enter));

    assert_eq!(app.chat_composer.buffer(), "next instruction");
    assert_eq!(
        app.active_agent_run().unwrap().request,
        "current instruction"
    );
    assert!(app
        .chat_composer
        .notice()
        .is_some_and(|notice| notice.contains("preserved")));
}

#[test]
fn composer_layout_grows_caps_and_degrades_safely() {
    let mut app = App::new();
    app.panel_mode = PanelMode::AiChat;
    let area = ratatui::prelude::Rect::new(0, 0, 80, 30);

    assert_eq!(crate::ui::chat_composer_geometry(&app, area).area.height, 2);
    app.chat_composer.set_text("one\ntwo\nthree");
    assert_eq!(crate::ui::chat_composer_geometry(&app, area).area.height, 3);
    app.chat_composer.set_text("0\n1\n2\n3\n4\n5\n6\n7\n8");
    let capped = crate::ui::chat_composer_geometry(&app, area);
    assert_eq!(capped.area.height, 6);

    let narrow = crate::ui::chat_composer_geometry(&app, ratatui::prelude::Rect::new(0, 0, 20, 20));
    assert!((1..=6).contains(&narrow.area.height));
    let tiny = crate::ui::chat_composer_geometry(&app, ratatui::prelude::Rect::new(0, 0, 20, 3));
    assert_eq!(tiny.area.height, 1);
}

#[test]
fn composer_keeps_context_and_shortcuts_at_supported_widths() {
    let mut app = App::new();
    app.panel_mode = PanelMode::AiChat;

    for width in [60, 80, 107, 108, 140] {
        let screen = rendered_chat(&app, width);
        for expected in [
            "Ask Aleph to inspect",
            "Ask · Current folder · writes ask first · idle",
            "Enter:send",
            "Alt+Enter:newline",
        ] {
            assert!(
                screen.contains(expected),
                "width {width} did not render {expected:?}\n{screen}"
            );
        }
    }
}

#[test]
fn rendered_composer_cursor_stays_inside_with_unicode_and_scrolling() {
    use ratatui::{backend::Backend, backend::TestBackend, Terminal};

    let mut app = App::new();
    app.panel_mode = PanelMode::AiChat;
    app.chat_composer
        .set_text("界🙂 one\ntwo\nthree\nfour\nfive\nsix\nseven");
    app.chat_composer.ensure_cursor_visible(54, 6);
    let area = ratatui::prelude::Rect::new(0, 0, 60, 16);
    let geometry = crate::ui::chat_composer_geometry(&app, area);
    let backend = TestBackend::new(area.width, area.height);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| crate::ui::draw(frame, &app)).unwrap();
    let cursor = terminal.backend_mut().get_cursor_position().unwrap();

    assert!(geometry.area.contains(cursor));
    assert!(app.chat_composer.viewport_row() > 0);
}

#[test]
fn composer_mouse_hit_testing_uses_shared_variable_geometry() {
    use ratatui::prelude::{Position, Rect};

    let mut app = App::new();
    app.chat_composer.set_text("0\n1\n2\n3\n4\n5\n6");
    let area = Rect::new(0, 0, 80, 30);
    let geometry = crate::ui::chat_composer_geometry(&app, area);
    let inside = Position::new(geometry.area.x, geometry.area.y);
    let former_fixed_row = Position::new(geometry.area.x, area.height - 3);

    assert!(crate::ui::chat_composer_hit_test(&app, area, inside));
    assert!(!crate::ui::chat_composer_hit_test(
        &app,
        area,
        former_fixed_row
    ));
}
