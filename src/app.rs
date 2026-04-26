use std::collections::VecDeque;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::Command;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use std::thread;
use std::time::{Duration, Instant};

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseEvent, MouseEventKind};
use keyring::Entry;
use rand::{rngs::OsRng, RngCore};
use ratatui::prelude::{Color, Line, Modifier, Span, Style};
use reqwest::blocking::Client;
use sha2::{Digest, Sha256};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AiProvider {
    OpenRouter,
    Strix,
}

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
    pub remote_id: Option<String>,
    pub title: String,
    pub content: String,
    pub raw_content: String,
    pub updated_at: String,
    pub folder_id: Option<usize>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PanelMode {
    Commands,
    NoteEditor,
    FullEditor,
    AiChat,
    LoginPicker,
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
        description: "Authenticate in the browser with OpenRouter or Strix (usage: /login openrouter | /login strix | /login strix <token>)",
    },
    CommandSpec {
        name: "status",
        description: "Show session, note, and runtime health",
    },
    CommandSpec {
        name: "sync",
        description: "Pull notes from Strix into the current Aleph session",
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
        name: "logout",
        description: "Clear the active OpenRouter login",
    },
    CommandSpec {
        name: "obsidian pair",
        description: "Pair a local Obsidian vault",
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
        description: "Ask OpenRouter a question",
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

pub const THINKING_FRAMES: [&str; 10] = [
    "◡", "⊙", "◠", "⊙", "◡", "⊙", "◉", "●", "◉", "⊙",
];

const OPENROUTER_CHAT_MODEL: &str = "nvidia/nemotron-3-nano-30b-a3b:free";
const OPENROUTER_SERVICE: &str = "Aleph";
const OPENROUTER_ACCOUNT: &str = "openrouter_api_key";
const OPENROUTER_AUTH_CALLBACK: &str = "/aleph/openrouter/callback";
const OPENROUTER_AUTH_PORT: u16 = 3000;
const STRIX_SERVICE: &str = "Aleph";
const STRIX_ACCOUNT: &str = "strix_access_token";
const STRIX_AUTH_CALLBACK: &str = "/aleph/strix/callback";
const STRIX_AUTH_PORT: u16 = 43879;
const STRIX_CLIENT_ID: &str = "aleph";
const STRIX_AUTH_BASE_URL: &str = "http://localhost:3000";
const STRIX_NOTES_LIMIT: usize = 100;
const MAX_CHAT_MESSAGES: usize = 24;
const CHAT_TEXT: Color = Color::Rgb(198, 198, 210);
const CHAT_MUTED: Color = Color::Rgb(120, 122, 138);
const CHAT_ACCENT: Color = Color::Rgb(156, 146, 201);
const CHAT_ACCENT_SOFT: Color = Color::Rgb(115, 106, 155);

enum ChatStreamUpdate {
    Delta(String),
    Done,
    Error(String),
}

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
    chat_scroll_offset: usize,
    openrouter_api_key: Option<String>,
    strix_access_token: Option<String>,
    chat_stream_rx: Option<Receiver<ChatStreamUpdate>>,
    openrouter_login_rx: Option<Receiver<Result<String, String>>>,
    openrouter_login_cancel: Option<Arc<AtomicBool>>,
    strix_login_rx: Option<Receiver<Result<String, String>>>,
    strix_login_cancel: Option<Arc<AtomicBool>>,
    ai_provider: AiProvider,
    strix_logs: Vec<String>,
    streaming_buffer: String,
    streaming_active: bool,
    chat_render_cache: Vec<Line<'static>>,
    chat_render_dirty: bool,
    chat_cache_stable_len: usize,
    login_picker_selected: usize,
    ghost_stream_rx: Option<Receiver<ChatStreamUpdate>>,
    ghost_streaming: bool,
    ghost_result: Option<String>,
}

#[allow(dead_code)]
impl App {
    pub fn new() -> Self {
        let openrouter_api_key = Self::load_openrouter_api_key();
        let strix_access_token = Self::load_strix_access_token();
        let connected = openrouter_api_key.is_some() || strix_access_token.is_some();
        let ai_provider = if openrouter_api_key.is_some() {
            AiProvider::OpenRouter
        } else if strix_access_token.is_some() {
            AiProvider::Strix
        } else {
            AiProvider::OpenRouter
        };

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
                    remote_id: None,
                    title: String::from("Strix gateway"),
                    content: String::from(
                        "Build a stable gateway that normalizes auth, streaming, and note operations.",
                    ),
                    raw_content: String::from(
                        "Build a stable gateway that normalizes auth, streaming, and note operations.",
                    ),
                    updated_at: String::from("seed"),
                    folder_id: Some(3),
                },
                Note {
                    id: 2,
                    remote_id: None,
                    title: String::from("Note editor"),
                    content: String::from(
                        "Use a terminal editor for quick edits, then move larger writes into the Strix product.",
                    ),
                    raw_content: String::from(
                        "Use a terminal editor for quick edits, then move larger writes into the Strix product.",
                    ),
                    updated_at: String::from("seed"),
                    folder_id: Some(3),
                },
                Note {
                    id: 3,
                    remote_id: None,
                    title: String::from("MCP server"),
                    content: String::from(
                        "Expose Aleph as an MCP bridge so external agents can use Strix knowledge.",
                    ),
                    raw_content: String::from(
                        "Expose Aleph as an MCP bridge so external agents can use Strix knowledge.",
                    ),
                    updated_at: String::from("seed"),
                    folder_id: None,
                },
                Note {
                    id: 4,
                    remote_id: None,
                    title: String::from("Feature ideas"),
                    content: String::from(
                        "Folder navigation, search within folders, nested folders like Strix.",
                    ),
                    raw_content: String::from(
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
            chat_scroll_offset: 0,
            openrouter_api_key,
            strix_access_token,
            chat_stream_rx: None,
            openrouter_login_rx: None,
            openrouter_login_cancel: None,
            strix_login_rx: None,
            strix_login_cancel: None,
            ai_provider,
            strix_logs: Vec::new(),
            streaming_buffer: String::new(),
            streaming_active: false,
            chat_render_cache: Vec::new(),
            chat_render_dirty: false,
            chat_cache_stable_len: 0,
            login_picker_selected: 0,
            ghost_stream_rx: None,
            ghost_streaming: false,
            ghost_result: None,
        };

        if app.strix_access_token.is_some() {
            if let Ok(notes) = Self::load_cached_strix_notes() {
                if !notes.is_empty() {
                    app.notes = notes;
                    app.selected_note = 0;
                    app.add_strix_log("Loaded cached Strix notes");
                    app.last_action = String::from("Loaded cached Strix notes. Run /sync to refresh.");
                }
            }
        }

        app.rebuild_chat_render_cache();
        app
    }

