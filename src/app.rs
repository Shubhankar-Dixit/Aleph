use std::time::Instant;

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

#[derive(Clone, Copy)]
pub struct CommandSpec {
    pub name: &'static str,
    pub description: &'static str,
}

#[derive(Clone)]
pub struct Note {
    pub id: usize,
    pub title: String,
    pub content: String,
    pub updated_at: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PanelMode {
    Commands,
    NoteEditor,
    FullEditor,
}

pub const COMMANDS: &[CommandSpec] = &[
    CommandSpec {
        name: "login",
        description: "Connect Aleph to a Strix session",
    },
    CommandSpec {
        name: "status",
        description: "Show session, note, and runtime health",
    },
    CommandSpec {
        name: "doctor",
        description: "Run local diagnostics",
    },
    CommandSpec {
        name: "config",
        description: "Inspect the local command config",
    },
    CommandSpec {
        name: "search",
        description: "Search notes and memories",
    },
    CommandSpec {
        name: "recall",
        description: "Show recent note activity",
    },
    CommandSpec {
        name: "ask",
        description: "Ask Strix a question",
    },
    CommandSpec {
        name: "note list",
        description: "List local notes",
    },
    CommandSpec {
        name: "note read",
        description: "Read a note by id, index, or title",
    },
    CommandSpec {
        name: "note create",
        description: "Create a note and open the editor",
    },
    CommandSpec {
        name: "note append",
        description: "Append text to the selected note",
    },
    CommandSpec {
        name: "note edit",
        description: "Edit the selected note in the bottom pane",
    },
    CommandSpec {
        name: "memory list",
        description: "List local memories",
    },
    CommandSpec {
        name: "memory save",
        description: "Save a local memory",
    },
    CommandSpec {
        name: "memory search",
        description: "Search stored memories",
    },
    CommandSpec {
        name: "canvas list",
        description: "List canvases",
    },
    CommandSpec {
        name: "canvas show",
        description: "Preview a canvas",
    },
    CommandSpec {
        name: "canvas export",
        description: "Export a canvas snapshot",
    },
    CommandSpec {
        name: "darwin run",
        description: "Run Darwin reasoning",
    },
    CommandSpec {
        name: "serve mcp",
        description: "Start the MCP server",
    },
];

const THINKING_FRAMES: [&str; 10] = [
    "◡", "⊙", "◠", "⊙", "◡", "⊙", "◉", "●", "◉", "⊙",
];

#[allow(dead_code)]
pub struct App {
    started_at: Instant,
    tick: u64,
    quit: bool,
    prompt: String,
    cursor: usize,
    history: Vec<String>,
    history_index: Option<usize>,
    selected_suggestion: usize,
    last_action: String,
    connected: bool,
    notes: Vec<Note>,
    memories: Vec<String>,
    canvases: Vec<String>,
    selected_note: usize,
    panel_mode: PanelMode,
    panel_title: String,
    panel_lines: Vec<String>,
    editor_note_index: Option<usize>,
    editor_buffer: String,
    editor_cursor: usize,
    thinking: bool,
    thinking_ticks_remaining: u8,
    ai_sidepanel_visible: bool,
    ai_panel_focused: bool,
    ai_input_buffer: String,
    ai_input_cursor: usize,
    suggestion_filter: Option<String>,
}

#[allow(dead_code)]
impl App {
    pub fn new() -> Self {
        Self {
            started_at: Instant::now(),
            tick: 0,
            quit: false,
            prompt: String::new(),
            cursor: 0,
            history: Vec::new(),
            history_index: None,
            selected_suggestion: 0,
            last_action: String::from("Ready to accept input."),
            connected: false,
            notes: vec![
                Note {
                    id: 1,
                    title: String::from("Strix gateway"),
                    content: String::from(
                        "Build a stable gateway that normalizes auth, streaming, and note operations.",
                    ),
                    updated_at: String::from("seed"),
                },
                Note {
                    id: 2,
                    title: String::from("Note editor"),
                    content: String::from(
                        "Use a terminal editor for quick edits, then move larger writes into the Strix product.",
                    ),
                    updated_at: String::from("seed"),
                },
                Note {
                    id: 3,
                    title: String::from("MCP server"),
                    content: String::from(
                        "Expose Aleph as an MCP bridge so external agents can use Strix knowledge.",
                    ),
                    updated_at: String::from("seed"),
                },
            ],
            memories: vec![
                String::from("Strix is service-backed; Aleph should not assume a local desktop app."),
                String::from("Note edit should stay lightweight and open a real text editor."),
                String::from("Keep the command surface aligned with the product plan."),
            ],
            canvases: vec![
                String::from("Architecture canvas"),
                String::from("Prompt flows"),
                String::from("Agent lifecycle"),
            ],
            selected_note: 0,
            panel_mode: PanelMode::Commands,
            panel_title: String::from("Commands"),
            panel_lines: Vec::new(),
            editor_note_index: None,
            editor_buffer: String::new(),
            editor_cursor: 0,
            thinking: false,
            thinking_ticks_remaining: 0,
            ai_sidepanel_visible: false,
            ai_panel_focused: false,
            ai_input_buffer: String::new(),
            ai_input_cursor: 0,
            suggestion_filter: None,
        }
    }

