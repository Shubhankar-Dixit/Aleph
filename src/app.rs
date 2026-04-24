use std::collections::VecDeque;
use std::time::Instant;

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseEvent, MouseEventKind};

#[derive(Clone, Copy)]
pub struct CommandSpec {
    pub name: &'static str,
    pub description: &'static str,
}

#[derive(Clone)]
pub struct Folder {
    pub id: usize,
    pub name: String,
    pub parent_id: Option<usize>,
}

#[derive(Clone)]
pub struct Note {
    pub id: usize,
    pub title: String,
    pub content: String,
    pub updated_at: String,
    pub folder_id: Option<usize>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PanelMode {
    Commands,
    NoteEditor,
    FullEditor,
    AiChat,
}

#[derive(Clone)]
pub struct ChatMessage {
    pub role: String,  // "user" or "assistant"
    pub content: String,
    pub timestamp: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum CursorStyle {
    Block,
    Line,
}

#[derive(Clone)]
pub struct EditorState {
    pub buffer: String,
    pub cursor: usize,
    pub scroll_offset: usize,
}

#[derive(Clone)]
pub struct SearchState {
    pub query: String,
    pub matches: Vec<usize>,
    pub current_match: Option<usize>,
    pub active: bool,
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
        name: "note move",
        description: "Move a note to a folder",
    },
    CommandSpec {
        name: "folder list",
        description: "List all folders",
    },
    CommandSpec {
        name: "folder create",
        description: "Create a new folder",
    },
    CommandSpec {
        name: "folder delete",
        description: "Delete a folder",
    },
    CommandSpec {
        name: "folder notes",
        description: "List notes in a folder",
    },
    CommandSpec {
        name: "folder tree",
        description: "Show folder hierarchy",
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
    folders: Vec<Folder>,
    memories: Vec<String>,
    canvases: Vec<String>,
    selected_note: usize,
    current_folder_id: Option<usize>,
    panel_mode: PanelMode,
    panel_title: String,
    panel_lines: Vec<String>,
    editor_note_index: Option<usize>,
    editor_buffer: String,
    editor_cursor: usize,
    thinking: bool,
    thinking_ticks_remaining: u8,
    ai_overlay_visible: bool,
    ai_overlay_pulse_ticks: u8,
    save_shimmer_ticks: u8,
    ai_input_buffer: String,
    ai_input_cursor: usize,
    suggestion_filter: Option<String>,
    editor_scroll_offset: usize,
    editor_word_wrap: bool,
    editor_cursor_style: CursorStyle,
    undo_stack: VecDeque<EditorState>,
    redo_stack: VecDeque<EditorState>,
    search_state: SearchState,
    chat_messages: Vec<ChatMessage>,
    chat_input_buffer: String,
    chat_input_cursor: usize,
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
            folders: vec![
                Folder {
                    id: 1,
                    name: String::from("Projects"),
                    parent_id: None,
                },
                Folder {
                    id: 2,
                    name: String::from("Ideas"),
                    parent_id: None,
                },
                Folder {
                    id: 3,
                    name: String::from("Aleph"),
                    parent_id: Some(1),
                },
            ],
            notes: vec![
                Note {
                    id: 1,
                    title: String::from("Strix gateway"),
                    content: String::from(
                        "Build a stable gateway that normalizes auth, streaming, and note operations.",
                    ),
                    updated_at: String::from("seed"),
                    folder_id: Some(3),
                },
                Note {
                    id: 2,
                    title: String::from("Note editor"),
                    content: String::from(
                        "Use a terminal editor for quick edits, then move larger writes into the Strix product.",
                    ),
                    updated_at: String::from("seed"),
                    folder_id: Some(3),
                },
                Note {
                    id: 3,
                    title: String::from("MCP server"),
                    content: String::from(
                        "Expose Aleph as an MCP bridge so external agents can use Strix knowledge.",
                    ),
                    updated_at: String::from("seed"),
                    folder_id: None,
                },
                Note {
                    id: 4,
                    title: String::from("Feature ideas"),
                    content: String::from(
                        "Folder navigation, search within folders, nested folders like Strix.",
                    ),
                    updated_at: String::from("seed"),
                    folder_id: Some(2),
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
            editor_cursor_style: CursorStyle::Line,
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
        }
    }

    pub fn on_tick(&mut self) {
        self.tick = self.tick.wrapping_add(1);
        if self.thinking && self.thinking_ticks_remaining > 0 {
            self.thinking_ticks_remaining -= 1;
            if self.thinking_ticks_remaining == 0 {
                self.thinking = false;
                // Add mock AI response based on chat mode or regular mode
                if self.panel_mode == PanelMode::AiChat || self.ai_overlay_visible {
                    self.add_mock_ai_response();
                } else {
                    self.panel_lines = vec![
                        String::from("AI response ready."),
                        String::from("In the future, this will connect to the agent runtime."),
                    ];
                }
            }
        }

        if self.ai_overlay_visible && self.ai_overlay_pulse_ticks > 0 {
            self.ai_overlay_pulse_ticks -= 1;
        }
        if self.save_shimmer_ticks > 0 {
            self.save_shimmer_ticks -= 1;
        }
    }

    fn add_mock_ai_response(&mut self) {
        let mock_responses = [
            "I understand. Let me help you with that.",
            "That's an interesting question. Here's what I think...",
            "I can assist with that. What specific aspects would you like to explore?",
            "Let me analyze this for you. Based on the context...",
            "I see what you're getting at. Here's my perspective...",
            "Good question. Let me break this down for you.",
            "I have some thoughts on this. First, consider...",
        ];

        let response = if !self.chat_messages.is_empty() {
            let last_user_msg = self.chat_messages.iter().rev().find(|m| m.role == "user");
            if let Some(msg) = last_user_msg {
                // Generate a contextual response based on keywords
                let content_lower = msg.content.to_lowercase();
                if content_lower.contains("hello") || content_lower.contains("hi") {
                    String::from("Hello! I'm your AI assistant. How can I help you today?")
                } else if content_lower.contains("help") {
                    String::from("I can help you with notes, folders, searching, and general questions. Try commands like /note list, /folder tree, or just ask me anything!")
                } else if content_lower.contains("note") {
                    String::from("I see you're working with notes. You can create notes with /note create, edit them with /note edit, and organize them into folders with /folder create.")
                } else if content_lower.contains("folder") {
                    String::from("Folders help organize your notes hierarchically. Use /folder list to see all folders, /folder tree for the structure, and /folder notes to see what's inside.")
                } else {
                    let idx = (self.tick as usize) % mock_responses.len();
                    format!("{}", mock_responses[idx])
                }
            } else {
                String::from("I'm ready to help. What would you like to discuss?")
            }
        } else {
            String::from("I'm ready to help. What would you like to discuss?")
        };

        self.chat_messages.push(ChatMessage {
            role: String::from("assistant"),
            content: response,
            timestamp: self.uptime(),
        });
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

    pub fn editor_scroll_offset(&self) -> usize {
        self.editor_scroll_offset
    }

    pub fn editor_word_wrap(&self) -> bool {
        self.editor_word_wrap
    }

    pub fn editor_cursor_style(&self) -> CursorStyle {
        self.editor_cursor_style
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
        if self.is_ai_chat() {
            self.handle_chat_key(key_event);
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

    pub fn handle_mouse(&mut self, mouse_event: MouseEvent) {
        if !self.is_full_editor() || !self.ai_overlay_visible {
            return;
        }

        if matches!(mouse_event.kind, MouseEventKind::Down(_)) {
            self.close_ai_overlay();
        }
    }

    fn handle_editor_key(&mut self, key_event: KeyEvent) {
        if self.search_state.active {
            self.handle_search_key(key_event);
            return;
        }
        match key_event.code {
            KeyCode::Char('c')
                if key_event.kind == KeyEventKind::Press
                    && key_event.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                self.exit_editor();
            }
            KeyCode::Char('s')
                if key_event.kind == KeyEventKind::Press
                    && key_event.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                self.save_editor();
            }
            KeyCode::Esc if key_event.kind == KeyEventKind::Press => self.exit_editor(),
            KeyCode::Enter if key_event.kind == KeyEventKind::Press => {
                self.save_undo_state();
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
            KeyCode::Up
                if matches!(key_event.kind, KeyEventKind::Press | KeyEventKind::Repeat)
                    && !key_event.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                self.editor_move_up()
            }
            KeyCode::Down
                if matches!(key_event.kind, KeyEventKind::Press | KeyEventKind::Repeat)
                    && !key_event.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                self.editor_move_down()
            }
            KeyCode::Up
                if key_event.kind == KeyEventKind::Press
                    && key_event.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                self.scroll_up(1)
            }
            KeyCode::Down
                if key_event.kind == KeyEventKind::Press
                    && key_event.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                self.scroll_down(1)
            }
            KeyCode::PageUp if key_event.kind == KeyEventKind::Press => self.scroll_up(10),
            KeyCode::PageDown if key_event.kind == KeyEventKind::Press => self.scroll_down(10),
            KeyCode::Home if key_event.kind == KeyEventKind::Press => self.editor_cursor = 0,
            KeyCode::End if key_event.kind == KeyEventKind::Press => {
                self.editor_cursor = self.editor_buffer.len()
            }
            KeyCode::Char('z')
                if key_event.kind == KeyEventKind::Press
                    && key_event.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                self.undo();
            }
            KeyCode::Char('y')
                if key_event.kind == KeyEventKind::Press
                    && key_event.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                self.redo();
            }
            KeyCode::Char('f')
                if key_event.kind == KeyEventKind::Press
                    && key_event.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                self.start_search();
            }
            KeyCode::F(3) if key_event.kind == KeyEventKind::Press => {
                if key_event.modifiers.contains(KeyModifiers::SHIFT) {
                    self.search_prev();
                } else {
                    self.search_next();
                }
            }
            KeyCode::Char('w')
                if key_event.kind == KeyEventKind::Press
                    && key_event.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                self.toggle_word_wrap();
            }
            KeyCode::Char('b')
                if key_event.kind == KeyEventKind::Press
                    && key_event.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                self.toggle_cursor_style();
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

    fn handle_chat_key(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Esc if key_event.kind == KeyEventKind::Press => {
                // Exit chat mode and return to commands
                self.panel_mode = PanelMode::Commands;
                self.panel_title = String::from("Commands");
                self.panel_lines.clear();
                self.last_action = String::from("Exited AI chat.");
            }
            KeyCode::Char('c')
                if key_event.kind == KeyEventKind::Press
                    && key_event.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                self.request_quit();
            }
            KeyCode::Enter if key_event.kind == KeyEventKind::Press => {
                // Send chat message
                let msg = self.chat_input_buffer.trim();
                if !msg.is_empty() {
                    self.chat_messages.push(ChatMessage {
                        role: String::from("user"),
                        content: msg.to_string(),
                        timestamp: self.uptime(),
                    });
                    self.thinking = true;
                    self.thinking_ticks_remaining = 20;
                    self.chat_input_buffer.clear();
                    self.chat_input_cursor = 0;
                }
            }
            KeyCode::Backspace if key_event.kind == KeyEventKind::Press => {
                if self.chat_input_cursor > 0 {
                    let prev = self.chat_input_buffer[..self.chat_input_cursor]
                        .chars()
                        .next_back()
                        .map(|c| c.len_utf8())
                        .unwrap_or(1);
                    self.chat_input_buffer
                        .drain(self.chat_input_cursor - prev..self.chat_input_cursor);
                    self.chat_input_cursor -= prev;
                }
            }
            KeyCode::Delete if key_event.kind == KeyEventKind::Press => {
                if self.chat_input_cursor < self.chat_input_buffer.len() {
                    let next = self.chat_input_buffer[self.chat_input_cursor..]
                        .chars()
                        .next()
                        .map(|c| c.len_utf8())
                        .unwrap_or(1);
                    self.chat_input_buffer
                        .drain(self.chat_input_cursor..self.chat_input_cursor + next);
                }
            }
            KeyCode::Left
                if matches!(key_event.kind, KeyEventKind::Press | KeyEventKind::Repeat) =>
            {
                if self.chat_input_cursor > 0 {
                    let prev = self.chat_input_buffer[..self.chat_input_cursor]
                        .chars()
                        .next_back()
                        .map(|c| c.len_utf8())
                        .unwrap_or(1);
                    self.chat_input_cursor -= prev;
                }
            }
            KeyCode::Right
                if matches!(key_event.kind, KeyEventKind::Press | KeyEventKind::Repeat) =>
            {
                if self.chat_input_cursor < self.chat_input_buffer.len() {
                    let next = self.chat_input_buffer[self.chat_input_cursor..]
                        .chars()
                        .next()
                        .map(|c| c.len_utf8())
                        .unwrap_or(1);
                    self.chat_input_cursor += next;
                }
            }
            KeyCode::Home if key_event.kind == KeyEventKind::Press => self.chat_input_cursor = 0,
            KeyCode::End if key_event.kind == KeyEventKind::Press => {
                self.chat_input_cursor = self.chat_input_buffer.len()
            }
            KeyCode::Char(character)
                if key_event.kind == KeyEventKind::Press
                    && !key_event.modifiers.contains(KeyModifiers::CONTROL)
                    && !key_event.modifiers.contains(KeyModifiers::ALT) =>
            {
                self.chat_input_buffer.insert(self.chat_input_cursor, character);
                self.chat_input_cursor += character.len_utf8();
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
            // Enter AI chat mode with the user's message
            let query = raw.to_string();
            self.history.push(query.clone());
            self.history_index = None;

            // Add user message to chat
            self.chat_messages.push(ChatMessage {
                role: String::from("user"),
                content: query.clone(),
                timestamp: self.uptime(),
            });

            // Switch to chat mode and start thinking
            self.panel_mode = PanelMode::AiChat;
            self.thinking = true;
            self.thinking_ticks_remaining = 20;
            self.last_action = format!("AI Chat: {}", query);
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
                let folder_id = self.current_folder_id;
                let folder_name = folder_id
                    .and_then(|id| self.get_folder_name(id))
                    .unwrap_or_else(|| String::from("All notes"));

                let lines = self
                    .notes
                    .iter()
                    .enumerate()
                    .filter(|(_, note)| {
                        // If we're in a folder, only show notes from that folder
                        folder_id.is_none() || note.folder_id == folder_id
                    })
                    .map(|(index, note)| {
                        let folder_indicator = if let Some(fid) = note.folder_id {
                            let fname = self.get_folder_name(fid).unwrap_or_default();
                            format!("[{}] ", &fname[..fname.len().min(8)])
                        } else {
                            String::from("[—] ")
                        };
                        format!(
                            "{:>2}. #{} {:<14} {}{}",
                            index + 1,
                            note.id,
                            if note.title.len() > 14 { format!("{}…", &note.title[..13]) } else { note.title.clone() },
                            folder_indicator,
                            Self::preview_text(&note.content, 32)
                        )
                    })
                    .collect::<Vec<_>>();

                self.set_result_panel(format!("Notes — {}", folder_name), lines);
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
                let note = &self.notes[index];
                let note_title = note.title.clone();
                let note_id = note.id;
                let note_updated = note.updated_at.clone();
                let note_content = note.content.clone();
                let folder_info = if let Some(fid) = note.folder_id {
                    format!("Folder: {}", self.get_folder_path(fid))
                } else {
                    String::from("Folder: Uncategorized")
                };

                let mut lines = vec![
                    format!("ID: {}", note_id),
                    format!("Updated: {}", note_updated),
                    folder_info,
                    String::new(),
                ];
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
                let note_id = self.notes.iter().map(|n| n.id).max().unwrap_or(0) + 1;
                self.notes.push(Note {
                    id: note_id,
                    title: title.clone(),
                    content: String::new(),
                    updated_at: String::from("draft"),
                    folder_id: self.current_folder_id,
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
                    vec![format!("Moved '{}' to folder '{}'.", note_title, folder_name)],
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
                        vec![String::from("Provide a folder ID or name after /folder delete.")],
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
                let lines = self.build_folder_tree();
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
        self.close_ai_overlay();
        self.last_action = format!("Editing note: {}", self.notes[index].title);
    }

    fn save_editor(&mut self) {
        let Some(index) = self.editor_note_index else {
            return;
        };

        let updated_at = self.uptime();
        if let Some(note) = self.notes.get_mut(index) {
            note.content = self.editor_buffer.clone();
            note.updated_at = updated_at;
        }
        self.save_shimmer_ticks = 4;
    }

    fn exit_editor(&mut self) {
        self.save_editor();
        let index = self.editor_note_index.unwrap_or(0);
        let note_title = self.notes.get(index).map(|n| n.title.clone()).unwrap_or_default();
        
        self.selected_note = index;
        self.set_result_panel(
            format!("Saved note: {}", note_title),
            self.note_detail_lines(index),
        );
        self.last_action = format!("Exited note: {}", note_title);
        self.editor_note_index = None;
    }

    fn note_detail_lines(&self, index: usize) -> Vec<String> {
        let Some(note) = self.notes.get(index) else {
            return vec![String::from("No note available.")];
        };

        let folder_info = if let Some(fid) = note.folder_id {
            format!("Folder: {}", self.get_folder_path(fid))
        } else {
            String::from("Folder: Uncategorized")
        };

        let mut lines = vec![
            format!("ID: {}", note.id),
            format!("Updated: {}", note.updated_at),
            folder_info,
            String::new(),
        ];
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

    fn resolve_folder_id(&self, target: &str) -> Option<usize> {
        let trimmed = target.trim();
        if trimmed.is_empty() {
            return self.current_folder_id;
        }

        // Try to parse as ID first (supports #1 or just 1)
        let normalized = trimmed.trim_start_matches('#');
        if let Ok(id) = normalized.parse::<usize>() {
            if self.folders.iter().any(|f| f.id == id) {
                return Some(id);
            }
        }

        // Search by name (case-insensitive)
        let lower = trimmed.to_lowercase();
        self.folders.iter().find_map(|folder| {
            if folder.name.eq_ignore_ascii_case(trimmed)
                || folder.name.to_lowercase().contains(&lower)
            {
                Some(folder.id)
            } else {
                None
            }
        })
    }

    fn get_folder_name(&self, folder_id: usize) -> Option<String> {
        self.folders
            .iter()
            .find(|f| f.id == folder_id)
            .map(|f| f.name.clone())
    }

    fn get_folder_path(&self, folder_id: usize) -> String {
        let mut path = Vec::new();
        let mut current_id = Some(folder_id);

        while let Some(id) = current_id {
            if let Some(folder) = self.folders.iter().find(|f| f.id == id) {
                path.push(folder.name.clone());
                current_id = folder.parent_id;
            } else {
                break;
            }
        }

        path.reverse();
        if path.is_empty() {
            String::from("/")
        } else {
            format!("/{}", path.join("/"))
        }
    }

    fn list_folders(&self) -> Vec<String> {
        if self.folders.is_empty() {
            return vec![String::from("No folders created yet. Use /folder create <name>")];
        }

        self.folders
            .iter()
            .map(|folder| {
                let prefix = if folder.parent_id.is_some() {
                    "  "
                } else {
                    ""
                };
                let note_count = self
                    .notes
                    .iter()
                    .filter(|n| n.folder_id == Some(folder.id))
                    .count();
                format!(
                    "{}{}. #{} {:<18} ({} notes) {}",
                    prefix,
                    folder.id,
                    folder.id,
                    folder.name,
                    note_count,
                    if let Some(parent_id) = folder.parent_id {
                        format!("[in #{}]", parent_id)
                    } else {
                        String::new()
                    }
                )
            })
            .collect()
    }

    fn build_folder_tree(&self) -> Vec<String> {
        if self.folders.is_empty() {
            return vec![String::from("No folders created yet.")];
        }

        let mut lines = Vec::new();
        let root_folders: Vec<&Folder> = self
            .folders
            .iter()
            .filter(|f| f.parent_id.is_none())
            .collect();

        for (i, folder) in root_folders.iter().enumerate() {
            self.render_folder_node(folder, "", i == root_folders.len() - 1, &mut lines);
        }

        let uncategorized_count = self.notes.iter().filter(|n| n.folder_id.is_none()).count();
        if uncategorized_count > 0 {
            lines.push(format!("└── Uncategorized ({} notes)", uncategorized_count));
        }

        lines
    }

    fn render_folder_node(
        &self,
        folder: &Folder,
        prefix: &str,
        is_last: bool,
        lines: &mut Vec<String>,
    ) {
        let note_count = self
            .notes
            .iter()
            .filter(|n| n.folder_id == Some(folder.id))
            .count();

        let connector = if is_last { "└── " } else { "├── " };
        lines.push(format!(
            "{}{}#{} {} ({} notes)",
            prefix, connector, folder.id, folder.name, note_count
        ));

        let child_prefix = format!("{}{}", prefix, if is_last { "    " } else { "│   " });
        let children: Vec<&Folder> = self
            .folders
            .iter()
            .filter(|f| f.parent_id == Some(folder.id))
            .collect();

        for (i, child) in children.iter().enumerate() {
            self.render_folder_node(child, &child_prefix, i == children.len() - 1, lines);
        }
    }

    fn save_undo_state(&mut self) {
        if self.undo_stack.len() >= 100 {
            self.undo_stack.pop_back();
        }
        self.undo_stack.push_front(EditorState {
            buffer: self.editor_buffer.clone(),
            cursor: self.editor_cursor,
            scroll_offset: self.editor_scroll_offset,
        });
        self.redo_stack.clear();
    }

    fn undo(&mut self) {
        if let Some(state) = self.undo_stack.pop_front() {
            if self.redo_stack.len() >= 100 {
                self.redo_stack.pop_back();
            }
            self.redo_stack.push_front(EditorState {
                buffer: self.editor_buffer.clone(),
                cursor: self.editor_cursor,
                scroll_offset: self.editor_scroll_offset,
            });
            self.editor_buffer = state.buffer;
            self.editor_cursor = state.cursor;
            self.editor_scroll_offset = state.scroll_offset;
        }
    }

    fn redo(&mut self) {
        if let Some(state) = self.redo_stack.pop_front() {
            if self.undo_stack.len() >= 100 {
                self.undo_stack.pop_back();
            }
            self.undo_stack.push_front(EditorState {
                buffer: self.editor_buffer.clone(),
                cursor: self.editor_cursor,
                scroll_offset: self.editor_scroll_offset,
            });
            self.editor_buffer = state.buffer;
            self.editor_cursor = state.cursor;
            self.editor_scroll_offset = state.scroll_offset;
        }
    }

    fn insert_editor_text(&mut self, text: &str) {
        self.save_undo_state();
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
        self.save_undo_state();
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
        self.save_undo_state();
        let next = self.editor_buffer[self.editor_cursor..]
            .chars()
            .next()
            .map(|character| character.len_utf8())
            .unwrap_or(1);
        self.editor_buffer.drain(self.editor_cursor..self.editor_cursor + next);
    }

    fn toggle_word_wrap(&mut self) {
        self.editor_word_wrap = !self.editor_word_wrap;
    }

    fn toggle_cursor_style(&mut self) {
        self.editor_cursor_style = match self.editor_cursor_style {
            CursorStyle::Block => CursorStyle::Line,
            CursorStyle::Line => CursorStyle::Block,
        };
    }

    fn scroll_up(&mut self, lines: usize) {
        self.editor_scroll_offset = self.editor_scroll_offset.saturating_sub(lines);
    }

    fn scroll_down(&mut self, lines: usize) {
        self.editor_scroll_offset = self.editor_scroll_offset.saturating_add(lines);
    }

    fn start_search(&mut self) {
        self.search_state.active = true;
        self.search_state.query.clear();
        self.search_state.matches.clear();
        self.search_state.current_match = None;
    }

    fn cancel_search(&mut self) {
        self.search_state.active = false;
        self.search_state.query.clear();
        self.search_state.matches.clear();
        self.search_state.current_match = None;
    }

    fn search_next(&mut self) {
        if self.search_state.matches.is_empty() {
            return;
        }
        let current = self.search_state.current_match.unwrap_or(0);
        let next = if current + 1 >= self.search_state.matches.len() {
            0
        } else {
            current + 1
        };
        self.search_state.current_match = Some(next);
        if let Some(&pos) = self.search_state.matches.get(next) {
            self.editor_cursor = pos;
        }
    }

    fn search_prev(&mut self) {
        if self.search_state.matches.is_empty() {
            return;
        }
        let current = self.search_state.current_match.unwrap_or(0);
        let prev = if current == 0 {
            self.search_state.matches.len() - 1
        } else {
            current - 1
        };
        self.search_state.current_match = Some(prev);
        if let Some(&pos) = self.search_state.matches.get(prev) {
            self.editor_cursor = pos;
        }
    }

    fn update_search(&mut self) {
        self.search_state.matches.clear();
        if self.search_state.query.is_empty() {
            self.search_state.current_match = None;
            return;
        }
        let query = self.search_state.query.to_lowercase();
        let buffer_lower = self.editor_buffer.to_lowercase();
        let mut start = 0;
        while let Some(pos) = buffer_lower[start..].find(&query) {
            let absolute_pos = start + pos;
            self.search_state.matches.push(absolute_pos);
            start = absolute_pos + 1;
        }
        if !self.search_state.matches.is_empty() {
            self.search_state.current_match = Some(0);
            self.editor_cursor = self.search_state.matches[0];
        } else {
            self.search_state.current_match = None;
        }
    }

    fn handle_search_key(&mut self, key_event: KeyEvent) {
        if key_event.kind != KeyEventKind::Press {
            return;
        }
        match key_event.code {
            KeyCode::Esc | KeyCode::Enter => {
                self.cancel_search();
            }
            KeyCode::Backspace => {
                self.search_state.query.pop();
                self.update_search();
            }
            KeyCode::Char(c) => {
                self.search_state.query.push(c);
                self.update_search();
            }
            _ => {}
        }
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

    fn editor_move_up(&mut self) {
        // Find the start of the current line
        let current_pos = self.editor_cursor;
        let line_start = self.editor_buffer[..current_pos]
            .rfind('\n')
            .map(|pos| pos + 1)
            .unwrap_or(0);

        // If we're on the first line, can't move up
        if line_start == 0 {
            return;
        }

        // Calculate column position within current line
        let column = current_pos - line_start;

        // Find the start of the previous line
        let prev_line_end = line_start - 1;
        let prev_line_start = self.editor_buffer[..prev_line_end]
            .rfind('\n')
            .map(|pos| pos + 1)
            .unwrap_or(0);

        // Move cursor to the same column in the previous line (or end of line if shorter)
        let prev_line_len = prev_line_end - prev_line_start;
        let new_column = column.min(prev_line_len);
        self.editor_cursor = prev_line_start + new_column;
    }

    fn editor_move_down(&mut self) {
        // Find the end of the current line
        let current_pos = self.editor_cursor;
        let line_end = self.editor_buffer[current_pos..]
            .find('\n')
            .map(|pos| current_pos + pos)
            .unwrap_or(self.editor_buffer.len());

        // If we're on the last line, can't move down
        if line_end >= self.editor_buffer.len() {
            return;
        }

        // Calculate column position within current line
        let line_start = self.editor_buffer[..current_pos]
            .rfind('\n')
            .map(|pos| pos + 1)
            .unwrap_or(0);
        let column = current_pos - line_start;

        // Find the end of the next line
        let next_line_start = line_end + 1;
        let next_line_end = self.editor_buffer[next_line_start..]
            .find('\n')
            .map(|pos| next_line_start + pos)
            .unwrap_or(self.editor_buffer.len());

        // Move cursor to the same column in the next line (or end of line if shorter)
        let next_line_len = next_line_end - next_line_start;
        let new_column = column.min(next_line_len);
        self.editor_cursor = next_line_start + new_column;
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
        if self.search_state.active {
            self.handle_search_key(key_event);
            return;
        }

        if self.ai_overlay_visible {
            match key_event.code {
                KeyCode::Esc if key_event.kind == KeyEventKind::Press => {
                    self.close_ai_overlay();
                    return;
                }
                KeyCode::Char(' ') if key_event.kind == KeyEventKind::Press
                    && key_event.modifiers.contains(KeyModifiers::CONTROL) =>
                {
                    self.toggle_ai_overlay();
                    return;
                }
                KeyCode::Char('c')
                    if key_event.kind == KeyEventKind::Press
                        && key_event.modifiers.contains(KeyModifiers::CONTROL) =>
                {
                    self.exit_editor();
                    return;
                }
                KeyCode::Char('s')
                    if key_event.kind == KeyEventKind::Press
                        && key_event.modifiers.contains(KeyModifiers::CONTROL) =>
                {
                    self.save_editor();
                    return;
                }
                KeyCode::Char('f')
                    if key_event.kind == KeyEventKind::Press
                        && key_event.modifiers.contains(KeyModifiers::CONTROL) =>
                {
                    self.start_search();
                    return;
                }
                KeyCode::Char('z')
                    if key_event.kind == KeyEventKind::Press
                        && key_event.modifiers.contains(KeyModifiers::CONTROL) =>
                {
                    self.undo();
                    return;
                }
                KeyCode::Char('y')
                    if key_event.kind == KeyEventKind::Press
                        && key_event.modifiers.contains(KeyModifiers::CONTROL) =>
                {
                    self.redo();
                    return;
                }
                KeyCode::Char('w')
                    if key_event.kind == KeyEventKind::Press
                        && key_event.modifiers.contains(KeyModifiers::CONTROL) =>
                {
                    self.toggle_word_wrap();
                    return;
                }
                KeyCode::Char('b')
                    if key_event.kind == KeyEventKind::Press
                        && key_event.modifiers.contains(KeyModifiers::CONTROL) =>
                {
                    self.toggle_cursor_style();
                    return;
                }
                _ => {
                    self.handle_ai_input_key(key_event);
                    return;
                }
            }
        }

        match key_event.code {
            KeyCode::Char('c')
                if key_event.kind == KeyEventKind::Press
                    && key_event.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                self.exit_editor();
            }
            KeyCode::Char('s')
                if key_event.kind == KeyEventKind::Press
                    && key_event.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                self.save_editor();
            }
            KeyCode::Esc if key_event.kind == KeyEventKind::Press => self.exit_editor(),
            KeyCode::Char(' ')
                if key_event.kind == KeyEventKind::Press
                    && key_event.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                self.toggle_ai_overlay();
            }
            KeyCode::Char('f')
                if key_event.kind == KeyEventKind::Press
                    && key_event.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                self.start_search();
            }
            KeyCode::F(3) if key_event.kind == KeyEventKind::Press => {
                if key_event.modifiers.contains(KeyModifiers::SHIFT) {
                    self.search_prev();
                } else {
                    self.search_next();
                }
            }
            _ => self.handle_editor_content_key(key_event),
        }
    }

    fn handle_editor_content_key(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Enter if key_event.kind == KeyEventKind::Press => {
                self.save_undo_state();
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
            KeyCode::Up
                if matches!(key_event.kind, KeyEventKind::Press | KeyEventKind::Repeat)
                    && !key_event.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                self.editor_move_up()
            }
            KeyCode::Down
                if matches!(key_event.kind, KeyEventKind::Press | KeyEventKind::Repeat)
                    && !key_event.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                self.editor_move_down()
            }
            KeyCode::Up
                if key_event.kind == KeyEventKind::Press
                    && key_event.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                self.scroll_up(1)
            }
            KeyCode::Down
                if key_event.kind == KeyEventKind::Press
                    && key_event.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                self.scroll_down(1)
            }
            KeyCode::PageUp if key_event.kind == KeyEventKind::Press => self.scroll_up(10),
            KeyCode::PageDown if key_event.kind == KeyEventKind::Press => self.scroll_down(10),
            KeyCode::Home if key_event.kind == KeyEventKind::Press => self.editor_cursor = 0,
            KeyCode::End if key_event.kind == KeyEventKind::Press => {
                self.editor_cursor = self.editor_buffer.len()
            }
            KeyCode::Char('z')
                if key_event.kind == KeyEventKind::Press
                    && key_event.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                self.undo();
            }
            KeyCode::Char('y')
                if key_event.kind == KeyEventKind::Press
                    && key_event.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                self.redo();
            }
            KeyCode::Char('w')
                if key_event.kind == KeyEventKind::Press
                    && key_event.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                self.toggle_word_wrap();
            }
            KeyCode::Char('b')
                if key_event.kind == KeyEventKind::Press
                    && key_event.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                self.toggle_cursor_style();
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
                if !self.ai_input_buffer.trim().is_empty() {
                    self.chat_messages.push(ChatMessage {
                        role: String::from("user"),
                        content: self.ai_input_buffer.clone(),
                        timestamp: self.uptime(),
                    });
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

    fn open_ai_overlay(&mut self) {
        self.ai_overlay_visible = true;
        self.ai_overlay_pulse_ticks = 6;
        self.thinking = false;
        self.thinking_ticks_remaining = 0;
        self.ai_input_buffer.clear();
        self.ai_input_cursor = 0;
        self.last_action = String::from("Summoned the Ghost.");
    }

    fn close_ai_overlay(&mut self) {
        self.ai_overlay_visible = false;
        self.ai_overlay_pulse_ticks = 0;
        self.thinking = false;
        self.thinking_ticks_remaining = 0;
    }

    fn toggle_ai_overlay(&mut self) {
        if self.ai_overlay_visible {
            self.close_ai_overlay();
        } else {
            self.open_ai_overlay();
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
}