    pub fn run_cli_command(&mut self, args: &[String]) -> Result<Vec<String>, String> {
        if args.is_empty() {
            return Ok(vec![
                String::from("Usage: aleph notes <list|search|read|write|append|create> ..."),
                String::from("Examples:"),
                String::from("  aleph notes search roadmap"),
                String::from("  aleph notes read <id>"),
                String::from("  aleph notes write <id> -   # content from stdin"),
            ]);
        }

        let area = args[0].as_str();
        if area == "sync" {
            let count = self.sync_strix_notes()?;
            return Ok(vec![format!("Synced {} notes from Strix.", count)]);
        }

        if area != "notes" && area != "note" {
            return Err(format!("Unknown Aleph CLI area '{}'. Try 'notes'.", area));
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
                            note.remote_id.as_deref().unwrap_or("local-only"),
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
                Ok(vec![
                    format!("# {}", note.title),
                    format!("Strix ID: {}", note.remote_id.as_deref().unwrap_or("local-only")),
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
                    return Err(String::from("Provide content or pass '-' to read content from stdin."));
                }
                self.ensure_cached_strix_notes_loaded();
                let mut note = self
                    .resolve_note_index(id)
                    .and_then(|index| self.notes.get(index).cloned())
                    .map(Ok)
                    .unwrap_or_else(|| self.load_strix_note(id, true))?;
                note.content = content;
                let updated = self.update_strix_note(&note)?;
                self.upsert_synced_note(updated.clone());
                Ok(vec![format!(
                    "Updated {} ({})",
                    updated.title,
                    updated.remote_id.as_deref().unwrap_or("local-only")
                )])
            }
            "append" => {
                let id = args
                    .get(2)
                    .ok_or_else(|| String::from("Usage: aleph notes append <id|title> <content>"))?;
                let content = args.get(3..).unwrap_or(&[]).join(" ");
                if content.is_empty() {
                    return Err(String::from("Provide content or pass '-' to read content from stdin."));
                }
                self.ensure_cached_strix_notes_loaded();
                let mut note = self
                    .resolve_note_index(id)
                    .and_then(|index| self.notes.get(index).cloned())
                    .map(Ok)
                    .unwrap_or_else(|| self.load_strix_note(id, true))?;
                if !note.content.is_empty() {
                    note.content.push('\n');
                }
                note.content.push_str(&content);
                let updated = self.update_strix_note(&note)?;
                self.upsert_synced_note(updated.clone());
                Ok(vec![format!(
                    "Appended to {} ({})",
                    updated.title,
                    updated.remote_id.as_deref().unwrap_or("local-only")
                )])
            }
            "create" => {
                let title = args
                    .get(2)
                    .map(|title| title.as_str())
                    .filter(|title| !title.trim().is_empty())
                    .unwrap_or("Untitled note");
                let content = args.get(3..).unwrap_or(&[]).join(" ");
                let note = self.create_strix_note(title, &content)?;
                self.upsert_synced_note(note.clone());
                Ok(vec![format!(
                    "Created {} ({})",
                    note.title,
                    note.remote_id.as_deref().unwrap_or("local-only")
                )])
            }
            _ => Err(format!("Unknown notes action '{}'.", action)),
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
                            self.rebuild_chat_render_cache();
                            self.set_result_panel(
                                "OpenRouter login",
                                vec![
                                    String::from("OpenRouter browser login completed successfully."),
                                    String::from("The API key has been stored locally."),
                                    String::from("You can start chatting right away."),
                                ],
                            );
                            self.last_action = String::from("Connected to OpenRouter via browser login.");
                        }
                        Err(error) => {
                            self.openrouter_api_key = None;
                            self.refresh_connection_state();
                            self.set_result_panel("OpenRouter login failed", vec![error]);
                            self.last_action = String::from("OpenRouter login failed.");
                        }
                    }