    pub fn on_tick(&mut self) {
        self.tick = self.tick.wrapping_add(1);
        if self.thinking && self.thinking_ticks_remaining > 0 {
            self.thinking_ticks_remaining -= 1;
            if self.thinking_ticks_remaining == 0 {
                self.thinking = false;
                self.panel_lines = vec![
                    String::from("AI response ready."),
                    String::from("In the future, this will connect to the agent runtime."),
                ];
            }
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

    pub fn ai_sidepanel_visible(&self) -> bool {
        self.ai_sidepanel_visible
    }

    pub fn ai_panel_focused(&self) -> bool {
        self.ai_panel_focused
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

    pub fn panel_title(&self) -> &str {
        &self.panel_title
    }

    pub fn panel_lines(&self) -> &[String] {
        &self.panel_lines
    }

    pub fn editor_buffer(&self) -> &str {
        &self.editor_buffer
    }

    pub fn editor_cursor(&self) -> usize {
        self.editor_cursor
    }

    pub fn editor_note_title(&self) -> Option<&str> {
        self.editor_note_index
            .and_then(|index| self.notes.get(index))
            .map(|note| note.title.as_str())
    }

    pub fn active_note(&self) -> Option<&Note> {
        self.notes.get(self.selected_note)
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

    pub fn visible_commands_window(&self, window_size: usize) -> (Vec<&'static CommandSpec>, usize) {
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
                    cmd.name.contains(&query)
                        || cmd.description.to_lowercase().contains(&query)
                })
                .collect()
        } else {
            COMMANDS.iter().collect()
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
            let mut all: Vec<_> = COMMANDS.iter().collect();
            all.truncate(limit);
            return all;
        }

        // Filter commands by query
        let mut matches: Vec<&'static CommandSpec> = COMMANDS
            .iter()
            .filter(|command| {
                command.name.contains(&query)
                    || command.description.to_lowercase().contains(&query)
            })
            .collect();

        matches.truncate(limit);
        matches
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

    pub fn handle_key(&mut self, key_event: KeyEvent) {
        if self.is_full_editor() {
            self.handle_full_editor_key(key_event);
            return;
        }
        if self.is_editing_note() {
            self.handle_editor_key(key_event);
            return;
        }

        match key_event.code {
            KeyCode::Char('c')
                if key_event.kind == KeyEventKind::Press
                    && key_event.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                self.request_quit();
            }
            KeyCode::Esc if key_event.kind == KeyEventKind::Press => {
                self.request_quit();
            }
            KeyCode::Enter if key_event.kind == KeyEventKind::Press => self.submit_prompt(),
            KeyCode::Backspace if key_event.kind == KeyEventKind::Press => self.backspace(),
            KeyCode::Delete if key_event.kind == KeyEventKind::Press => self.delete(),
            KeyCode::Left if matches!(key_event.kind, KeyEventKind::Press | KeyEventKind::Repeat) => {
                self.move_left()
            }
            KeyCode::Right
                if matches!(key_event.kind, KeyEventKind::Press | KeyEventKind::Repeat) =>
            {
                self.move_right()
            }
            KeyCode::Home if key_event.kind == KeyEventKind::Press => self.cursor = 0,
            KeyCode::End if key_event.kind == KeyEventKind::Press => self.cursor = self.prompt.len(),
            KeyCode::Up if matches!(key_event.kind, KeyEventKind::Press | KeyEventKind::Repeat) => {
                self.cycle_suggestion(-1)
            }
            KeyCode::Down
                if matches!(key_event.kind, KeyEventKind::Press | KeyEventKind::Repeat) =>
            {
                self.cycle_suggestion(1)
            }
            KeyCode::Tab if key_event.kind == KeyEventKind::Press => self.autocomplete(),
            KeyCode::Char(character)
                if key_event.kind == KeyEventKind::Press
                    && !key_event.modifiers.contains(KeyModifiers::CONTROL)
                    && !key_event.modifiers.contains(KeyModifiers::ALT) =>
            {
                self.insert_character(character);
            }
            _ => {}
        }
    }

    fn handle_editor_key(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Char('c')
                if key_event.kind == KeyEventKind::Press
                    && key_event.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                self.save_editor();
            }
            KeyCode::Char('s')
                if key_event.kind == KeyEventKind::Press
                    && key_event.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                self.save_editor();
            }
            KeyCode::Esc if key_event.kind == KeyEventKind::Press => self.save_editor(),
            KeyCode::Enter if key_event.kind == KeyEventKind::Press => {
                self.insert_editor_character('\n');
            }
            KeyCode::Tab if key_event.kind == KeyEventKind::Press => {
                self.insert_editor_text("    ");
            }
            KeyCode::Backspace if key_event.kind == KeyEventKind::Press => self.editor_backspace(),
            KeyCode::Delete if key_event.kind == KeyEventKind::Press => self.editor_delete(),
            KeyCode::Left if matches!(key_event.kind, KeyEventKind::Press | KeyEventKind::Repeat) => {
                self.editor_move_left()
            }
            KeyCode::Right
                if matches!(key_event.kind, KeyEventKind::Press | KeyEventKind::Repeat) =>
            {
                self.editor_move_right()
            }
            KeyCode::Home if key_event.kind == KeyEventKind::Press => self.editor_cursor = 0,
            KeyCode::End if key_event.kind == KeyEventKind::Press => {
                self.editor_cursor = self.editor_buffer.len()
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

    fn normalized_prompt(&self) -> String {
        Self::normalize_command_input(self.prompt.trim().trim_start_matches('/'))
    }

    fn insert_character(&mut self, character: char) {
        self.prompt.insert(self.cursor, character);
        self.cursor += character.len_utf8();
        self.history_index = None;
        self.suggestion_filter = None;
        self.sync_selection();
    }

    fn backspace(&mut self) {
        if self.cursor == 0 {
            return;
        }

        let previous = self.prompt[..self.cursor]
            .chars()
            .next_back()
            .map(|character| character.len_utf8())
            .unwrap_or(1);
        self.prompt.drain(self.cursor - previous..self.cursor);
        self.cursor -= previous;
        self.history_index = None;
        self.suggestion_filter = None;
        self.sync_selection();
    }

    fn delete(&mut self) {
        if self.cursor >= self.prompt.len() {
            return;
        }

        let next = self.prompt[self.cursor..]
            .chars()
            .next()
            .map(|character| character.len_utf8())
            .unwrap_or(1);
        self.prompt.drain(self.cursor..self.cursor + next);
        self.history_index = None;
        self.suggestion_filter = None;
        self.sync_selection();
    }

    fn move_left(&mut self) {
        if self.cursor == 0 {
            return;
        }

        let previous = self.prompt[..self.cursor]
            .chars()
            .next_back()
            .map(|character| character.len_utf8())
            .unwrap_or(1);
        self.cursor -= previous;
    }

    fn move_right(&mut self) {
        if self.cursor >= self.prompt.len() {
            return;
        }

        let next = self.prompt[self.cursor..]
            .chars()
            .next()
            .map(|character| character.len_utf8())
            .unwrap_or(1);
        self.cursor += next;
    }

    fn cycle_suggestion(&mut self, direction: isize) {
        // Save the current query the first time we cycle
        if self.suggestion_filter.is_none() {
            let query = self.normalized_prompt().to_lowercase();
            self.suggestion_filter = Some(query);
        }

        // Get filtered list based on suggestion_filter
        let query = self.suggestion_filter.as_ref().unwrap().clone();

        let suggestions: Vec<_> = if query.is_empty() {
            COMMANDS.iter().collect()
        } else {
            COMMANDS
                .iter()
                .filter(|cmd| {
                    cmd.name.contains(&query)
                        || cmd.description.to_lowercase().contains(&query)
                })
                .collect()
        };

        if suggestions.is_empty() {
            return;
        }

        let len = suggestions.len() as isize;
        let current = self.selected_suggestion as isize;
        let next_index = (current + direction).rem_euclid(len) as usize;
        self.selected_suggestion = next_index;

        let selected = suggestions[next_index];
        self.prompt = format!("/{}", selected.name);
        self.cursor = self.prompt.len();
        self.last_action = format!("Selected: {}", selected.name);
    }

    fn autocomplete(&mut self) {
        let suggestions = self.visible_commands(16);
        if suggestions.is_empty() {
            return;
        }

        let selected = suggestions[self.selected_suggestion.min(suggestions.len() - 1)];
        self.prompt = Self::command_label(selected);
        self.cursor = self.prompt.len();
        self.history_index = None;
        self.last_action = format!("Autocomplete: {}", Self::command_label(selected));
        self.sync_selection();

        if !suggestions.is_empty() {
            self.selected_suggestion = (self.selected_suggestion + 1) % suggestions.len();
        }
    }

    fn submit_prompt(&mut self) {
        let raw = self.prompt.trim();
        if raw.is_empty() {
            self.last_action = String::from("Type a query, command, or press Ctrl+C to quit.");
            return;
        }

        if !raw.starts_with('/') {
            let query = raw.to_string();
            self.history.push(query.clone());
            self.history_index = None;
            self.thinking = true;
            self.thinking_ticks_remaining = 15;
            self.set_result_panel(
                "AI Query",
                vec![
                    format!("Query: {}", query),
                    String::from("Processing your request..."),
                ],
            );
            self.last_action = format!("AI Query: {}", query);
            self.reset_prompt();
            return;
        }

        let prompt = self.normalized_prompt();
        let Some((command, args)) = Self::parse_command(prompt.as_str()) else {
            self.set_result_panel(
                "Unknown command",
                vec![
                    format!("I don't know `/{}'.", prompt),
                    String::from("Try /note list, /note edit, or /memory search."),
                ],
            );
            self.last_action = format!("Unknown: /{}", prompt);
            self.reset_prompt();
            return;
        };

        self.history.push(format!("/{}", prompt));
        self.history_index = None;
        self.execute_command(command.name, args);
        self.reset_prompt();
    }

    fn reset_prompt(&mut self) {
        self.prompt.clear();
        self.cursor = 0;
        self.selected_suggestion = 0;
        self.suggestion_filter = None;
    }

    fn normalize_command_input(prompt: &str) -> String {
        prompt.split_whitespace().collect::<Vec<_>>().join(" ")
    }

    fn parse_command<'a>(prompt: &'a str) -> Option<(&'static CommandSpec, &'a str)> {
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

    fn execute_command(&mut self, command: &str, args: &str) {
        match command {
            "login" => {
                self.connected = true;
                self.set_result_panel(
                    "Login",
                    vec![
                        String::from("Connected to the local Aleph session."),
                        String::from("Strix gateway integration is still a local mock."),
                    ],
                );
                self.last_action = String::from("Connected to Strix.");
            }
            "logout" => {
                self.connected = false;
                self.set_result_panel(
                    "Logout",
                    vec![String::from("Disconnected the current session.")],
                );
                self.last_action = String::from("Disconnected.");
            }
            "status" => {
                self.set_result_panel(
                    "Status",
                    vec![
                        format!("Session: {}", if self.connected { "connected" } else { "offline" }),
                        format!("Notes: {}", self.notes.len()),
                        format!("Memories: {}", self.memories.len()),
                        format!("Canvases: {}", self.canvases.len()),
                        format!("Uptime: {}", self.uptime()),
                    ],
                );
                self.last_action = String::from("Refreshed status.");
            }
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
            "config" => {
                self.set_result_panel(
                    "Config",
                    vec![
                        String::from("Theme: dark purple on black"),
                        String::from("Editor: embedded terminal note editor"),
                        String::from("Commands: local and Strix-aligned"),
                    ],
                );
                self.last_action = String::from("Opened config summary.");
            }
            "search" => {
                let query = args.trim();
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
                    vec![String::from("Ask Strix a question after the command, for example: /ask what should ship next?")]
                } else {
                    vec![
                        format!("Question: {}", query),
                        String::from("This local build is still wired to the plan layer, not the live Strix backend."),
                        String::from("Use note read/edit for now, then connect the service layer next."),
                    ]
                };
                self.set_result_panel("Ask", lines);
                self.last_action = String::from("Prepared an ask response.");
            }
            "note list" => {
                let lines = self
                    .notes
                    .iter()
                    .enumerate()
                    .map(|(index, note)| {
                        format!(
                            "{:>2}. #{} {:<18} {}",
                            index + 1,
                            note.id,
                            note.title,
                            Self::preview_text(&note.content, 42)
                        )
                    })
                    .collect::<Vec<_>>();

                self.set_result_panel("Notes", lines);
                self.last_action = String::from("Listed notes.");
            }
            "note read" => {
                let Some(index) = self.resolve_note_index(args.trim()) else {
                    self.set_result_panel(
                        "Note not found",
                        vec![String::from("Try /note read 1 or /note read Strix gateway.")],
                    );
                    self.last_action = String::from("Note not found.");
                    return;
                };

                self.selected_note = index;
                let (note_title, note_id, note_updated, note_content) = {
                    let note = &self.notes[index];
                    (
                        note.title.clone(),
                        note.id,
                        note.updated_at.clone(),
                        note.content.clone(),
                    )
                };
                let mut lines = vec![format!("ID: {}", note_id), format!("Updated: {}", note_updated), String::new()];
                lines.extend(note_content.lines().map(|line| line.to_string()));
                self.set_result_panel(format!("Note: {}", note_title), lines);
                self.last_action = format!("Opened note: {}", note_title);
            }
            "note create" => {
                let title = if args.trim().is_empty() {
                    String::from("Untitled note")
                } else {
                    args.trim().to_string()
                };
                let note_id = self.notes.len() + 1;
                self.notes.push(Note {
                    id: note_id,
                    title: title.clone(),
                    content: String::new(),
                    updated_at: String::from("draft"),
                });
                let index = self.notes.len() - 1;
                self.open_note_editor(index);
                self.last_action = format!("Created note: {}", title);
            }
            "note append" => {
                let Some(index) = self.current_note_index() else {
                    self.set_result_panel(
                        "Append failed",
                        vec![String::from("No note is selected right now.")],
                    );
                    self.last_action = String::from("No selected note to append to.");
                    return;
                };

                let append_text = args.trim();
                if append_text.is_empty() {
                    self.set_result_panel(
                        "Append failed",
                        vec![String::from("Provide text after /note append.")],
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
                    note.updated_at = updated_at;
                    (note.title.clone(), note.content.clone())
                };
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
                        vec![String::from("No notes are available yet." )],
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
                    self.set_result_panel(
                        "Edit failed",
                        vec![String::from("Note not found.")],
                    );
                    self.last_action = String::from("Note not found.");
                    return;
                };

                self.open_note_editor(index);
                self.last_action = format!("Editing note: {}", self.notes[index].title);
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
                    vec![memory.to_string(), String::from("Stored in the local demo memory list.")],
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
            "canvas list" => {
                let lines = self
                    .canvases
                    .iter()
                    .enumerate()
                    .map(|(index, canvas)| format!("{:>2}. {}", index + 1, canvas))
                    .collect::<Vec<_>>();
                self.set_result_panel("Canvases", lines);
                self.last_action = String::from("Listed canvases.");
            }
            "canvas show" => {
                let query = args.trim();
                let canvas = self
                    .canvases
                    .iter()
                    .find(|name| query.is_empty() || name.to_lowercase().contains(&query.to_lowercase()))
                    .cloned()
                    .unwrap_or_else(|| String::from("Canvas not found"));
                self.set_result_panel(
                    format!("Canvas: {}", query),
                    vec![canvas, String::from("Canvas previews stay text-first in this build.")],
                );
                self.last_action = String::from("Opened a canvas preview.");
            }
            "canvas export" => {
                let query = if args.trim().is_empty() {
                    String::from("selected canvas")
                } else {
                    args.trim().to_string()
                };
                self.set_result_panel(
                    "Canvas export",
                    vec![
                        format!("Exported {} as JSON in the local demo flow.", query),
                        String::from("Wire this to the real Strix export endpoint next."),
                    ],
                );
                self.last_action = String::from("Exported a canvas snapshot.");
            }
            "darwin run" => {
                let query = args.trim();
                self.set_result_panel(
                    "Darwin",
                    vec![
                        format!("Question: {}", query),
                        String::from("Champion thesis: push note editing and MCP wiring first."),
                        String::from("Risks: local mock data will need a real Strix adapter later."),
                    ],
                );
                self.last_action = String::from("Ran Darwin reasoning.");
            }
            "serve mcp" => {
                self.set_result_panel(
                    "MCP server",
                    vec![
                        String::from("The MCP server entrypoint is still a stub in this sample build."),
                        String::from("Use this command to wire the gateway layer once the transport is ready."),
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

    fn set_result_panel(&mut self, title: impl Into<String>, lines: Vec<String>) {
        self.panel_mode = PanelMode::Commands;
        self.panel_title = title.into();
        self.panel_lines = lines;
        self.editor_note_index = None;
    }

    fn open_note_editor(&mut self, index: usize) {
        if self.notes.is_empty() {
            return;
        }

        let index = index.min(self.notes.len() - 1);
        self.selected_note = index;
        self.editor_note_index = Some(index);
        self.editor_buffer = self.notes[index].content.clone();
        self.editor_cursor = self.editor_buffer.len();
        self.panel_mode = PanelMode::FullEditor;
        self.panel_title = format!("Editing: {}", self.notes[index].title);
        self.panel_lines.clear();
        self.ai_sidepanel_visible = false;
        self.last_action = format!("Editing note: {}", self.notes[index].title);
    }

    fn save_editor(&mut self) {
        let Some(index) = self.editor_note_index else {
            return;
        };

        let updated_at = self.uptime();
        let note_title = if let Some(note) = self.notes.get_mut(index) {
            note.content = self.editor_buffer.clone();
            note.updated_at = updated_at;
            note.title.clone()
        } else {
            return;
        };

        self.selected_note = index;
        self.set_result_panel(
            format!("Saved note: {}", note_title),
            self.note_detail_lines(index),
        );
        self.last_action = format!("Saved note: {}", note_title);
        self.editor_note_index = None;
    }

    fn note_detail_lines(&self, index: usize) -> Vec<String> {
        let Some(note) = self.notes.get(index) else {
            return vec![String::from("No note available.")];
        };

        let mut lines = vec![format!("ID: {}", note.id), format!("Updated: {}", note.updated_at), String::new()];
        lines.extend(note.content.lines().map(|line| line.to_string()));
        lines
    }

    fn current_note_index(&self) -> Option<usize> {
        if self.notes.is_empty() {
            None
        } else {
            Some(self.selected_note.min(self.notes.len() - 1))
        }
    }

    fn resolve_note_index(&self, target: &str) -> Option<usize> {
        let trimmed = target.trim();
        if trimmed.is_empty() {
            return self.current_note_index();
        }

        let normalized = trimmed.trim_start_matches('#');
        if let Ok(index) = normalized.parse::<usize>() {
            if index == 0 {
                return None;
            }

            if index > self.notes.len() {
                return None;
            }

            return Some(index - 1);
        }

        let lower = trimmed.to_lowercase();
        self.notes.iter().enumerate().find_map(|(index, note)| {
            let title = note.title.to_lowercase();
            if note.title.eq_ignore_ascii_case(trimmed) || title.contains(&lower) {
                Some(index)
            } else {
                None
            }
        })
    }

    fn search_notes(&self, query: &str) -> Vec<String> {
        let query = query.to_lowercase();
        self.notes
            .iter()
            .filter(|note| {
                query.is_empty()
                    || note.title.to_lowercase().contains(&query)
                    || note.content.to_lowercase().contains(&query)
            })
            .map(|note| format!("{} — {}", note.title, Self::preview_text(&note.content, 56)))
            .collect()
    }

    fn preview_text(content: &str, limit: usize) -> String {
        let collapsed = content.split_whitespace().collect::<Vec<_>>().join(" ");
        let preview = collapsed.trim();

        if preview.chars().count() <= limit {
            return preview.to_string();
        }

        preview.chars().take(limit.saturating_sub(1)).collect::<String>() + "…"
    }

    fn insert_editor_text(&mut self, text: &str) {
        for character in text.chars() {
            self.insert_editor_character(character);
        }
    }

    fn insert_editor_character(&mut self, character: char) {
        self.editor_buffer.insert(self.editor_cursor, character);
        self.editor_cursor += character.len_utf8();
    }

    fn editor_backspace(&mut self) {
        if self.editor_cursor == 0 {
            return;
        }

        let previous = self.editor_buffer[..self.editor_cursor]
            .chars()
            .next_back()
            .map(|character| character.len_utf8())
            .unwrap_or(1);
        self.editor_buffer.drain(self.editor_cursor - previous..self.editor_cursor);
        self.editor_cursor -= previous;
    }

    fn editor_delete(&mut self) {
        if self.editor_cursor >= self.editor_buffer.len() {
            return;
        }

        let next = self.editor_buffer[self.editor_cursor..]
            .chars()
            .next()
            .map(|character| character.len_utf8())
            .unwrap_or(1);
        self.editor_buffer.drain(self.editor_cursor..self.editor_cursor + next);
    }

    fn editor_move_left(&mut self) {
        if self.editor_cursor == 0 {
            return;
        }

        let previous = self.editor_buffer[..self.editor_cursor]
            .chars()
            .next_back()
            .map(|character| character.len_utf8())
            .unwrap_or(1);
        self.editor_cursor -= previous;
    }

    fn editor_move_right(&mut self) {
        if self.editor_cursor >= self.editor_buffer.len() {
            return;
        }

        let next = self.editor_buffer[self.editor_cursor..]
            .chars()
            .next()
            .map(|character| character.len_utf8())
            .unwrap_or(1);
        self.editor_cursor += next;
    }

    fn sync_selection(&mut self) {
        let suggestions = self.visible_commands(16);
        if suggestions.is_empty() {
            self.selected_suggestion = 0;
            return;
        }

        self.selected_suggestion = self.selected_suggestion.min(suggestions.len() - 1);
    }

    fn handle_full_editor_key(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Char('c')
                if key_event.kind == KeyEventKind::Press
                    && key_event.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                self.save_editor();
            }
            KeyCode::Char('s')
                if key_event.kind == KeyEventKind::Press
                    && key_event.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                self.save_editor();
            }
            KeyCode::Esc if key_event.kind == KeyEventKind::Press => self.save_editor(),
            KeyCode::Char('l')
                if key_event.kind == KeyEventKind::Press
                    && key_event.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                if self.ai_sidepanel_visible {
                    self.ai_sidepanel_visible = false;
                    self.ai_panel_focused = false;
                } else {
                    self.ai_sidepanel_visible = true;
                    self.ai_panel_focused = true;
                }
            }
            KeyCode::Tab if key_event.kind == KeyEventKind::Press && self.ai_sidepanel_visible => {
                self.ai_panel_focused = !self.ai_panel_focused;
            }
            _ => {
                if self.ai_panel_focused {
                    self.handle_ai_input_key(key_event);
                } else {
                    self.handle_editor_content_key(key_event);
                }
            }
        }
    }

    fn handle_editor_content_key(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Enter if key_event.kind == KeyEventKind::Press => {
                self.insert_editor_character('\n');
            }
            KeyCode::Tab if key_event.kind == KeyEventKind::Press => {
                self.insert_editor_text("    ");
            }
            KeyCode::Backspace if key_event.kind == KeyEventKind::Press => self.editor_backspace(),
            KeyCode::Delete if key_event.kind == KeyEventKind::Press => self.editor_delete(),
            KeyCode::Left if matches!(key_event.kind, KeyEventKind::Press | KeyEventKind::Repeat) => {
                self.editor_move_left()
            }
            KeyCode::Right
                if matches!(key_event.kind, KeyEventKind::Press | KeyEventKind::Repeat) =>
            {
                self.editor_move_right()
            }
            KeyCode::Home if key_event.kind == KeyEventKind::Press => self.editor_cursor = 0,
            KeyCode::End if key_event.kind == KeyEventKind::Press => {
                self.editor_cursor = self.editor_buffer.len()
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

    fn handle_ai_input_key(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Enter if key_event.kind == KeyEventKind::Press => {
                // Send message to AI
                if !self.ai_input_buffer.trim().is_empty() {
                    self.thinking = true;
                    self.thinking_ticks_remaining = 15;
                    self.ai_input_buffer.clear();
                    self.ai_input_cursor = 0;
                }
            }
            KeyCode::Backspace if key_event.kind == KeyEventKind::Press => {
                if self.ai_input_cursor > 0 {
                    let prev = self.ai_input_buffer[..self.ai_input_cursor]
                        .chars()
                        .next_back()
                        .map(|c| c.len_utf8())
                        .unwrap_or(1);
                    self.ai_input_buffer.drain(self.ai_input_cursor - prev..self.ai_input_cursor);
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
                    self.ai_input_buffer.drain(self.ai_input_cursor..self.ai_input_cursor + next);
                }
            }
            KeyCode::Left if matches!(key_event.kind, KeyEventKind::Press | KeyEventKind::Repeat) => {
                if self.ai_input_cursor > 0 {
                    let prev = self.ai_input_buffer[..self.ai_input_cursor]
                        .chars()
                        .next_back()
                        .map(|c| c.len_utf8())
                        .unwrap_or(1);
                    self.ai_input_cursor -= prev;
                }
            }
            KeyCode::Right if matches!(key_event.kind, KeyEventKind::Press | KeyEventKind::Repeat) => {
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
}

#[cfg(test)]
mod tests {
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

    #[test]
    fn repeated_character_events_do_not_duplicate_input() {
        let mut app = App::new();

        app.handle_key(press(KeyCode::Char('a')));
        app.handle_key(repeat(KeyCode::Char('a')));

        assert_eq!(app.prompt(), "a");
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

        assert_eq!(app.last_action(), "Refreshed status.");
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
    fn note_edit_opens_the_editor_and_saves_changes() {
        let mut app = App::new();

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

        assert!(!app.is_full_editor());
        assert!(app.notes[0].content.contains("Added from the editor"));
    }
}