                    self.openrouter_login_rx = None;
                    self.openrouter_login_cancel = None;
                    login_finished = true;
                }
                Ok(Err(error)) => {
                    self.refresh_connection_state();
                    self.set_result_panel("OpenRouter login failed", vec![error]);
                    self.last_action = String::from("OpenRouter login failed.");
                    self.openrouter_login_rx = None;
                    self.openrouter_login_cancel = None;
                    login_finished = true;
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.refresh_connection_state();
                    self.set_result_panel(
                        "OpenRouter login failed",
                        vec![String::from("The browser login flow disconnected before completion.")],
                    );
                    self.last_action = String::from("OpenRouter login disconnected.");
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
                            self.refresh_connection_state();
                            self.add_strix_log("Browser login completed successfully");
                            self.set_result_panel(
                                "Strix login",
                                vec![
                                    String::from("Strix browser login completed successfully."),
                                    String::from("The native app access token has been stored locally."),
                                    String::from("Aleph can now call Strix-native APIs as they come online."),
                                ],
                            );
                            self.last_action = String::from("Connected to Strix via browser login.");
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
                        vec![String::from("The browser login flow disconnected before completion.")],
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
                    self.streaming_active = true;
                    self.streaming_buffer.push_str(&chunk);
                    if let Some(message) = self
                        .chat_messages
                        .iter_mut()
                        .rev()
                        .find(|message| message.role == "assistant")
                    {
                        message.content.push_str(&chunk);
                    }
                    self.chat_render_dirty = true;
                    self.thinking = true;
                }
                Ok(ChatStreamUpdate::Done) => {
                    if self.streaming_buffer.trim().is_empty() {
                        if let Some(message) = self
                            .chat_messages
                            .iter_mut()
                            .rev()
                            .find(|message| message.role == "assistant")
                        {
                            message.content = String::from("OpenRouter returned no content.");
                        }
                    }

                    self.streaming_buffer.clear();
                    self.streaming_active = false;
                    self.rebuild_chat_render_cache();
                    self.chat_render_dirty = false;
                    self.thinking = false;
                    self.thinking_ticks_remaining = 0;
                    self.chat_stream_rx = None;
                    self.last_action = String::from("OpenRouter response received.");
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
                            message.content = format!("OpenRouter chat failed: {}", error);
                        } else {
                            message.content.push_str("\n\n");
                            message.content.push_str(&format!("[OpenRouter error: {}]", error));
                        }
                    } else {
                        self.push_chat_message("assistant", format!("OpenRouter chat failed: {}", error));
                    }

                    self.streaming_buffer.clear();
                    self.streaming_active = false;
                    self.rebuild_chat_render_cache();
                    self.chat_render_dirty = false;
                    self.thinking = false;
                    self.thinking_ticks_remaining = 0;
                    self.chat_stream_rx = None;
                    self.last_action = String::from("OpenRouter request failed.");
                    stream_finished = true;
                }
                Err(TryRecvError::Empty) => {
                    self.thinking = true;
                    break;
                }
                Err(TryRecvError::Disconnected) => {
                    if let Some(message) = self
                        .chat_messages
                        .iter_mut()
                        .rev()
                        .find(|message| message.role == "assistant")
                    {
                        message.content = String::from("OpenRouter chat disconnected before a response arrived.");
                    }

                    self.streaming_buffer.clear();
                    self.streaming_active = false;
                    self.rebuild_chat_render_cache();
                    self.chat_render_dirty = false;
                    self.thinking = false;
                    self.thinking_ticks_remaining = 0;
                    self.chat_stream_rx = None;
                    self.last_action = String::from("OpenRouter request disconnected.");
                    stream_finished = true;
                }
            }
        }

        if self.chat_render_dirty {
            self.rebuild_chat_render_cache_streaming();
            self.chat_render_dirty = false;
        }

        self.process_ghost_stream();

        if self.ai_overlay_visible && self.ai_overlay_pulse_ticks > 0 {
            self.ai_overlay_pulse_ticks -= 1;
        }
        if self.save_shimmer_ticks > 0 {
            self.save_shimmer_ticks -= 1;
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

    pub fn chat_scroll_offset(&self) -> usize {
        self.chat_scroll_offset
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

    pub fn ai_provider(&self) -> AiProvider {
        self.ai_provider
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

    pub fn login_picker_selected(&self) -> usize {
        self.login_picker_selected
    }

    pub fn is_login_picker(&self) -> bool {
        self.panel_mode == PanelMode::LoginPicker
    }

    pub fn is_ghost_streaming(&self) -> bool {
        self.ghost_streaming
    }

    pub fn ghost_result(&self) -> Option<&str> {
        self.ghost_result.as_deref()
    }

    pub fn is_openrouter_login_pending(&self) -> bool {
        self.openrouter_login_rx.is_some()
    }

    pub fn is_strix_login_pending(&self) -> bool {
        self.strix_login_rx.is_some()
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
        if self.is_login_picker() {
            self.handle_login_picker_key(key_event);
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
        if self.is_ai_chat() {
            match mouse_event.kind {
                MouseEventKind::ScrollUp => self.scroll_chat_up(3),
                MouseEventKind::ScrollDown => self.scroll_chat_down(3),
                _ => {}
            }
            return;
        }

        if self.is_full_editor() && self.ai_overlay_visible && matches!(mouse_event.kind, MouseEventKind::Down(_)) {
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
                    if self.start_chat_turn(msg.to_string()) {
                        self.chat_input_buffer.clear();
                        self.chat_input_cursor = 0;
                    }
                }
            }
            KeyCode::PageUp if key_event.kind == KeyEventKind::Press => self.scroll_chat_up(10),
            KeyCode::PageDown if key_event.kind == KeyEventKind::Press => self.scroll_chat_down(10),
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
        let raw = self.prompt.trim().to_string();
        if raw.is_empty() {
            self.last_action = String::from("Type a query, command, or press Ctrl+C to quit.");
            return;
        }

        if !raw.starts_with('/') {
            let query = raw.clone();
            if self.start_chat_turn(query) {
                self.history.push(raw);
                self.history_index = None;
                self.reset_prompt();
            }
            return;
        }

        let prompt = Self::expand_command_alias(&self.normalized_prompt());
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

        if args.trim().is_empty() && Self::command_expects_argument(command.name) {
            self.prompt = format!("/{} ", command.name);
            self.cursor = self.prompt.len();
            self.last_action = format!("Add a target or text for /{}.", command.name);
            return;
        }

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

    fn expand_command_alias(prompt: &str) -> String {
        let trimmed = prompt.trim();
        let aliases = [
            ("find", "search"),
            ("open", "note read"),
            ("read", "note read"),
            ("edit", "note edit"),
            ("new", "note create"),
            ("create", "note create"),
            ("list", "note list"),
            ("ls", "note list"),
        ];

        for (alias, command) in aliases {
            if trimmed == alias {
                return command.to_string();
            }
            if let Some(rest) = trimmed.strip_prefix(alias) {
                if rest.starts_with(char::is_whitespace) {
                    return format!("{}{}", command, rest);
                }
            }
        }

        trimmed.to_string()
    }

    fn command_expects_argument(command: &str) -> bool {
        matches!(
            command,
            "search"
                | "ask"
                | "note create"
                | "note append"
                | "note move"
                | "folder create"
                | "folder delete"
                | "folder notes"
                | "memory save"
                | "memory search"
                | "canvas show"
                | "canvas export"
                | "darwin run"
        )
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
                                format!("You are already connected to {}.", provider),
                                String::from("Use /logout first if you want to switch providers or re-authenticate."),
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
                            // /login openrouter -> browser login
                            if self.start_openrouter_browser_login() {
                                return;
                            }
                            self.set_result_panel(
                                "OpenRouter login failed",
                                vec![String::from("Unable to start the browser-based OpenRouter login flow.")],
                            );
                            self.last_action = String::from("OpenRouter login failed.");
                            return;
                        } else if maybe_provider == "strix" {
                            if self.start_strix_browser_login() {
                                return;
                            }
                            self.set_result_panel(
                                "Strix login failed",
                                vec![String::from("Unable to start the browser-based Strix login flow.")],
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
                                vec![String::from("Unable to start the browser-based Strix login flow.")],
                            );
                            self.last_action = String::from("Strix login failed.");
                            return;
                        }

                        self.add_strix_log("Saving Strix access token");
                        match self.store_strix_access_token(token) {
                            Ok(()) => {
                                self.strix_access_token = Some(token.to_string());
                                self.refresh_connection_state();
                                self.add_strix_log("Connected to Strix successfully");
                                self.set_result_panel(
                                    "Strix login",
                                    vec![
                                        String::from("Strix authentication configured."),
                                        String::from("The native access token has been stored locally."),
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
                                    "OpenRouter login",
                                    vec![
                                        String::from("OpenRouter login saved locally."),
                                        String::from("AI chat is ready to use now."),
                                    ],
                                );
                                self.last_action = String::from("Connected to OpenRouter.");
                            }
                            Err(error) => {
                                self.openrouter_api_key = None;
                                self.refresh_connection_state();
                                self.set_result_panel("OpenRouter login failed", vec![error]);
                                self.last_action = String::from("OpenRouter login failed.");
                            }
                        }
                    }
                }
            }
            "logout" => {
                self.openrouter_api_key = None;
                self.strix_access_token = None;
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
                        String::from("The current OpenRouter login has been cleared."),
                        String::from("The current Strix login has also been cleared."),
                        String::from("Use /login openrouter or /login strix to connect again."),
                    ],
                );
                self.last_action = String::from("Disconnected from providers.");
            }
            "obsidian pair" => {
                self.set_result_panel(
                    "Obsidian pairing",
                    vec![
                        String::from("Pair a local Obsidian vault so Aleph can read notes."),
                        String::from("This is a UI placeholder for the future vault picker and sync flow."),
                    ],
                );
                self.last_action = String::from("Opened Obsidian pairing.");
            }
            "status" => {
                self.set_result_panel(
                    "Status",
                    vec![
                        format!("OpenRouter: {}", if self.is_openrouter_connected() { "connected" } else { "offline" }),
                        format!("Strix: {}", if self.is_strix_connected() { "connected" } else { "offline" }),
                        format!("Notes: {}", self.notes.len()),
                        format!("Cache: {}", Self::strix_cache_path().display()),
                        format!("Memories: {}", self.memories.len()),
                        format!("Canvases: {}", self.canvases.len()),
                        format!("Uptime: {}", self.uptime()),
                    ],
                );
                self.last_action = String::from("Refreshed provider status.");
            }
            "sync" => {
                match self.sync_strix_notes() {
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
                }
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
                        String::from("AI chat: OpenRouter-backed"),
                        String::from("Strix auth: OAuth-style browser flow with PKCE"),
                        String::from("Login: /login openrouter, /login strix, or provider token env vars"),
                    ],
                );
                self.last_action = String::from("Opened config summary.");
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
                        lines.push(format!("No cached Strix matches for '{}'. Run /sync to refresh.", query));
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
                    vec![String::from("Ask OpenRouter a question after the command, for example: /ask what should ship next?")]
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
            "note list" => {
                if self.is_strix_connected() {
                    self.ensure_cached_strix_notes_loaded();
                }
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
                            "{:>2}. #{} {:<14} {}{}{}",
                            index + 1,
                            note.id,
                            if note.title.len() > 14 { format!("{}…", &note.title[..13]) } else { note.title.clone() },
                            folder_indicator,
                            Self::preview_text(&note.content, 32),
                            note.remote_id.as_deref().map(|id| format!(" [{}]", id)).unwrap_or_default()
                        )
                    })
                    .collect::<Vec<_>>();

                self.set_result_panel(format!("Notes — {}", folder_name), lines);
                self.last_action = String::from("Listed notes.");
            }
            "note read" => {
                if self.is_strix_connected() {
                    self.ensure_cached_strix_notes_loaded();
                }
                if self.is_strix_connected() && !args.trim().is_empty() && self.resolve_note_index(args.trim()).is_none() {
                    if let Ok(note) = self.load_strix_note(args.trim(), true) {
                        self.upsert_synced_note(note);
                    }
                }
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
                    format!("Strix ID: {}", note.remote_id.as_deref().unwrap_or("local-only")),
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
                let mut note = Note {
                    id: note_id,
                    remote_id: None,
                    title: title.clone(),
                    content: String::new(),
                    raw_content: String::new(),
                    updated_at: String::from("draft"),
                    folder_id: self.current_folder_id,
                };
                if self.is_strix_connected() {
                    match self.create_strix_note(&title, "") {
                        Ok(remote_note) => note = remote_note,
                        Err(error) => {
                            self.set_result_panel("Strix create failed", vec![error]);
                            self.last_action = String::from("Strix note create failed.");
                            return;
                        }
                    }
                }
                self.notes.push(note);
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
                if let Err(error) = self.push_note_to_strix(index) {
                    self.set_result_panel("Strix push failed", vec![error]);
                    self.last_action = String::from("Strix note append failed.");
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

                if self.is_strix_connected() {
                    if let Some(remote_id) = self.notes[index].remote_id.clone() {
                        if let Ok(note) = self.load_strix_note(&remote_id, true) {
                            let mut refreshed = note;
                            refreshed.id = self.notes[index].id;
                            refreshed.folder_id = self.notes[index].folder_id;
                            self.notes[index] = refreshed;
                            let _ = Self::save_cached_strix_notes(&self.notes);
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

    fn push_chat_message(&mut self, role: impl Into<String>, content: impl Into<String>) {
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

    fn scroll_chat_up(&mut self, lines: usize) {
        self.chat_scroll_offset = self.chat_scroll_offset.saturating_add(lines);
    }

    fn scroll_chat_down(&mut self, lines: usize) {
        self.chat_scroll_offset = self.chat_scroll_offset.saturating_sub(lines);
    }

    fn add_strix_log(&mut self, message: impl Into<String>) {
        let timestamp = self.uptime();
        self.strix_logs.push(format!("[{}] {}", timestamp, message.into()));
        // Keep only last 50 log entries
        if self.strix_logs.len() > 50 {
            self.strix_logs.drain(0..self.strix_logs.len() - 50);
        }
    }

    fn set_ai_provider(&mut self, provider: AiProvider) {
        self.ai_provider = provider;
        self.add_strix_log(format!("Switched to {:?} provider", provider));
    }

    fn clear_strix_logs(&mut self) {
        self.strix_logs.clear();
    }

    fn refresh_connection_state(&mut self) {
        self.connected = self.openrouter_api_key.is_some() || self.strix_access_token.is_some();
    }

    fn start_openrouter_browser_login(&mut self) -> bool {
        if self.openrouter_login_rx.is_some() {
            self.last_action = String::from("OpenRouter login is already running. Use /logout to cancel it first.");
            return false;
        }

        let (sender, receiver) = mpsc::channel();
        let cancel_flag = Arc::new(AtomicBool::new(false));
        self.openrouter_login_rx = Some(receiver);
        self.openrouter_login_cancel = Some(cancel_flag.clone());
        self.set_result_panel(
            "OpenRouter browser login",
            vec![
                String::from("A browser window will open for OpenRouter sign-in."),
                String::from("After you authorize Aleph, the API key will be stored locally."),
                String::from("If the browser does not open, copy the auth URL from the terminal."),
            ],
        );
        self.last_action = String::from("Starting OpenRouter browser login.");

        thread::spawn(move || {
            let result = Self::run_openrouter_browser_login_flow(cancel_flag);
            let _ = sender.send(result);
        });

        true
    }

    fn run_openrouter_browser_login_flow(cancel_flag: Arc<AtomicBool>) -> Result<String, String> {
        let (code_verifier, code_challenge) = Self::build_pkce_pair();
        let callback_nonce = Self::build_login_nonce();
        let callback_path = format!("{}/{}", OPENROUTER_AUTH_CALLBACK, callback_nonce);
        let callback_url = format!("http://127.0.0.1:3000{}", callback_path);
        let auth_url = format!(
            "https://openrouter.ai/auth?callback_url={}&code_challenge={}&code_challenge_method=S256",
            urlencoding::encode(&callback_url),
            urlencoding::encode(&code_challenge),
        );

        let listener = TcpListener::bind(("127.0.0.1", OPENROUTER_AUTH_PORT))
            .map_err(|error| format!("failed to bind local OpenRouter callback listener: {}", error))?;
        listener
            .set_nonblocking(true)
            .map_err(|error| format!("failed to configure the callback listener: {}", error))?;

        Self::open_browser(&auth_url)?;

        let deadline = Instant::now() + Duration::from_secs(600);
        let (mut stream, _) = loop {
            if cancel_flag.load(Ordering::Relaxed) {
                return Err(String::from("OpenRouter browser login was canceled."));
            }

            match listener.accept() {
                Ok(connection) => break connection,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    if Instant::now() >= deadline {
                        return Err(String::from("OpenRouter browser login timed out waiting for the callback."));
                    }
                    thread::sleep(Duration::from_millis(100));
                }
                Err(error) => {
                    return Err(format!("failed to accept OpenRouter callback: {}", error));
                }
            }
        };

        let code = Self::read_openrouter_callback_code(&mut stream, &callback_path)?;

        Self::write_openrouter_callback_response(
            &mut stream,
            "OpenRouter login completed. You can return to Aleph now.",
        )?;

        if cancel_flag.load(Ordering::Relaxed) {
            return Err(String::from("OpenRouter browser login was canceled."));
        }

        Self::exchange_openrouter_code_for_key(&code, &code_verifier)
    }

    fn start_strix_browser_login(&mut self) -> bool {
        if self.strix_login_rx.is_some() {
            self.last_action = String::from("Strix login is already running. Use /logout to cancel it first.");
            return false;
        }

        let (sender, receiver) = mpsc::channel();
        let cancel_flag = Arc::new(AtomicBool::new(false));
        self.strix_login_rx = Some(receiver);
        self.strix_login_cancel = Some(cancel_flag.clone());
        self.set_result_panel(
            "Strix browser login",
            vec![
                String::from("A browser window will open for Strix sign-in."),
                String::from("After you authenticate, Aleph receives a native app token via localhost."),
                format!("Server: {}", Self::strix_auth_base_url()),
            ],
        );
        self.add_strix_log("Starting browser login");
        self.last_action = String::from("Starting Strix browser login.");

        thread::spawn(move || {
            let result = Self::run_strix_browser_login_flow(cancel_flag);
            let _ = sender.send(result);
        });

        true
    }

    fn run_strix_browser_login_flow(cancel_flag: Arc<AtomicBool>) -> Result<String, String> {
        let (code_verifier, code_challenge) = Self::build_pkce_pair();
        let state = Self::build_login_nonce();
        let callback_path = format!("{}/{}", STRIX_AUTH_CALLBACK, state);
        let redirect_uri = format!("http://127.0.0.1:{}{}", STRIX_AUTH_PORT, callback_path);
        let auth_base_url = Self::strix_auth_base_url();
        let auth_url = format!(
            "{}/api/auth/native/start?client_id={}&redirect_uri={}&response_type=code&scope={}&state={}&code_challenge={}&code_challenge_method=S256",
            auth_base_url,
            urlencoding::encode(STRIX_CLIENT_ID),
            urlencoding::encode(&redirect_uri),
            urlencoding::encode("native:session"),
            urlencoding::encode(&state),
            urlencoding::encode(&code_challenge),
        );

        let listener = TcpListener::bind(("127.0.0.1", STRIX_AUTH_PORT))
            .map_err(|error| format!("failed to bind local Strix callback listener: {}", error))?;
        listener
            .set_nonblocking(true)
            .map_err(|error| format!("failed to configure the Strix callback listener: {}", error))?;

        Self::open_browser(&auth_url)?;

        let deadline = Instant::now() + Duration::from_secs(600);
        let (mut stream, _) = loop {
            if cancel_flag.load(Ordering::Relaxed) {
                return Err(String::from("Strix browser login was canceled."));
            }

            match listener.accept() {
                Ok(connection) => break connection,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    if Instant::now() >= deadline {
                        return Err(String::from("Strix browser login timed out waiting for the callback."));
                    }
                    thread::sleep(Duration::from_millis(100));
                }
                Err(error) => {
                    return Err(format!("failed to accept Strix callback: {}", error));
                }
            }
        };

        let request_path = Self::read_oauth_callback_path(&mut stream, &callback_path, "Strix")?;
        if let Some(error) = Self::query_parameter(&request_path, "error") {
            return Err(format!("Strix login returned an error: {}", error));
        }
        let returned_state = Self::query_parameter(&request_path, "state")
            .ok_or_else(|| String::from("Strix callback did not include state"))?;
        if returned_state != state {
            return Err(String::from("Strix callback state did not match the login request."));
        }
        let code = Self::query_parameter(&request_path, "code")
            .ok_or_else(|| String::from("Strix callback did not include an authorization code"))?;

        Self::write_oauth_callback_response(
            &mut stream,
            "Strix login complete",
            "Strix login completed. You can return to Aleph now.",
            "Strix",
        )?;

        if cancel_flag.load(Ordering::Relaxed) {
            return Err(String::from("Strix browser login was canceled."));
        }

        Self::exchange_strix_code_for_token(&auth_base_url, &code, &code_verifier, &redirect_uri)
    }

    fn build_pkce_pair() -> (String, String) {
        let mut bytes = [0u8; 32];
        OsRng.fill_bytes(&mut bytes);
        let verifier = URL_SAFE_NO_PAD.encode(bytes);
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
        (verifier, challenge)
    }

    fn build_login_nonce() -> String {
        let mut bytes = [0u8; 12];
        OsRng.fill_bytes(&mut bytes);
        URL_SAFE_NO_PAD.encode(bytes)
    }

    fn open_browser(url: &str) -> Result<(), String> {
        if cfg!(target_os = "windows") {
            Command::new("cmd")
                .arg("/C")
                .arg("start")
                .arg("")
                .arg(url.replace("&", "^&"))
                .spawn()
                .map_err(|error| format!("failed to open the browser: {}", error))?;
            return Ok(());
        }

        if cfg!(target_os = "macos") {
            Command::new("open")
                .arg(url)
                .spawn()
                .map_err(|error| format!("failed to open the browser: {}", error))?;
            return Ok(());
        }

        Command::new("xdg-open")
            .arg(url)
            .spawn()
            .map_err(|error| format!("failed to open the browser: {}", error))?;
        Ok(())
    }

    fn read_openrouter_callback_code(
        stream: &mut std::net::TcpStream,
        expected_path: &str,
    ) -> Result<String, String> {
        Self::read_oauth_callback_parameter(stream, expected_path, "code", "OpenRouter")
    }

    fn read_oauth_callback_parameter(
        stream: &mut std::net::TcpStream,
        expected_path: &str,
        parameter: &str,
        provider: &str,
    ) -> Result<String, String> {
        let request_path = Self::read_oauth_callback_path(stream, expected_path, provider)?;
        if let Some(error) = Self::query_parameter(&request_path, "error") {
            return Err(format!("{} login returned an error: {}", provider, error));
        }

        Self::query_parameter(&request_path, parameter)
            .ok_or_else(|| format!("{} callback did not include {}", provider, parameter))
    }

    fn read_oauth_callback_path(
        stream: &mut std::net::TcpStream,
        expected_path: &str,
        provider: &str,
    ) -> Result<String, String> {
        let request_path = {
            let mut reader = BufReader::new(stream);
            let mut request_line = String::new();
            reader
                .read_line(&mut request_line)
                .map_err(|error| format!("failed to read {} callback request: {}", provider, error))?;

            let mut header = String::new();
            loop {
                header.clear();
                let bytes_read = reader
                    .read_line(&mut header)
                    .map_err(|error| format!("failed to read {} callback headers: {}", provider, error))?;
                if bytes_read == 0 || header == "\r\n" {
                    break;
                }
            }

            request_line
                .split_whitespace()
                .nth(1)
                .ok_or_else(|| format!("{} callback request did not include a path", provider))?
                .to_string()
        };

        let request_path_only = request_path.split('?').next().unwrap_or(&request_path);
        if request_path_only != expected_path {
            return Err(format!("{} callback arrived on an unexpected path.", provider));
        }

        Ok(request_path)
    }

    fn write_openrouter_callback_response(
        stream: &mut std::net::TcpStream,
        message: &str,
    ) -> Result<(), String> {
        Self::write_oauth_callback_response(
            stream,
            "OpenRouter login complete",
            message,
            "OpenRouter",
        )
    }

    fn write_oauth_callback_response(
        stream: &mut std::net::TcpStream,
        title: &str,
        message: &str,
        provider: &str,
    ) -> Result<(), String> {
        let body = format!(
            "<html><body style=\"font-family: sans-serif; padding: 2rem;\"><h1>{}</h1><p>{}</p></body></html>",
            title,
            message
        );
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        stream
            .write_all(response.as_bytes())
            .map_err(|error| format!("failed to write {} callback response: {}", provider, error))
    }

    fn exchange_openrouter_code_for_key(code: &str, code_verifier: &str) -> Result<String, String> {
        let payload = serde_json::json!({
            "code": code,
            "code_verifier": code_verifier,
            "code_challenge_method": "S256",
        });

        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|error| format!("failed to build HTTP client: {}", error))?;

        let response = client
            .post("https://openrouter.ai/api/v1/auth/keys")
            .json(&payload)
            .send()
            .map_err(|error| format!("failed to exchange the OpenRouter authorization code: {}", error))?;

        let status = response.status();
        let body = response
            .text()
            .map_err(|error| format!("failed to read OpenRouter auth response: {}", error))?;

        if !status.is_success() {
            return Err(format!("{}: {}", status, body));
        }

        let value: serde_json::Value = serde_json::from_str(&body)
            .map_err(|error| format!("failed to parse OpenRouter auth response: {}", error))?;

        value
            .get("key")
            .and_then(|key| key.as_str())
            .map(|key| key.trim().to_string())
            .filter(|key| !key.is_empty())
            .ok_or_else(|| String::from("OpenRouter auth response did not include an API key"))
    }


    fn exchange_strix_code_for_token(
        auth_base_url: &str,
        code: &str,
        code_verifier: &str,
        redirect_uri: &str,
    ) -> Result<String, String> {
        let payload = serde_json::json!({
            "grant_type": "authorization_code",
            "code": code,
            "code_verifier": code_verifier,
            "client_id": STRIX_CLIENT_ID,
            "redirect_uri": redirect_uri,
        });

        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|error| format!("failed to build HTTP client: {}", error))?;

        let response = client
            .post(format!("{}/api/auth/native/token", auth_base_url))
            .json(&payload)
            .send()
            .map_err(|error| format!("failed to exchange the Strix authorization code: {}", error))?;

        let status = response.status();
        let body = response
            .text()
            .map_err(|error| format!("failed to read Strix auth response: {}", error))?;

        if !status.is_success() {
            return Err(format!("{}: {}", status, body));
        }

        let value: serde_json::Value = serde_json::from_str(&body)
            .map_err(|error| format!("failed to parse Strix auth response: {}", error))?;

        value
            .get("access_token")
            .and_then(|token| token.as_str())
            .map(|token| token.trim().to_string())
            .filter(|token| !token.is_empty())
            .ok_or_else(|| String::from("Strix auth response did not include an access token"))
    }

    fn query_parameter(path: &str, name: &str) -> Option<String> {
        let query = path.split_once('?')?.1;

        for pair in query.split('&') {
            let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
            if key == name {
                return urlencoding::decode(value).ok().map(|decoded| decoded.into_owned());
            }
        }

        None
    }

    fn parse_chat_markdown_spans_owned(text: &str) -> Vec<Span<'static>> {
        let mut spans = Vec::new();
        let mut remaining = text;

        while !remaining.is_empty() {
            if let Some(pos) = remaining.find("**") {
                if pos > 0 {
                    spans.push(Span::raw(remaining[..pos].to_string()));
                }
                remaining = &remaining[pos + 2..];
                if let Some(end_pos) = remaining.find("**") {
                    spans.push(Span::styled(
                        remaining[..end_pos].to_string(),
                        Style::default().add_modifier(Modifier::BOLD),
                    ));
                    remaining = &remaining[end_pos + 2..];
                } else {
                    spans.push(Span::raw("**"));
                    spans.push(Span::raw(remaining.to_string()));
                    break;
                }
            } else if let Some(pos) = remaining.find('*') {
                if pos > 0 {
                    spans.push(Span::raw(remaining[..pos].to_string()));
                }
                remaining = &remaining[pos + 1..];
                if let Some(end_pos) = remaining.find('*') {
                    spans.push(Span::styled(
                        remaining[..end_pos].to_string(),
                        Style::default().add_modifier(Modifier::ITALIC),
                    ));
                    remaining = &remaining[end_pos + 1..];
                } else {
                    spans.push(Span::raw("*"));
                    spans.push(Span::raw(remaining.to_string()));
                    break;
                }
            } else if let Some(pos) = remaining.find('`') {
                if pos > 0 {
                    spans.push(Span::raw(remaining[..pos].to_string()));
                }
                remaining = &remaining[pos + 1..];
                if let Some(end_pos) = remaining.find('`') {
                    spans.push(Span::styled(
                        remaining[..end_pos].to_string(),
                        Style::default().fg(CHAT_ACCENT_SOFT),
                    ));
                    remaining = &remaining[end_pos + 1..];
                } else {
                    spans.push(Span::raw("`"));
                    spans.push(Span::raw(remaining.to_string()));
                    break;
                }
            } else {
                spans.push(Span::raw(remaining.to_string()));
                break;
            }
        }

        if spans.is_empty() {
            spans.push(Span::raw(text.to_string()));
        }

        spans
    }

    fn render_chat_markdown_line_owned(line: &str) -> Line<'static> {
        let mut spans = Vec::new();
        let mut remaining = line;
        let trimmed = line.trim_start();
        let indent_len = line.len() - trimmed.len();

        if trimmed.starts_with("# ") {
            spans.push(Span::styled(
                line[..indent_len + 2].to_string(),
                Style::default().fg(CHAT_ACCENT_SOFT),
            ));
            spans.push(Span::styled(
                trimmed[2..].to_string(),
                Style::default().fg(CHAT_TEXT).add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
            ));
            return Line::from(spans);
        } else if trimmed.starts_with("## ") {
            spans.push(Span::styled(
                line[..indent_len + 3].to_string(),
                Style::default().fg(CHAT_MUTED),
            ));
            spans.push(Span::styled(
                trimmed[3..].to_string(),
                Style::default().fg(CHAT_TEXT).add_modifier(Modifier::BOLD),
            ));
            return Line::from(spans);
        } else if trimmed.starts_with("### ") {
            spans.push(Span::styled(
                line[..indent_len + 4].to_string(),
                Style::default().fg(CHAT_MUTED),
            ));
            spans.push(Span::styled(
                trimmed[4..].to_string(),
                Style::default().fg(CHAT_TEXT).add_modifier(Modifier::BOLD | Modifier::ITALIC),
            ));
            return Line::from(spans);
        } else if let Some(stripped) = trimmed.strip_prefix("- ") {
            if indent_len > 0 {
                spans.push(Span::raw(line[..indent_len].to_string()));
            }
            spans.push(Span::styled("• ", Style::default().fg(CHAT_ACCENT)));
            remaining = stripped;
        } else if let Some(stripped) = trimmed.strip_prefix("* ") {
            if indent_len > 0 {
                spans.push(Span::raw(line[..indent_len].to_string()));
            }
            spans.push(Span::styled("• ", Style::default().fg(CHAT_ACCENT)));
            remaining = stripped;
        } else if let Some(pos) = trimmed.find(". ") {
            if pos > 0 && trimmed[..pos].chars().all(|c| c.is_ascii_digit()) {
                if indent_len > 0 {
                    spans.push(Span::raw(line[..indent_len].to_string()));
                }
                spans.push(Span::styled(trimmed[..=pos + 1].to_string(), Style::default().fg(CHAT_ACCENT)));
                remaining = &trimmed[pos + 2..];
            }
        }

        spans.extend(Self::parse_chat_markdown_spans_owned(remaining));
        Line::from(spans)
    }

    fn start_chat_turn(&mut self, query: String) -> bool {
        let query = query.trim().to_string();
        if query.is_empty() {
            return false;
        }

        if self.chat_stream_rx.is_some() {
            self.last_action = String::from("OpenRouter is still answering the previous message.");
            return false;
        }

        let Some(api_key) = self.openrouter_api_key.clone() else {
            self.push_chat_message("user", query.clone());
            self.push_chat_message("assistant", String::from("OpenRouter is not connected. Run /login <api-key> first."));
            self.thinking = false;
            self.thinking_ticks_remaining = 0;
            self.streaming_active = false;
            self.last_action = String::from("OpenRouter login required.");
            return true;
        };

        self.push_chat_message("user", query.clone());

        let conversation = self.openrouter_conversation();

        self.push_chat_message("assistant", String::new());

        self.panel_mode = PanelMode::AiChat;
        self.thinking = true;
        self.thinking_ticks_remaining = 20;
        self.chat_scroll_offset = 0;
        self.streaming_buffer.clear();
        self.streaming_active = true;
        self.last_action = format!("AI Chat: {}", query);

        let (sender, receiver) = mpsc::channel();
        self.chat_stream_rx = Some(receiver);

        thread::spawn(move || {
            if let Err(error) = Self::send_openrouter_chat_streaming(&api_key, &conversation, sender.clone()) {
                let _ = sender.send(ChatStreamUpdate::Error(error));
            }
        });

        true
    }

    fn openrouter_conversation(&self) -> Vec<(String, String)> {
        let mut conversation = Vec::new();
        conversation.push((
            String::from("system"),
            String::from("You are Aleph, a concise terminal assistant. Keep answers practical and grounded. If the user asks for detail, expand, but default to short, useful responses."),
        ));

        let mut recent_messages: Vec<_> = self.chat_messages.iter().rev().take(12).cloned().collect();
        recent_messages.reverse();

        for message in recent_messages {
            if message.content.trim().is_empty() {
                continue;
            }
            conversation.push((message.role, message.content));
        }

        conversation
    }

    fn rebuild_chat_render_cache(&mut self) {
        let mut lines: Vec<Line<'static>> = Vec::new();

        if self.chat_messages.is_empty() {
            lines.push(Line::from(""));
            lines.push(Line::from(vec![Span::styled(
                if self.is_openrouter_connected() {
                    "Welcome to Aleph AI chat. Type a message below to start."
                } else {
                    "Welcome to Aleph AI chat. Run /login openrouter to sign in in your browser."
                },
                Style::default().fg(CHAT_MUTED),
            )]));
            self.chat_render_cache = lines;
            self.chat_cache_stable_len = self.chat_render_cache.len();
            return;
        }

        let msg_count = self.chat_messages.len();
        for (index, message) in self.chat_messages.iter().enumerate() {
            if index > 0 {
                lines.push(Line::from(""));
            }

            let is_user = message.role == "user";
            let prefix = if is_user { "You" } else { "Aleph" };
            let color = if is_user { CHAT_ACCENT_SOFT } else { CHAT_ACCENT };

            lines.push(Line::from(vec![
                Span::styled(
                    format!("{} ", prefix),
                    Style::default().fg(color).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("({})", message.timestamp),
                    Style::default().fg(CHAT_MUTED),
                ),
            ]));

            // Mark stable length right after the last message's header
            if index == msg_count - 1 {
                self.chat_cache_stable_len = lines.len();
            }

            if message.content.trim().is_empty() {
                continue;
            }

            for content_line in message.content.lines() {
                if content_line.is_empty() {
                    lines.push(Line::from(""));
                } else {
                    lines.push(Self::render_chat_markdown_line_owned(content_line));
                }
            }
        }

        self.chat_render_cache = lines;
    }

    /// Fast incremental rebuild: only re-render the last message's content.
    /// Used during streaming so we don't re-parse every previous message's
    /// markdown on each token from the model.
    fn rebuild_chat_render_cache_streaming(&mut self) {
        self.chat_render_cache.truncate(self.chat_cache_stable_len);

        if let Some(last_msg) = self.chat_messages.last() {
            if !last_msg.content.trim().is_empty() {
                for content_line in last_msg.content.lines() {
                    if content_line.is_empty() {
                        self.chat_render_cache.push(Line::from(""));
                    } else {
                        self.chat_render_cache.push(Self::render_chat_markdown_line_owned(content_line));
                    }
                }
            }
        }
    }

    fn send_openrouter_chat_streaming(
        api_key: &str,
        conversation: &[(String, String)],
        sender: Sender<ChatStreamUpdate>,
    ) -> Result<(), String> {
        let messages: Vec<_> = conversation
            .iter()
            .map(|(role, content)| serde_json::json!({
                "role": role,
                "content": content,
            }))
            .collect();

        let payload = serde_json::json!({
            "model": OPENROUTER_CHAT_MODEL,
            "messages": messages,
            "temperature": 0.7,
            "stream": true,
        });

        let client = Client::builder()
            .timeout(Duration::from_secs(1800))
            .build()
            .map_err(|error| format!("failed to build HTTP client: {}", error))?;

        let response = client
            .post("https://openrouter.ai/api/v1/chat/completions")
            .bearer_auth(api_key)
            .header("Accept", "text/event-stream")
            .json(&payload)
            .send()
            .map_err(|error| format!("request failed: {}", error))?;

        let status = response.status();

        if !status.is_success() {
            let body = response
                .text()
                .map_err(|error| format!("failed to read response: {}", error))?;
            return Err(format!("{}: {}", status, body));
        }

        let mut reader = BufReader::with_capacity(256, response);
        let mut line = String::new();
        let mut event_data = String::new();

        loop {
            line.clear();
            let bytes_read = reader
                .read_line(&mut line)
                .map_err(|error| format!("failed to read OpenRouter stream: {}", error))?;

            if bytes_read == 0 {
                break;
            }

            let trimmed = line.trim_end_matches(['\r', '\n']);
            if trimmed.is_empty() {
                if !event_data.is_empty() && Self::handle_openrouter_stream_event(&event_data, &sender)? {
                    return Ok(());
                }
                event_data.clear();
                continue;
            }

            if trimmed.starts_with(':') {
                continue;
            }

            if let Some(payload) = trimmed.strip_prefix("data:") {
                if !event_data.is_empty() {
                    event_data.push('\n');
                }
                event_data.push_str(payload.strip_prefix(' ').unwrap_or(payload));
            }
        }

        if !event_data.is_empty() {
            let _ = Self::handle_openrouter_stream_event(&event_data, &sender)?;
        }

        let _ = sender.send(ChatStreamUpdate::Done);
        Ok(())
    }

    fn handle_openrouter_stream_event(
        event_data: &str,
        sender: &Sender<ChatStreamUpdate>,
    ) -> Result<bool, String> {
        let trimmed = event_data.trim();
        if trimmed.is_empty() {
            return Ok(false);
        }

        if trimmed == "[DONE]" {
            let _ = sender.send(ChatStreamUpdate::Done);
            return Ok(true);
        }

        let value: serde_json::Value = serde_json::from_str(trimmed)
            .map_err(|error| format!("failed to parse OpenRouter stream chunk: {}", error))?;

        if let Some(error) = value
            .get("error")
            .and_then(|error| error.get("message"))
            .and_then(|message| message.as_str())
        {
            let _ = sender.send(ChatStreamUpdate::Error(error.to_string()));
            return Ok(true);
        }

        if let Some(choice) = value.get("choices").and_then(|choices| choices.get(0)) {
            if let Some(content) = choice
                .get("delta")
                .and_then(|delta| delta.get("content"))
                .and_then(|content| content.as_str())
            {
                if !content.is_empty() {
                    let _ = sender.send(ChatStreamUpdate::Delta(content.to_string()));
                }
            }

            if let Some(finish_reason) = choice.get("finish_reason").and_then(|reason| reason.as_str()) {
                if finish_reason == "error" {
                    let message = value
                        .get("error")
                        .and_then(|error| error.get("message"))
                        .and_then(|message| message.as_str())
                        .unwrap_or("OpenRouter reported a streaming error");
                    let _ = sender.send(ChatStreamUpdate::Error(message.to_string()));
                } else {
                    let _ = sender.send(ChatStreamUpdate::Done);
                }
                return Ok(true);
            }
        }

        Ok(false)
    }

    pub fn is_openrouter_connected(&self) -> bool {
        self.connected && self.openrouter_api_key.is_some()
    }


    pub fn is_strix_connected(&self) -> bool {
        self.connected && self.strix_access_token.is_some()
    }

    fn load_strix_access_token() -> Option<String> {
        if let Ok(entry) = Self::strix_token_entry() {
            if let Ok(password) = entry.get_password() {
                let trimmed = password.trim().to_string();
                if !trimmed.is_empty() {
                    return Some(trimmed);
                }
            }
        }

        std::env::var("STRIX_ACCESS_TOKEN")
            .ok()
            .map(|token| token.trim().to_string())
            .filter(|token| !token.is_empty())
    }

    fn store_strix_access_token(&self, access_token: &str) -> Result<(), String> {
        let entry = Self::strix_token_entry()?;
        entry
            .set_password(access_token.trim())
            .map_err(|error| format!("failed to save Strix login: {}", error))
    }

    fn clear_strix_access_token(&self) {
        if let Ok(entry) = Self::strix_token_entry() {
            let _ = entry.delete_credential();
        }
    }

    fn strix_token_entry() -> Result<Entry, String> {
        Entry::new(STRIX_SERVICE, STRIX_ACCOUNT)
            .map_err(|error| format!("failed to open Strix credential store: {}", error))
    }

    fn strix_auth_base_url() -> String {
        std::env::var("STRIX_AUTH_BASE_URL")
            .ok()
            .map(|url| url.trim().trim_end_matches('/').to_string())
            .filter(|url| !url.is_empty())
            .unwrap_or_else(|| String::from(STRIX_AUTH_BASE_URL))
    }

    fn strix_api_base_url() -> String {
        std::env::var("STRIX_API_BASE_URL")
            .ok()
            .map(|url| url.trim().trim_end_matches('/').to_string())
            .filter(|url| !url.is_empty())
            .unwrap_or_else(Self::strix_auth_base_url)
    }

    fn strix_access_token(&self) -> Result<&str, String> {
        self.strix_access_token
            .as_deref()
            .filter(|token| !token.trim().is_empty())
            .ok_or_else(|| String::from("Strix is not connected. Run /login strix first."))
    }

    fn strix_json_request(
        &self,
        method: &str,
        path: &str,
        payload: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, String> {
        let token = self.strix_access_token()?;
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|error| format!("failed to build HTTP client: {}", error))?;
        let url = format!("{}{}", Self::strix_api_base_url(), path);
        let mut request = match method {
            "GET" => client.get(url),
            "POST" => client.post(url),
            "PATCH" => client.patch(url),
            _ => return Err(format!("unsupported Strix HTTP method: {}", method)),
        }
        .bearer_auth(token);

        if let Some(payload) = payload {
            request = request.json(&payload);
        }

        let response = request
            .send()
            .map_err(|error| format!("Strix request failed: {}", error))?;
        let status = response.status();
        let body = response
            .text()
            .map_err(|error| format!("failed to read Strix response: {}", error))?;
        if !status.is_success() {
            return Err(format!("Strix returned {}: {}", status, body));
        }
        serde_json::from_str(&body).map_err(|error| format!("failed to parse Strix response: {}", error))
    }

    fn sync_strix_notes(&mut self) -> Result<usize, String> {
        let notes = self.load_strix_notes("", STRIX_NOTES_LIMIT)?;
        let count = notes.len();
        self.notes = notes;
        self.selected_note = 0;
        Self::save_cached_strix_notes(&self.notes)?;
        self.add_strix_log(format!("Synced {} notes", count));
        Ok(count)
    }

    fn load_strix_notes(&self, query: &str, limit: usize) -> Result<Vec<Note>, String> {
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

    fn load_strix_note(&self, id_or_title: &str, hydrate_content: bool) -> Result<Note, String> {
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

    fn create_strix_note(&self, title: &str, content: &str) -> Result<Note, String> {
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

    fn update_strix_note(&self, note: &Note) -> Result<Note, String> {
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

    fn push_note_to_strix(&mut self, index: usize) -> Result<(), String> {
        if !self.is_strix_connected() {
            return Ok(());
        }
        let Some(note) = self.notes.get(index).cloned() else {
            return Ok(());
        };
        let mut synced = if note.remote_id.is_some() {
            self.update_strix_note(&note)?
        } else {
            self.create_strix_note(&note.title, &note.content)?
        };
        synced.id = note.id;
        synced.folder_id = note.folder_id;
        if let Some(slot) = self.notes.get_mut(index) {
            *slot = synced;
        }
        Self::save_cached_strix_notes(&self.notes)?;
        self.add_strix_log("Pushed note changes to Strix");
        Ok(())
    }

    fn upsert_synced_note(&mut self, mut note: Note) {
        if let Some(remote_id) = note.remote_id.clone() {
            if let Some((index, existing)) = self
                .notes
                .iter_mut()
                .enumerate()
                .find(|(_, existing)| existing.remote_id.as_deref() == Some(remote_id.as_str()))
            {
                note.id = existing.id;
                *existing = note;
                self.selected_note = index;
                return;
            }
        }

        note.id = self.notes.len() + 1;
        self.notes.push(note);
        self.selected_note = self.notes.len() - 1;
        let _ = Self::save_cached_strix_notes(&self.notes);
    }

    fn note_from_strix_value(local_id: usize, value: &serde_json::Value) -> Note {
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

    fn ensure_cached_strix_notes_loaded(&mut self) {
        let has_remote_notes = self.notes.iter().any(|note| note.remote_id.is_some());
        if has_remote_notes {
            return;
        }

        if let Ok(notes) = Self::load_cached_strix_notes() {
            if !notes.is_empty() {
                self.notes = notes;
                self.selected_note = 0;
                self.add_strix_log("Loaded cached Strix notes");
            }
        }
    }

    fn strix_cache_path() -> PathBuf {
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
            return PathBuf::from(home).join(".cache").join("aleph").join("strix-notes.json");
        }

        std::env::temp_dir().join("aleph-strix-notes.json")
    }

    fn load_cached_strix_notes() -> Result<Vec<Note>, String> {
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
            .map(|(index, value)| Self::note_from_strix_value(index + 1, value))
            .collect())
    }

    fn save_cached_strix_notes(notes: &[Note]) -> Result<(), String> {
        let path = Self::strix_cache_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("failed to create Strix note cache directory: {}", error))?;
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

    fn now_millis() -> u128 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_millis())
            .unwrap_or(0)
    }

    fn html_to_terminal_text(input: &str) -> String {
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

    fn push_block_prefix(output: &mut String, prefix: &str) {
        if !output.trim_end().is_empty() {
            Self::push_collapsed_newline(output);
        }
        output.push_str(prefix);
    }

    fn push_collapsed_newline(output: &mut String) {
        if output.ends_with("\n\n") {
            return;
        }
        if output.ends_with('\n') {
            output.push('\n');
        } else {
            output.push_str("\n\n");
        }
    }

    fn decode_html_entities(input: &str) -> String {
        input
            .replace("&nbsp;", " ")
            .replace("&amp;", "&")
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&quot;", "\"")
            .replace("&#39;", "'")
    }

    fn text_to_strix_html(input: &str) -> String {
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

            if let Some(task) = trimmed.strip_prefix("- [ ] ").or_else(|| trimmed.strip_prefix("- [x] ")) {
                if !task_list_open {
                    html.push_str("<ul data-type=\"taskList\">");
                    task_list_open = true;
                }
                let checked = if trimmed.starts_with("- [x] ") { "true" } else { "false" };
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
                html.push_str(&format!("<ul><li><p>{}</p></li></ul>", Self::escape_html(text)));
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

    fn escape_html(input: &str) -> String {
        input
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
            .replace('\'', "&#39;")
    }

    fn load_openrouter_api_key() -> Option<String> {
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

    fn store_openrouter_api_key(&self, api_key: &str) -> Result<(), String> {
        let entry = Self::openrouter_key_entry()?;
        entry
            .set_password(api_key.trim())
            .map_err(|error| format!("failed to save OpenRouter login: {}", error))
    }

    fn clear_openrouter_api_key(&self) {
        if let Ok(entry) = Self::openrouter_key_entry() {
            let _ = entry.delete_credential();
        }
    }

    fn openrouter_key_entry() -> Result<Entry, String> {
        Entry::new(OPENROUTER_SERVICE, OPENROUTER_ACCOUNT)
            .map_err(|error| format!("failed to open OpenRouter credential store: {}", error))
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
        if let Err(error) = self.push_note_to_strix(index) {
            self.last_action = format!("Strix push failed: {}", error);
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
            format!("Strix ID: {}", note.remote_id.as_deref().unwrap_or("local-only")),
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
            let remote_matches = note
                .remote_id
                .as_deref()
                .map(|remote_id| remote_id.eq_ignore_ascii_case(trimmed))
                .unwrap_or(false);
            if remote_matches || note.title.eq_ignore_ascii_case(trimmed) || title.contains(&lower) {
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
            .map(|note| {
                let id_label = note
                    .remote_id
                    .as_deref()
                    .map(|remote_id| format!("#{} {}", note.id, remote_id))
                    .unwrap_or_else(|| format!("#{}", note.id));
                format!("{} {} — {}", id_label, note.title, Self::preview_text(&note.content, 56))
            })
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
                    self.ghost_submit_instruction();
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
        self.ghost_result = None;
        self.last_action = String::from("Summoned the Ghost.");
    }

    fn close_ai_overlay(&mut self) {
        self.ai_overlay_visible = false;
        self.ai_overlay_pulse_ticks = 0;
        self.thinking = false;
        self.thinking_ticks_remaining = 0;
        self.ghost_result = None;
        self.ghost_streaming = false;
        self.ghost_stream_rx = None;
    }

    fn toggle_ai_overlay(&mut self) {
        if self.ai_overlay_visible {
            self.close_ai_overlay();
        } else {
            self.open_ai_overlay();
        }
    }

    fn open_login_picker(&mut self) {
        self.panel_mode = PanelMode::LoginPicker;
        self.panel_title = String::from("Sign in");
        self.panel_lines = vec![String::from("picker")];
        self.login_picker_selected = 0;
        self.last_action = String::from("Choose an AI provider.");
    }

    fn handle_login_picker_key(&mut self, key_event: KeyEvent) {
        if key_event.kind != KeyEventKind::Press {
            return;
        }
        match key_event.code {
            KeyCode::Esc => {
                self.panel_mode = PanelMode::Commands;
                self.panel_title = String::from("Commands");
                self.panel_lines.clear();
                self.last_action = String::from("Cancelled login.");
            }
            KeyCode::Up => {
                if self.login_picker_selected > 0 {
                    self.login_picker_selected -= 1;
                }
            }
            KeyCode::Down => {
                if self.login_picker_selected < 1 {
                    self.login_picker_selected += 1;
                }
            }
            KeyCode::Enter => {
                match self.login_picker_selected {
                    0 => {
                        // OpenRouter: start browser login
                        self.panel_mode = PanelMode::Commands;
                        if !self.start_openrouter_browser_login() {
                            self.set_result_panel(
                                "OpenRouter login failed",
                                vec![String::from("Unable to start the browser-based OpenRouter login flow.")],
                            );
                            self.last_action = String::from("OpenRouter login failed.");
                        }
                    }
                    1 => {
                        self.panel_mode = PanelMode::Commands;
                        if !self.start_strix_browser_login() {
                            self.set_result_panel(
                                "Strix login failed",
                                vec![String::from("Unable to start the browser-based Strix login flow.")],
                            );
                            self.last_action = String::from("Strix login failed.");
                        }
                    }
                    _ => {}
                }
            }
            KeyCode::Char('c') if key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                self.request_quit();
            }
            _ => {}
        }
    }

    fn ghost_submit_instruction(&mut self) {
        let instruction = self.ai_input_buffer.trim().to_string();
        if instruction.is_empty() {
            return;
        }

        let Some(api_key) = self.openrouter_api_key.clone() else {
            self.ghost_result = Some(String::from("Not connected. Run /login first."));
            return;
        };

        let editor_content = self.editor_buffer.clone();
        self.ghost_streaming = true;
        self.ghost_result = None;
        self.thinking = true;
        self.thinking_ticks_remaining = 20;

        // Build a conversation for the ghost editor
        let system_prompt = String::from(
            "You are a writing assistant embedded in a note editor. The user will give you the current note content and an instruction. \
             Respond ONLY with the complete updated note content — no explanations, no markdown code fences, no preamble. \
             If the user asks a question about the text, answer concisely in plain text. \
             If the user asks to edit, rewrite, or add to the text, return the full updated text."
        );

        let user_msg = format!(
            "Current note content:\n---\n{}\n---\n\nInstruction: {}",
            editor_content, instruction
        );

        let conversation = vec![
            (String::from("system"), system_prompt),
            (String::from("user"), user_msg),
        ];

        let (sender, receiver) = mpsc::channel();
        self.ghost_stream_rx = Some(receiver);

        thread::spawn(move || {
            if let Err(error) = Self::send_openrouter_chat_streaming(&api_key, &conversation, sender.clone()) {
                let _ = sender.send(ChatStreamUpdate::Error(error));
            }
        });

        self.ai_input_buffer.clear();
        self.ai_input_cursor = 0;
    }

    pub fn process_ghost_stream(&mut self) {
        let mut finished = false;
        while !finished {
            let result = match self.ghost_stream_rx.as_ref() {
                Some(receiver) => receiver.try_recv(),
                None => break,
            };

            match result {
                Ok(ChatStreamUpdate::Delta(chunk)) => {
                    if self.ghost_result.is_none() {
                        self.ghost_result = Some(String::new());
                    }
                    if let Some(ref mut buf) = self.ghost_result {
                        buf.push_str(&chunk);
                    }
                    self.thinking = true;
                }
                Ok(ChatStreamUpdate::Done) => {
                    self.ghost_streaming = false;
                    self.thinking = false;
                    self.thinking_ticks_remaining = 0;
                    self.ghost_stream_rx = None;

                    // Apply the result to the editor buffer
                    if let Some(ref result) = self.ghost_result {
                        let result = result.trim().to_string();
                        if !result.is_empty() {
                            self.save_undo_state();
                            self.editor_buffer = result;
                            self.editor_cursor = self.editor_buffer.len().min(self.editor_cursor);
                            self.last_action = String::from("Ghost applied edits.");
                        }
                    }
                    self.ghost_result = None;
                    finished = true;
                }
                Ok(ChatStreamUpdate::Error(error)) => {
                    self.ghost_result = Some(format!("Error: {}", error));
                    self.ghost_streaming = false;
                    self.thinking = false;
                    self.thinking_ticks_remaining = 0;
                    self.ghost_stream_rx = None;
                    self.last_action = String::from("Ghost request failed.");
                    finished = true;
                }
                Err(TryRecvError::Empty) => {
                    self.thinking = true;
                    break;
                }
                Err(TryRecvError::Disconnected) => {
                    self.ghost_result = Some(String::from("Ghost disconnected."));
                    self.ghost_streaming = false;
                    self.thinking = false;
                    self.thinking_ticks_remaining = 0;
                    self.ghost_stream_rx = None;
                    finished = true;
                }
            }
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
