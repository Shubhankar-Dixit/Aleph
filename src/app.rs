use std::collections::{HashMap, VecDeque};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::thread;
use std::time::{Duration, Instant};

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseEvent, MouseEventKind};
use keyring::Entry;
use rand::{rngs::OsRng, RngCore};
use ratatui::prelude::{Color, Line, Modifier, Span, Style};
use reqwest::blocking::Client;
use sha2::{Digest, Sha256};

mod commands;
pub mod model;

pub use commands::{COMMANDS, THINKING_FRAMES};
pub use model::*;

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
const OBSIDIAN_SERVICE: &str = "Aleph";
const OBSIDIAN_ACCOUNT: &str = "obsidian_vault_path";
const NOTE_SAVE_TARGET_CONFIG: &str = "note-save-target";
const AI_PROVIDER_CONFIG: &str = "ai-provider";
const AGENT_MODE_CONFIG: &str = "agent-mode";
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum AgentAction {
    Chat,
    CreateNote,
    EditNote,
}

struct AgentDecision {
    action: AgentAction,
    note_index: Option<usize>,
    title: Option<String>,
    rationale: String,
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
    editor_selection: Selection,
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
    obsidian_vault_path: Option<PathBuf>,
    obsidian_vaults: Vec<ObsidianVault>,
    obsidian_vault_selected: usize,
    note_save_target: NoteSaveTarget,
    ai_provider: AiProvider,
    strix_logs: Vec<String>,
    streaming_buffer: String,
    streaming_active: bool,
    chat_render_cache: Vec<Line<'static>>,
    chat_render_dirty: bool,
    chat_cache_stable_len: usize,
    agent_mode_enabled: bool,
    login_picker_selected: usize,
    settings_selected: usize,
    pending_agent_query: Option<String>,
    pending_agent_decision: Option<AgentDecision>,
    ghost_stream_rx: Option<Receiver<ChatStreamUpdate>>,
    ghost_streaming: bool,
    ghost_result: Option<String>,
    pending_ai_edit: Option<AiEditProposal>,
    ai_draft_create_title: Option<String>,
    note_list_selected: usize,
    note_list_indices: Vec<usize>,
    note_list_pending_delete: Option<usize>,
    editing_title: bool,
    title_buffer: String,
    title_cursor: usize,
}

#[allow(dead_code)]
impl App {
    pub fn new() -> Self {
        let openrouter_api_key = Self::load_openrouter_api_key();
        let strix_access_token = Self::load_strix_access_token();
        let obsidian_vault_path = Self::load_obsidian_vault_path();
        let obsidian_vaults = Self::discover_obsidian_vaults();
        let connected = openrouter_api_key.is_some() || strix_access_token.is_some();
        let default_ai_provider = if openrouter_api_key.is_some() {
            AiProvider::OpenRouter
        } else if strix_access_token.is_some() {
            AiProvider::Strix
        } else {
            AiProvider::OpenRouter
        };
        let ai_provider = Self::load_ai_provider().unwrap_or(default_ai_provider);
        let default_note_save_target = if strix_access_token.is_some() {
            NoteSaveTarget::Strix
        } else if obsidian_vault_path.is_some() {
            NoteSaveTarget::Obsidian
        } else {
            NoteSaveTarget::Local
        };
        let note_save_target = Self::load_note_save_target()
            .filter(|target| {
                Self::note_save_target_is_available(
                    *target,
                    obsidian_vault_path.is_some(),
                    strix_access_token.is_some(),
                )
            })
            .unwrap_or(default_note_save_target);

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
                    obsidian_path: None,
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
                    obsidian_path: None,
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
                    obsidian_path: None,
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
                    obsidian_path: None,
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
            editor_selection: Selection::default(),
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
            obsidian_vault_path,
            obsidian_vaults,
            obsidian_vault_selected: 0,
            note_save_target,
            ai_provider,
            strix_logs: Vec::new(),
            streaming_buffer: String::new(),
            streaming_active: false,
            chat_render_cache: Vec::new(),
            chat_render_dirty: false,
            chat_cache_stable_len: 0,
            agent_mode_enabled: Self::load_agent_mode_enabled().unwrap_or(true),
            login_picker_selected: 0,
            settings_selected: 0,
            pending_agent_query: None,
            pending_agent_decision: None,
            ghost_stream_rx: None,
            ghost_streaming: false,
            ghost_result: None,
            pending_ai_edit: None,
            ai_draft_create_title: None,
            note_list_selected: 0,
            note_list_indices: Vec::new(),
            note_list_pending_delete: None,
            editing_title: false,
            title_buffer: String::new(),
            title_cursor: 0,
        };

        if app.strix_access_token.is_some() {
            if let Ok(notes) = Self::load_cached_strix_notes() {
                if !notes.is_empty() {
                    app.notes = notes;
                    app.selected_note = 0;
                    app.add_strix_log("Loaded cached Strix notes");
                    app.last_action =
                        String::from("Loaded cached Strix notes. Run /sync to refresh.");
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

        if area == "obsidian" {
            self.refresh_obsidian_vaults();
            return self.run_obsidian_cli_command(&args[1..]);
        }

        if area != "notes" && area != "note" {
            return Err(format!(
                "Unknown Aleph CLI area '{}'. Try 'notes' or 'obsidian'.",
                area
            ));
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
                            note.obsidian_path
                                .as_ref()
                                .map(|path| format!("obsidian:{}", path.display()))
                                .or_else(|| note.remote_id.clone())
                                .unwrap_or_else(|| String::from("local-only")),
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
                    Self::note_source_label(&note),
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
                    return Err(String::from(
                        "Provide content or pass '-' to read content from stdin.",
                    ));
                }
                self.ensure_cached_strix_notes_loaded();
                let local_index = self.resolve_note_index(id);
                let mut note = local_index
                    .and_then(|index| self.notes.get(index).cloned())
                    .map(Ok)
                    .unwrap_or_else(|| self.load_strix_note(id, true))?;
                note.content = content;
                note.raw_content = note.content.clone();
                if let Some(index) = local_index {
                    if let Some(slot) = self.notes.get_mut(index) {
                        *slot = note.clone();
                    }
                    self.write_note_to_obsidian(index)?;
                }
                let updated = if note.remote_id.is_some() {
                    self.update_strix_note(&note)?
                } else {
                    note.clone()
                };
                if updated.remote_id.is_some() || local_index.is_none() {
                    self.upsert_synced_note(updated.clone());
                }
                Ok(vec![format!(
                    "Updated {} ({})",
                    updated.title,
                    Self::note_source_label(&updated)
                )])
            }
            "append" => {
                let id = args.get(2).ok_or_else(|| {
                    String::from("Usage: aleph notes append <id|title> <content>")
                })?;
                let content = args.get(3..).unwrap_or(&[]).join(" ");
                if content.is_empty() {
                    return Err(String::from(
                        "Provide content or pass '-' to read content from stdin.",
                    ));
                }
                self.ensure_cached_strix_notes_loaded();
                let local_index = self.resolve_note_index(id);
                let mut note = local_index
                    .and_then(|index| self.notes.get(index).cloned())
                    .map(Ok)
                    .unwrap_or_else(|| self.load_strix_note(id, true))?;
                if !note.content.is_empty() {
                    note.content.push('\n');
                }
                note.content.push_str(&content);
                note.raw_content = note.content.clone();
                if let Some(index) = local_index {
                    if let Some(slot) = self.notes.get_mut(index) {
                        *slot = note.clone();
                    }
                    self.write_note_to_obsidian(index)?;
                }
                let updated = if note.remote_id.is_some() {
                    self.update_strix_note(&note)?
                } else {
                    note.clone()
                };
                if updated.remote_id.is_some() || local_index.is_none() {
                    self.upsert_synced_note(updated.clone());
                }
                Ok(vec![format!(
                    "Appended to {} ({})",
                    updated.title,
                    Self::note_source_label(&updated)
                )])
            }
            "create" => {
                let title = args
                    .get(2)
                    .map(|title| title.as_str())
                    .filter(|title| !title.trim().is_empty())
                    .unwrap_or("Untitled note");
                let content = args.get(3..).unwrap_or(&[]).join(" ");
                let mut note = self.create_strix_note(title, &content)?;
                if let Some(path) = self.obsidian_note_path_for_title(title) {
                    if let Some(parent) = path.parent() {
                        fs::create_dir_all(parent).map_err(|error| {
                            format!("failed to create '{}': {}", parent.display(), error)
                        })?;
                    }
                    fs::write(&path, &content).map_err(|error| {
                        format!("failed to write '{}': {}", path.display(), error)
                    })?;
                    note.obsidian_path = Some(path);
                }
                self.upsert_synced_note(note.clone());
                Ok(vec![format!(
                    "Created {} ({})",
                    note.title,
                    Self::note_source_label(&note)
                )])
            }
            _ => Err(format!("Unknown notes action '{}'.", action)),
        }
    }

    fn run_obsidian_cli_command(&mut self, args: &[String]) -> Result<Vec<String>, String> {
        let action = args.first().map(|value| value.as_str()).unwrap_or("status");
        match action {
            "pair" => {
                self.refresh_obsidian_vaults();
                let target = args.get(1).map(|value| value.as_str()).unwrap_or("");
                let path = if target.is_empty() {
                    match self.obsidian_vaults.as_slice() {
                        [vault] => vault.path.clone(),
                        [] => {
                            return Err(String::from(
                                "No Obsidian vaults found. Run `aleph obsidian pair <path>`.",
                            ))
                        }
                        _ => {
                            return Ok(std::iter::once(String::from(
                                "Multiple vaults found. Re-run with a number or name:",
                            ))
                            .chain(self.format_obsidian_vault_lines())
                            .collect())
                        }
                    }
                } else {
                    self.resolve_obsidian_vault_target(target)
                        .unwrap_or_else(|| PathBuf::from(Self::expand_home(target)))
                };
                let message = self.pair_obsidian_vault(path)?;
                Ok(vec![
                    message,
                    String::from("Run `aleph obsidian sync` to import notes."),
                ])
            }
            "vaults" | "list" => {
                self.refresh_obsidian_vaults();
                let mut lines = self.format_obsidian_vault_lines();
                if lines.is_empty() {
                    lines.push(String::from(
                        "No Obsidian vaults found. Run `aleph obsidian pair <path>`.",
                    ));
                }
                Ok(lines)
            }
            "sync" => {
                let count = self.sync_obsidian_notes()?;
                Ok(vec![format!("Imported {} Obsidian notes.", count)])
            }
            "status" => Ok(vec![
                format!("Obsidian: {}", self.obsidian_status_label()),
                format!("Detected vaults: {}", self.obsidian_vaults.len()),
                format!("Config: {}", Self::obsidian_config_path().display()),
                format!(
                    "Pairing fallback: {}",
                    Self::obsidian_pairing_path().display()
                ),
            ]),
            "open" => {
                let target = args.get(1..).unwrap_or(&[]).join(" ");
                self.open_obsidian_target(&target)
                    .map(|message| vec![message])
            }
            _ => Err(format!(
                "Unknown obsidian action '{}'. Try pair, vaults, sync, status, or open.",
                action
            )),
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
                                "OpenRouter provider",
                                vec![
                                    String::from(
                                        "OpenRouter authorization completed successfully.",
                                    ),
                                    String::from(
                                        "The API key has been stored locally as a model provider.",
                                    ),
                                    String::from("AI chat can use OpenRouter now."),
                                ],
                            );
                            self.last_action =
                                String::from("Configured OpenRouter as a model provider.");
                        }
                        Err(error) => {
                            self.openrouter_api_key = None;
                            self.refresh_connection_state();
                            self.set_result_panel("OpenRouter provider failed", vec![error]);
                            self.last_action = String::from("OpenRouter provider setup failed.");
                        }
                    }

                    self.openrouter_login_rx = None;
                    self.openrouter_login_cancel = None;
                    login_finished = true;
                }
                Ok(Err(error)) => {
                    self.refresh_connection_state();
                    self.set_result_panel("OpenRouter provider failed", vec![error]);
                    self.last_action = String::from("OpenRouter provider setup failed.");
                    self.openrouter_login_rx = None;
                    self.openrouter_login_cancel = None;
                    login_finished = true;
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.refresh_connection_state();
                    self.set_result_panel(
                        "OpenRouter provider failed",
                        vec![String::from(
                            "The browser login flow disconnected before completion.",
                        )],
                    );
                    self.last_action = String::from("OpenRouter provider setup disconnected.");
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
                            self.note_save_target = NoteSaveTarget::Strix;
                            let _ = self.store_note_save_target();
                            self.refresh_connection_state();
                            self.add_strix_log("Browser login completed successfully");
                            self.set_result_panel(
                                "Strix login",
                                vec![
                                    String::from("Strix browser login completed successfully."),
                                    String::from(
                                        "The native app access token has been stored locally.",
                                    ),
                                    String::from(
                                        "Aleph can now call Strix-native APIs as they come online.",
                                    ),
                                ],
                            );
                            self.last_action =
                                String::from("Connected to Strix via browser login.");
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
                        vec![String::from(
                            "The browser login flow disconnected before completion.",
                        )],
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
                            message.content = String::from("Aleph returned no content.");
                        }
                    }

                    self.streaming_buffer.clear();
                    self.streaming_active = false;
                    self.rebuild_chat_render_cache();
                    self.chat_render_dirty = false;
                    self.thinking = false;
                    self.thinking_ticks_remaining = 0;
                    self.chat_stream_rx = None;
                    self.last_action = String::from("AI response received.");
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
                            message.content = format!("AI chat failed: {}", error);
                        } else {
                            message.content.push_str("\n\n");
                            message.content.push_str(&format!("[AI error: {}]", error));
                        }
                    } else {
                        self.push_chat_message("assistant", format!("AI chat failed: {}", error));
                    }

                    self.streaming_buffer.clear();
                    self.streaming_active = false;
                    self.rebuild_chat_render_cache();
                    self.chat_render_dirty = false;
                    self.thinking = false;
                    self.thinking_ticks_remaining = 0;
                    self.chat_stream_rx = None;
                    self.last_action = String::from("AI request failed.");
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
                        message.content =
                            String::from("AI chat disconnected before a response arrived.");
                    }

                    self.streaming_buffer.clear();
                    self.streaming_active = false;
                    self.rebuild_chat_render_cache();
                    self.chat_render_dirty = false;
                    self.thinking = false;
                    self.thinking_ticks_remaining = 0;
                    self.chat_stream_rx = None;
                    self.last_action = String::from("AI request disconnected.");
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

    pub fn editor_display_buffer(&self) -> &str {
        if let Some(proposal) = self.pending_ai_edit.as_ref() {
            return &proposal.proposed;
        }

        if self.ghost_streaming {
            if let Some(result) = self.ghost_result.as_deref() {
                if !result.trim().is_empty() {
                    return result;
                }
            }
        }

        &self.editor_buffer
    }

    pub fn editor_cursor(&self) -> usize {
        self.editor_cursor
    }

    pub fn editor_display_cursor(&self) -> usize {
        if self.has_live_ai_editor_preview() {
            self.editor_display_buffer().len()
        } else {
            self.editor_cursor
        }
    }

    pub fn has_live_ai_editor_preview(&self) -> bool {
        self.pending_ai_edit.is_some()
            || (self.ghost_streaming
                && self
                    .ghost_result
                    .as_deref()
                    .map(|result| !result.trim().is_empty())
                    .unwrap_or(false))
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

    pub fn editor_selection(&self) -> &Selection {
        &self.editor_selection
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

    pub fn ai_provider_label(&self) -> &'static str {
        self.model_provider_label()
    }

    pub fn model_provider_label(&self) -> &'static str {
        match self.ai_provider {
            AiProvider::OpenRouter => "OpenRouter",
            AiProvider::Strix => "Strix",
        }
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

    pub fn is_agent_mode_enabled(&self) -> bool {
        self.agent_mode_enabled
    }

    pub fn login_picker_selected(&self) -> usize {
        self.login_picker_selected
    }

    pub fn is_login_picker(&self) -> bool {
        self.panel_mode == PanelMode::LoginPicker
    }

    pub fn is_settings(&self) -> bool {
        self.panel_mode == PanelMode::Settings
    }

    pub fn settings_selected(&self) -> usize {
        self.settings_selected
    }

    pub fn is_note_list(&self) -> bool {
        self.panel_mode == PanelMode::NoteList
    }

    pub fn is_vault_picker(&self) -> bool {
        self.panel_mode == PanelMode::VaultPicker
    }

    pub fn note_list_selected(&self) -> usize {
        self.note_list_selected
    }

    pub fn note_list_indices(&self) -> &[usize] {
        &self.note_list_indices
    }

    pub fn note_list_delete_is_pending(&self) -> bool {
        self.note_list_indices
            .get(self.note_list_selected)
            .copied()
            .map(|index| self.note_list_pending_delete == Some(index))
            .unwrap_or(false)
    }

    pub fn obsidian_vaults(&self) -> &[ObsidianVault] {
        &self.obsidian_vaults
    }

    pub fn obsidian_vault_selected(&self) -> usize {
        self.obsidian_vault_selected
    }

    pub fn obsidian_vault_path(&self) -> Option<&Path> {
        self.obsidian_vault_path.as_deref()
    }

    pub fn is_obsidian_paired(&self) -> bool {
        self.obsidian_vault_path.is_some()
    }

    pub fn note_save_target_label(&self) -> &'static str {
        Self::note_save_target_name(self.note_save_target)
    }

    pub fn is_editing_title(&self) -> bool {
        self.editing_title
    }

    pub fn title_buffer(&self) -> &str {
        &self.title_buffer
    }

    pub fn title_cursor(&self) -> usize {
        Self::clamp_to_char_boundary(&self.title_buffer, self.title_cursor)
    }

    pub fn is_ghost_streaming(&self) -> bool {
        self.ghost_streaming
    }

    pub fn ghost_result(&self) -> Option<&str> {
        self.ghost_result.as_deref()
    }

    pub fn has_pending_ai_edit(&self) -> bool {
        self.pending_ai_edit.is_some()
    }

    pub fn pending_ai_diff_lines(&self) -> Vec<String> {
        let Some(proposal) = self.pending_ai_edit.as_ref() else {
            return Vec::new();
        };

        proposal.diff_lines.clone()
    }

    pub fn pending_ai_instruction(&self) -> Option<&str> {
        self.pending_ai_edit
            .as_ref()
            .map(|proposal| proposal.instruction.as_str())
    }

    pub fn pending_ai_proposal_label(&self) -> &'static str {
        match self
            .pending_ai_edit
            .as_ref()
            .and_then(|proposal| proposal.note_index)
        {
            Some(_) => "Proposed note edits",
            None => "Proposed new note",
        }
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

    pub fn visible_commands_window(
        &self,
        window_size: usize,
    ) -> (Vec<&'static CommandSpec>, usize) {
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
                    self.is_command_visible(cmd)
                        && (cmd.name.contains(&query)
                            || cmd.description.to_lowercase().contains(&query))
                })
                .collect()
        } else {
            COMMANDS
                .iter()
                .filter(|cmd| self.is_command_visible(cmd))
                .collect()
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
            let mut all: Vec<_> = COMMANDS
                .iter()
                .filter(|cmd| self.is_command_visible(cmd))
                .collect();
            all.truncate(limit);
            return all;
        }

        // Filter commands by query
        let mut matches: Vec<&'static CommandSpec> = COMMANDS
            .iter()
            .filter(|command| {
                self.is_command_visible(command)
                    && (command.name.contains(&query)
                        || command.description.to_lowercase().contains(&query))
            })
            .collect();

        matches.truncate(limit);
        matches
    }

    fn is_command_visible(&self, cmd: &CommandSpec) -> bool {
        match cmd.name {
            "config" => false, // Hidden alias for /settings
            "login" => !self.connected,
            "logout" => self.connected,
            _ => true,
        }
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
        if self.is_note_list() {
            self.handle_note_list_key(key_event);
            return;
        }
        if self.is_vault_picker() {
            self.handle_vault_picker_key(key_event);
            return;
        }
        if self.is_settings() {
            self.handle_settings_key(key_event);
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
            KeyCode::Left
                if matches!(key_event.kind, KeyEventKind::Press | KeyEventKind::Repeat) =>
            {
                self.move_left()
            }
            KeyCode::Right
                if matches!(key_event.kind, KeyEventKind::Press | KeyEventKind::Repeat) =>
            {
                self.move_right()
            }
            KeyCode::Home if key_event.kind == KeyEventKind::Press => self.cursor = 0,
            KeyCode::End if key_event.kind == KeyEventKind::Press => {
                self.cursor = self.prompt.len()
            }
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

        if self.is_full_editor()
            && self.ai_overlay_visible
            && matches!(mouse_event.kind, MouseEventKind::Down(_))
        {
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
            KeyCode::Char('a')
                if key_event.kind == KeyEventKind::Press
                    && key_event.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                self.select_all_editor();
            }
            KeyCode::Esc if key_event.kind == KeyEventKind::Press => self.exit_editor(),
            KeyCode::Enter if key_event.kind == KeyEventKind::Press => {
                self.save_undo_state();
                self.clear_editor_selection();
                self.insert_editor_character('\n');
            }
            KeyCode::Tab if key_event.kind == KeyEventKind::Press => {
                self.clear_editor_selection();
                self.insert_editor_text("    ");
            }
            KeyCode::Backspace if key_event.kind == KeyEventKind::Press => {
                self.clear_editor_selection();
                self.editor_backspace()
            }
            KeyCode::Delete if key_event.kind == KeyEventKind::Press => {
                self.clear_editor_selection();
                self.editor_delete()
            }
            KeyCode::Left
                if matches!(key_event.kind, KeyEventKind::Press | KeyEventKind::Repeat) =>
            {
                self.clear_editor_selection();
                self.editor_move_left()
            }
            KeyCode::Right
                if matches!(key_event.kind, KeyEventKind::Press | KeyEventKind::Repeat) =>
            {
                self.clear_editor_selection();
                self.editor_move_right()
            }
            KeyCode::Up
                if matches!(key_event.kind, KeyEventKind::Press | KeyEventKind::Repeat)
                    && !key_event.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                self.clear_editor_selection();
                self.editor_move_up()
            }
            KeyCode::Down
                if matches!(key_event.kind, KeyEventKind::Press | KeyEventKind::Repeat)
                    && !key_event.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                self.clear_editor_selection();
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
            KeyCode::Home if key_event.kind == KeyEventKind::Press => {
                self.clear_editor_selection();
                self.editor_cursor = 0
            }
            KeyCode::End if key_event.kind == KeyEventKind::Press => {
                self.clear_editor_selection();
                self.editor_cursor = self.editor_buffer.len()
            }
            KeyCode::Char('z')
                if key_event.kind == KeyEventKind::Press
                    && key_event.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                self.clear_editor_selection();
                self.undo();
            }
            KeyCode::Char('y')
                if key_event.kind == KeyEventKind::Press
                    && key_event.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                self.clear_editor_selection();
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
                if self.pending_agent_decision.is_some() {
                    self.cancel_pending_agent_action();
                    return;
                }
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
            KeyCode::Char('g')
                if key_event.kind == KeyEventKind::Press
                    && key_event.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                self.toggle_agent_mode();
            }
            KeyCode::Enter if key_event.kind == KeyEventKind::Press => {
                // Send chat message
                let msg = self.chat_input_buffer.trim().to_string();
                if self.pending_agent_decision.is_some() {
                    if msg.is_empty() || Self::is_affirmative_agent_permission(&msg) {
                        if self.confirm_pending_agent_action() {
                            self.chat_input_buffer.clear();
                            self.chat_input_cursor = 0;
                        }
                        return;
                    }

                    if Self::is_negative_agent_permission(&msg) {
                        self.cancel_pending_agent_action();
                        self.chat_input_buffer.clear();
                        self.chat_input_cursor = 0;
                        return;
                    }

                    self.cancel_pending_agent_action();
                }

                if !msg.is_empty() {
                    if (self.agent_mode_enabled && self.try_start_agent_action(&msg))
                        || self.start_chat_turn(msg)
                    {
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
                self.chat_input_buffer
                    .insert(self.chat_input_cursor, character);
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
            COMMANDS
                .iter()
                .filter(|cmd| self.is_command_visible(cmd))
                .collect()
        } else {
            COMMANDS
                .iter()
                .filter(|cmd| {
                    self.is_command_visible(cmd)
                        && (cmd.name.contains(&query)
                            || cmd.description.to_lowercase().contains(&query))
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
            if self.agent_mode_enabled && self.try_start_agent_action(&query) {
                self.history.push(raw);
                self.history_index = None;
                self.reset_prompt();
                return;
            }
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
                | "agent edit"
                | "note move"
                | "folder create"
                | "folder delete"
                | "folder notes"
                | "memory save"
                | "memory search"
        )
    }

    fn toggle_agent_mode(&mut self) {
        self.agent_mode_enabled = !self.agent_mode_enabled;
        let mode_message = if self.agent_mode_enabled {
            "Agent mode enabled. Aleph will route note-writing requests to tools."
        } else {
            "Chat mode enabled. Aleph will answer without taking note actions."
        };
        self.last_action = if let Err(error) = self.store_agent_mode_enabled() {
            format!("{} (save failed: {})", mode_message, error)
        } else {
            String::from(mode_message)
        };
    }

    fn cycle_note_save_target(&mut self) {
        let next = match self.note_save_target {
            NoteSaveTarget::Local => {
                if self.is_obsidian_paired() {
                    NoteSaveTarget::Obsidian
                } else if self.is_strix_connected() {
                    NoteSaveTarget::Strix
                } else {
                    NoteSaveTarget::Local
                }
            }
            NoteSaveTarget::Obsidian => {
                if self.is_strix_connected() {
                    NoteSaveTarget::Strix
                } else {
                    NoteSaveTarget::Local
                }
            }
            NoteSaveTarget::Strix => NoteSaveTarget::Local,
        };

        if next == self.note_save_target {
            self.last_action = String::from(
                "Only Local note saving is available until Obsidian is paired or Strix is connected.",
            );
            return;
        }

        self.note_save_target = next;
        if let Err(error) = self.store_note_save_target() {
            self.last_action = format!(
                "Note save target: {} (save failed: {})",
                self.note_save_target_label(),
                error
            );
            return;
        }
        self.last_action = format!("Note save target: {}", self.note_save_target_label());
    }

    fn try_start_agent_action(&mut self, query: &str) -> bool {
        let decision = self.plan_agent_action(query);
        match decision.action {
            AgentAction::CreateNote | AgentAction::EditNote => {
                self.stage_agent_action(query, decision);
                true
            }
            AgentAction::Chat => false,
        }
    }

    fn stage_agent_action(&mut self, query: &str, decision: AgentDecision) {
        self.panel_mode = PanelMode::AiChat;
        self.chat_scroll_offset = 0;
        self.push_chat_message("user", query.trim());

        if decision.action == AgentAction::EditNote && decision.note_index.is_none() {
            self.pending_agent_query = None;
            self.pending_agent_decision = None;
            self.push_chat_message(
                "assistant",
                "I think this is note work, but I need a target note. Name the note, select one with `/note list`, or ask me to create a new note.",
            );
            self.last_action = String::from("Agent needs a note target.");
            return;
        }

        let message = self.agent_permission_message(&decision);
        self.pending_agent_query = Some(query.trim().to_string());
        self.pending_agent_decision = Some(decision);
        self.push_chat_message("assistant", message);
        self.last_action = String::from("Agent action waiting for permission.");
    }

    fn agent_permission_message(&self, decision: &AgentDecision) -> String {
        match decision.action {
            AgentAction::CreateNote => {
                let title = decision.title.as_deref().unwrap_or("AI draft");
                format!(
                    "I can create a new note titled `{}` and draft it in the editor. Press Enter to allow, type `no` to cancel, or type a different instruction.",
                    title
                )
            }
            AgentAction::EditNote => {
                let note_title = decision
                    .note_index
                    .and_then(|index| self.notes.get(index))
                    .map(|note| note.title.as_str())
                    .unwrap_or("the selected note");
                format!(
                    "I can edit `{}` using the note-writing agent. Press Enter to allow, type `no` to cancel, or type a different instruction.",
                    note_title
                )
            }
            AgentAction::Chat => String::new(),
        }
    }

    fn confirm_pending_agent_action(&mut self) -> bool {
        let Some(decision) = self.pending_agent_decision.take() else {
            return false;
        };
        let query = self.pending_agent_query.take().unwrap_or_default();
        match decision.action {
            AgentAction::CreateNote => self.start_note_create_agent(&query, decision.title),
            AgentAction::EditNote => self.start_note_edit_agent(&query, decision),
            AgentAction::Chat => false,
        }
    }

    fn cancel_pending_agent_action(&mut self) {
        self.pending_agent_query = None;
        self.pending_agent_decision = None;
        self.push_chat_message("assistant", "Cancelled the pending note action.");
        self.last_action = String::from("Cancelled pending agent action.");
    }

    fn is_affirmative_agent_permission(input: &str) -> bool {
        matches!(
            input.trim().to_lowercase().as_str(),
            "y" | "yes" | "ok" | "okay" | "do it" | "allow" | "approve" | "confirm" | "go"
        )
    }

    fn is_negative_agent_permission(input: &str) -> bool {
        matches!(
            input.trim().to_lowercase().as_str(),
            "n" | "no" | "nope" | "cancel" | "stop" | "don't" | "dont" | "reject"
        )
    }

    fn plan_agent_action(&self, query: &str) -> AgentDecision {
        if let Some(decision) = self.plan_agent_action_with_provider(query) {
            return decision;
        }

        self.plan_agent_action_locally(query)
    }

    fn plan_agent_action_locally(&self, query: &str) -> AgentDecision {
        if Self::looks_like_how_to_question(&query.to_lowercase()) {
            return AgentDecision {
                action: AgentAction::Chat,
                note_index: None,
                title: None,
                rationale: String::from("question"),
            };
        }

        let target_note = self.resolve_agent_note_target(query);
        if Self::looks_like_note_edit_request(query) || self.should_work_on_existing_note(query) {
            let rationale = if target_note.is_some() {
                "edit-target"
            } else {
                "edit-missing-target"
            };
            return AgentDecision {
                action: AgentAction::EditNote,
                note_index: target_note,
                title: None,
                rationale: String::from(rationale),
            };
        }
        if Self::looks_like_note_create_request(query) {
            return AgentDecision {
                action: AgentAction::CreateNote,
                note_index: None,
                title: Self::infer_note_title_from_request(query),
                rationale: String::from("create"),
            };
        }
        AgentDecision {
            action: AgentAction::Chat,
            note_index: None,
            title: None,
            rationale: String::from("chat"),
        }
    }

    fn plan_agent_action_with_provider(&self, query: &str) -> Option<AgentDecision> {
        let messages = self.agent_planner_conversation(query);
        let result = match self.ai_provider {
            AiProvider::OpenRouter => {
                let api_key = self.openrouter_api_key.as_deref()?;
                Self::send_openrouter_chat_blocking(api_key, &messages)
            }
            AiProvider::Strix => {
                let access_token = self.strix_access_token.as_deref()?;
                Self::send_strix_planner_request(
                    &Self::strix_api_base_url(),
                    access_token,
                    &messages,
                    &self.notes,
                )
            }
        };

        result
            .ok()
            .and_then(|content| self.parse_agent_planner_response(&content, query))
    }

    fn agent_planner_conversation(&self, query: &str) -> Vec<(String, String)> {
        let system = String::from(
            "You are Aleph's agent planner. Decide what Aleph should do with the user's chat input. \
             You may choose exactly one action: chat, create_note, edit_note. \
             Use edit_note when the user asks to work on, continue, improve, rewrite, append to, organize, or otherwise change an existing/current note. \
             Use create_note when the user wants new durable writing and no existing note is the right target. \
             Use chat for questions, explanations, brainstorming without durable write intent, or when you need to ask a clarification. \
             Return ONLY compact JSON with this schema: {\"action\":\"chat|create_note|edit_note\",\"note_id\":number|null,\"title\":string|null,\"rationale\":string}. \
             Do not write prose outside JSON.",
        );

        let mut notes = Vec::new();
        for (index, note) in self.notes.iter().enumerate().take(40) {
            notes.push(format!(
                "- id={}{} title=\"{}\" preview=\"{}\"",
                note.id,
                if index == self.selected_note {
                    " selected=true"
                } else {
                    ""
                },
                note.title.replace('"', "'"),
                Self::preview_text(&note.content, 120).replace('"', "'")
            ));
        }

        let selected = self
            .notes
            .get(self.selected_note)
            .map(|note| format!("id={} title=\"{}\"", note.id, note.title.replace('"', "'")))
            .unwrap_or_else(|| String::from("none"));
        let user = format!(
            "Selected note: {}\n\nAvailable notes:\n{}\n\nUser input:\n{}",
            selected,
            notes.join("\n"),
            query
        );

        vec![
            (String::from("system"), system),
            (String::from("user"), user),
        ]
    }

    fn parse_agent_planner_response(&self, content: &str, query: &str) -> Option<AgentDecision> {
        let json = Self::extract_json_object(content)?;
        let value: serde_json::Value = serde_json::from_str(json).ok()?;
        let action = match value.get("action")?.as_str()?.trim() {
            "chat" => AgentAction::Chat,
            "create_note" => AgentAction::CreateNote,
            "edit_note" => AgentAction::EditNote,
            _ => return None,
        };
        let note_index = value
            .get("note_id")
            .and_then(|id| {
                id.as_u64()
                    .and_then(|id| self.note_index_by_id(id as usize))
                    .or_else(|| {
                        id.as_str()
                            .and_then(|target| self.resolve_note_index(target))
                    })
            })
            .or_else(|| {
                value
                    .get("title")
                    .and_then(|title| title.as_str())
                    .and_then(|title| self.resolve_note_index(title))
            })
            .or_else(|| {
                if action == AgentAction::EditNote {
                    self.resolve_agent_note_target(query)
                } else {
                    None
                }
            });
        let title = value
            .get("title")
            .and_then(|title| title.as_str())
            .map(str::trim)
            .filter(|title| !title.is_empty())
            .map(|title| title.chars().take(80).collect::<String>());
        let rationale = value
            .get("rationale")
            .and_then(|rationale| rationale.as_str())
            .map(str::trim)
            .filter(|rationale| !rationale.is_empty())
            .unwrap_or("model-plan")
            .chars()
            .take(120)
            .collect::<String>();

        Some(AgentDecision {
            action,
            note_index,
            title,
            rationale,
        })
    }

    fn extract_json_object(content: &str) -> Option<&str> {
        let trimmed = content.trim();
        if trimmed.starts_with('{') && trimmed.ends_with('}') {
            return Some(trimmed);
        }

        let start = trimmed.find('{')?;
        let end = trimmed.rfind('}')?;
        (start < end).then_some(&trimmed[start..=end])
    }

    fn start_note_create_agent(&mut self, query: &str, title: Option<String>) -> bool {
        let title = title.unwrap_or_else(|| String::from("AI draft"));
        self.panel_mode = PanelMode::FullEditor;
        self.panel_title = format!("Drafting: {}", title);
        self.panel_lines.clear();
        self.editor_note_index = None;
        self.editor_buffer.clear();
        self.editor_cursor = 0;
        self.editor_scroll_offset = 0;
        self.open_ai_overlay();
        self.ai_draft_create_title = Some(title.clone());
        self.ai_input_buffer = query.trim().to_string();
        self.ai_input_cursor = self.ai_input_buffer.len();
        self.ghost_submit_instruction();
        self.last_action = format!("AI is drafting a new note: {}", title);
        true
    }

    fn looks_like_note_create_request(query: &str) -> bool {
        let lower = query.to_lowercase();
        if Self::looks_like_how_to_question(&lower) {
            return false;
        }

        let mentions_note = lower.contains("note")
            || lower.contains("notes")
            || lower.contains("draft")
            || lower.contains("write-up")
            || lower.contains("writeup");
        let direct_note_create = [
            "write a note",
            "write me a note",
            "write notes",
            "create a note",
            "create note",
            "make a note",
            "make note",
            "draft a note",
            "draft note",
            "compose a note",
            "compose note",
            "new note",
            "add a note",
            "take a note",
            "write-up",
            "writeup",
            "turn this into a note",
            "save this as a note",
            "write this down",
            "capture this",
        ]
        .iter()
        .any(|needle| lower.contains(needle));
        if direct_note_create {
            return true;
        }

        let starts_like_write_task = [
            "write ",
            "draft ",
            "compose ",
            "outline ",
            "prepare ",
            "make ",
            "create ",
            "generate ",
            "can you write ",
            "please write ",
            "can you draft ",
            "please draft ",
        ]
        .iter()
        .any(|prefix| lower.trim_start().starts_with(prefix));

        let content_shape = [
            " about ",
            " on ",
            " for ",
            " explaining ",
            " covering ",
            " that ",
        ]
        .iter()
        .any(|needle| lower.contains(needle));

        starts_like_write_task && (mentions_note || content_shape)
    }

    fn infer_note_title_from_request(query: &str) -> Option<String> {
        let trimmed = query.trim();
        for marker in [" titled ", " called ", " named "] {
            if let Some((_, rest)) = trimmed.to_lowercase().split_once(marker) {
                let start = trimmed.len().saturating_sub(rest.len());
                let title = trimmed[start..]
                    .split(['.', ',', ';'])
                    .next()
                    .unwrap_or_default()
                    .trim_matches(|c: char| c == '"' || c == '\'' || c.is_whitespace());
                if !title.is_empty() {
                    return Some(title.chars().take(80).collect());
                }
            }
        }

        for marker in [" about ", " on "] {
            if let Some((_, rest)) = trimmed.to_lowercase().split_once(marker) {
                let start = trimmed.len().saturating_sub(rest.len());
                let topic = trimmed[start..]
                    .split(['.', ';'])
                    .next()
                    .unwrap_or_default()
                    .trim_matches(|c: char| c == '"' || c == '\'' || c.is_whitespace());
                if !topic.is_empty() {
                    return Some(Self::title_case_note_topic(topic));
                }
            }
        }

        None
    }

    fn title_case_note_topic(topic: &str) -> String {
        let words = topic
            .split_whitespace()
            .take(8)
            .map(|word| {
                let mut chars = word.chars();
                match chars.next() {
                    Some(first) => {
                        let mut titled = first.to_uppercase().collect::<String>();
                        titled.push_str(chars.as_str());
                        titled
                    }
                    None => String::new(),
                }
            })
            .filter(|word| !word.is_empty())
            .collect::<Vec<_>>();

        if words.is_empty() {
            String::from("AI draft")
        } else {
            words.join(" ")
        }
    }

    fn start_note_edit_agent(&mut self, query: &str, decision: AgentDecision) -> bool {
        let Some(index) = decision.note_index else {
            self.set_result_panel(
                "AI note edit",
                vec![
                    String::from("I decided this is note work, but I need a target note."),
                    String::from(
                        "Name a note, select one with /note list, or ask me to create a new note.",
                    ),
                ],
            );
            self.last_action = String::from("AI note edit needs a note target.");
            return true;
        };

        self.open_note_editor(index);
        self.open_ai_overlay();
        self.ai_input_buffer = query.trim().to_string();
        self.ai_input_cursor = self.ai_input_buffer.len();
        self.ghost_submit_instruction();
        self.last_action = format!(
            "AI is preparing edits for note: {} ({})",
            self.notes[index].title, decision.rationale
        );
        true
    }

    fn should_work_on_existing_note(&self, query: &str) -> bool {
        let lower = query.to_lowercase();
        if Self::looks_like_how_to_question(&lower) {
            return false;
        }

        let asks_for_work = [
            "work on",
            "keep working on",
            "continue",
            "finish",
            "develop",
            "refine",
            "iterate on",
            "take another pass",
            "make progress",
        ]
        .iter()
        .any(|needle| lower.contains(needle));
        let references_existing_context = [
            "existing note",
            "current note",
            "selected note",
            "this note",
            "that note",
            "the note",
            "existing draft",
            "current draft",
            "this draft",
            "it",
            "this",
        ]
        .iter()
        .any(|needle| lower.contains(needle));

        asks_for_work && references_existing_context
    }

    fn looks_like_note_edit_request(query: &str) -> bool {
        let lower = query.to_lowercase();
        if Self::looks_like_how_to_question(&lower) {
            return false;
        }

        let mentions_note = lower.contains("note")
            || lower.contains("notes")
            || lower.contains("this doc")
            || lower.contains("current doc")
            || lower.contains("current note")
            || lower.contains("selected note")
            || lower.contains("this note")
            || lower.contains("draft");
        let references_current_text = [
            "this",
            "current",
            "selected",
            "existing",
            "the note",
            "my note",
            "the draft",
            "my draft",
        ]
        .iter()
        .any(|needle| lower.contains(needle));
        let wants_edit = [
            "edit",
            "rewrite",
            "update",
            "change",
            "append",
            "add",
            "insert",
            "write",
            "draft",
            "improve",
            "fix",
            "clean up",
            "summarize",
            "turn this into",
            "make this",
            "make it",
            "expand",
            "shorten",
            "polish",
            "refactor",
            "convert",
        ]
        .iter()
        .any(|needle| lower.contains(needle));

        (mentions_note || references_current_text) && wants_edit
    }

    fn looks_like_how_to_question(lower: &str) -> bool {
        let trimmed = lower.trim_start();
        [
            "how do i ",
            "how can i ",
            "how should i ",
            "what is ",
            "what are ",
            "why does ",
            "why is ",
            "can you explain ",
            "explain how ",
        ]
        .iter()
        .any(|prefix| trimmed.starts_with(prefix))
    }

    fn resolve_agent_note_target(&self, query: &str) -> Option<usize> {
        let lower = query.to_lowercase();
        if let Some(index) = self.find_note_mentioned_in_text(query) {
            return Some(index);
        }

        for marker in ["note ", "notes ", "doc ", "draft "] {
            if let Some(pos) = lower.find(marker) {
                let candidate = query[pos + marker.len()..]
                    .split(['.', ',', ':', ';'])
                    .next()
                    .unwrap_or_default()
                    .trim_matches(|c: char| c == '"' || c == '\'' || c.is_whitespace());
                if let Some(index) = self.resolve_note_index(candidate) {
                    return Some(index);
                }
            }
        }
        self.current_note_index()
    }

    fn find_note_mentioned_in_text(&self, query: &str) -> Option<usize> {
        let lower = query.to_lowercase();
        for token in lower.split_whitespace() {
            let normalized = token
                .trim_matches(|character: char| !character.is_ascii_alphanumeric())
                .trim_start_matches('#');
            if let Ok(note_id) = normalized.parse::<usize>() {
                if let Some(index) = self
                    .notes
                    .iter()
                    .enumerate()
                    .find_map(|(index, note)| (note.id == note_id).then_some(index))
                {
                    return Some(index);
                }
            }
        }

        self.notes
            .iter()
            .enumerate()
            .filter_map(|(index, note)| {
                let title = note.title.to_lowercase();
                let title_words = title.split_whitespace().count();
                let remote_match = note
                    .remote_id
                    .as_deref()
                    .map(|remote_id| lower.contains(&remote_id.to_lowercase()))
                    .unwrap_or(false);

                if remote_match || (!title.is_empty() && lower.contains(&title)) {
                    Some((index, title_words))
                } else {
                    None
                }
            })
            .max_by_key(|(_, title_words)| *title_words)
            .map(|(index, _)| index)
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
                                vec![String::from("Unable to start the browser-based OpenRouter authorization flow.")],
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

    fn split_note_body_args(args: &str) -> (&str, &str) {
        args.split_once("::").unwrap_or((args, ""))
    }

    fn open_note_list_panel(&mut self) {
        if self.is_strix_connected() {
            self.ensure_cached_strix_notes_loaded();
        }

        let previous_selected_note = self
            .note_list_indices
            .get(self.note_list_selected)
            .copied()
            .and_then(|index| self.notes.get(index).map(|note| note.id));
        let folder_id = self.current_folder_id;
        let folder_name = folder_id
            .and_then(|id| self.get_folder_name(id))
            .unwrap_or_else(|| String::from("All notes"));

        self.note_list_indices = self
            .notes
            .iter()
            .enumerate()
            .filter(|(_, note)| folder_id.is_none() || note.folder_id == folder_id)
            .map(|(index, _)| index)
            .collect();

        self.panel_lines = self
            .note_list_indices
            .iter()
            .enumerate()
            .map(|(list_index, &note_index)| self.note_list_line(list_index, note_index))
            .collect::<Vec<_>>();

        self.note_list_selected = previous_selected_note
            .and_then(|note_id| {
                self.note_list_indices
                    .iter()
                    .position(|&index| self.notes.get(index).map(|note| note.id) == Some(note_id))
            })
            .unwrap_or(0)
            .min(self.note_list_indices.len().saturating_sub(1));
        self.panel_mode = PanelMode::NoteList;
        self.panel_title = format!("Notes - {} (Enter open, Delete delete)", folder_name);
    }

    fn note_list_line(&self, list_index: usize, note_index: usize) -> String {
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

    fn truncate_chars(value: &str, max_chars: usize) -> String {
        value.chars().take(max_chars).collect()
    }

    fn create_note_from_content(&mut self, title: &str, content: &str) -> Result<usize, String> {
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

    fn delete_note_at_index(&mut self, index: usize) -> Result<String, String> {
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

        Ok(note.title)
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
        self.strix_logs
            .push(format!("[{}] {}", timestamp, message.into()));
        // Keep only last 50 log entries
        if self.strix_logs.len() > 50 {
            self.strix_logs.drain(0..self.strix_logs.len() - 50);
        }
    }

    fn set_ai_provider(&mut self, provider: AiProvider) {
        self.ai_provider = provider;
        let _ = self.store_ai_provider();
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
            self.last_action = String::from(
                "OpenRouter provider setup is already running. Use /logout to cancel it first.",
            );
            return false;
        }

        let (sender, receiver) = mpsc::channel();
        let cancel_flag = Arc::new(AtomicBool::new(false));
        self.openrouter_login_rx = Some(receiver);
        self.openrouter_login_cancel = Some(cancel_flag.clone());
        self.set_result_panel(
            "OpenRouter provider setup",
            vec![
                String::from("A browser window will open for OpenRouter authorization."),
                String::from("After you authorize Aleph, the API key will be stored locally."),
                String::from("If the browser does not open, copy the auth URL from the terminal."),
            ],
        );
        self.last_action = String::from("Starting OpenRouter provider setup.");

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
        let callback_url = format!("http://localhost:{}{}", OPENROUTER_AUTH_PORT, callback_path);
        let auth_url = format!(
            "https://openrouter.ai/auth?callback_url={}&code_challenge={}&code_challenge_method=S256",
            urlencoding::encode(&callback_url),
            urlencoding::encode(&code_challenge),
        );

        let listener = TcpListener::bind(("127.0.0.1", OPENROUTER_AUTH_PORT)).map_err(|error| {
            format!(
                "failed to bind local OpenRouter callback listener: {}",
                error
            )
        })?;
        listener
            .set_nonblocking(true)
            .map_err(|error| format!("failed to configure the callback listener: {}", error))?;

        Self::open_browser(&auth_url)?;

        let deadline = Instant::now() + Duration::from_secs(600);
        let (mut stream, _) = loop {
            if cancel_flag.load(Ordering::Relaxed) {
                return Err(String::from("OpenRouter authorization was canceled."));
            }

            match listener.accept() {
                Ok(connection) => break connection,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    if Instant::now() >= deadline {
                        return Err(String::from(
                            "OpenRouter authorization timed out waiting for the callback.",
                        ));
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
            "OpenRouter authorization completed. You can return to Aleph now.",
        )?;

        if cancel_flag.load(Ordering::Relaxed) {
            return Err(String::from("OpenRouter authorization was canceled."));
        }

        Self::exchange_openrouter_code_for_key(&code, &code_verifier)
    }

    fn start_strix_browser_login(&mut self) -> bool {
        if self.strix_login_rx.is_some() {
            self.last_action =
                String::from("Strix login is already running. Use /logout to cancel it first.");
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
                String::from(
                    "After you authenticate, Aleph receives a native app token via localhost.",
                ),
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
        listener.set_nonblocking(true).map_err(|error| {
            format!("failed to configure the Strix callback listener: {}", error)
        })?;

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
                        return Err(String::from(
                            "Strix browser login timed out waiting for the callback.",
                        ));
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
            return Err(String::from(
                "Strix callback state did not match the login request.",
            ));
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
            reader.read_line(&mut request_line).map_err(|error| {
                format!("failed to read {} callback request: {}", provider, error)
            })?;

            let mut header = String::new();
            loop {
                header.clear();
                let bytes_read = reader.read_line(&mut header).map_err(|error| {
                    format!("failed to read {} callback headers: {}", provider, error)
                })?;
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
            return Err(format!(
                "{} callback arrived on an unexpected path.",
                provider
            ));
        }

        Ok(request_path)
    }

    fn write_openrouter_callback_response(
        stream: &mut std::net::TcpStream,
        message: &str,
    ) -> Result<(), String> {
        Self::write_oauth_callback_response(
            stream,
            "OpenRouter authorization complete",
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
            .map_err(|error| {
                format!(
                    "failed to exchange the OpenRouter authorization code: {}",
                    error
                )
            })?;

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
            .map_err(|error| {
                format!("failed to exchange the Strix authorization code: {}", error)
            })?;

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
                return urlencoding::decode(value)
                    .ok()
                    .map(|decoded| decoded.into_owned());
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
                Style::default()
                    .fg(CHAT_TEXT)
                    .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
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
                Style::default()
                    .fg(CHAT_TEXT)
                    .add_modifier(Modifier::BOLD | Modifier::ITALIC),
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
                spans.push(Span::styled(
                    trimmed[..=pos + 1].to_string(),
                    Style::default().fg(CHAT_ACCENT),
                ));
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
            self.last_action = String::from("Aleph is still answering the previous message.");
            return false;
        }

        let provider = self.ai_provider;
        let openrouter_api_key = self.openrouter_api_key.clone();
        let strix_access_token = self.strix_access_token.clone();

        self.push_chat_message("user", query.clone());

        let conversation = match provider {
            AiProvider::OpenRouter => self.openrouter_conversation(),
            AiProvider::Strix => Vec::new(),
        };
        let strix_notes = if provider == AiProvider::Strix {
            self.notes.clone()
        } else {
            Vec::new()
        };

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

        match provider {
            AiProvider::OpenRouter => {
                let Some(api_key) = openrouter_api_key else {
                    let _ = sender.send(ChatStreamUpdate::Error(String::from(
                        "OpenRouter is not configured as a model provider. Run /login openrouter first.",
                    )));
                    return true;
                };
                thread::spawn(move || {
                    if let Err(error) = Self::send_openrouter_chat_streaming(
                        &api_key,
                        &conversation,
                        sender.clone(),
                    ) {
                        let _ = sender.send(ChatStreamUpdate::Error(error));
                    }
                });
            }
            AiProvider::Strix => {
                let Some(access_token) = strix_access_token else {
                    let _ = sender.send(ChatStreamUpdate::Error(String::from(
                        "Strix is not connected. Run /login strix first.",
                    )));
                    return true;
                };
                let base_url = Self::strix_api_base_url();
                thread::spawn(move || {
                    if let Err(error) = Self::send_strix_chat(
                        &base_url,
                        &access_token,
                        &query,
                        &strix_notes,
                        sender.clone(),
                    ) {
                        let _ = sender.send(ChatStreamUpdate::Error(error));
                    }
                });
            }
        }

        true
    }

    fn openrouter_conversation(&self) -> Vec<(String, String)> {
        let mut conversation = Vec::new();
        conversation.push((
            String::from("system"),
            String::from("You are Aleph, a concise terminal assistant. Keep answers practical and grounded. If the user asks for detail, expand, but default to short, useful responses."),
        ));

        let mut recent_messages: Vec<_> =
            self.chat_messages.iter().rev().take(12).cloned().collect();
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
                if self.is_openrouter_connected() || self.is_strix_connected() {
                    "Welcome to Aleph AI chat. Type a message below to start."
                } else {
                    "Welcome to Aleph AI chat. Run /login to sign in."
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
            let color = if is_user {
                CHAT_ACCENT_SOFT
            } else {
                CHAT_ACCENT
            };

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
                        self.chat_render_cache
                            .push(Self::render_chat_markdown_line_owned(content_line));
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
            .map(|(role, content)| {
                serde_json::json!({
                    "role": role,
                    "content": content,
                })
            })
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
                if !event_data.is_empty()
                    && Self::handle_openrouter_stream_event(&event_data, &sender)?
                {
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

            if let Some(finish_reason) = choice
                .get("finish_reason")
                .and_then(|reason| reason.as_str())
            {
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

    fn send_openrouter_chat_blocking(
        api_key: &str,
        conversation: &[(String, String)],
    ) -> Result<String, String> {
        let messages: Vec<_> = conversation
            .iter()
            .map(|(role, content)| {
                serde_json::json!({
                    "role": role,
                    "content": content,
                })
            })
            .collect();

        let payload = serde_json::json!({
            "model": OPENROUTER_CHAT_MODEL,
            "messages": messages,
            "temperature": 0.1,
            "stream": false,
        });

        let client = Client::builder()
            .timeout(Duration::from_secs(45))
            .build()
            .map_err(|error| format!("failed to build HTTP client: {}", error))?;

        let response = client
            .post("https://openrouter.ai/api/v1/chat/completions")
            .bearer_auth(api_key)
            .json(&payload)
            .send()
            .map_err(|error| format!("request failed: {}", error))?;
        let status = response.status();
        let body = response
            .text()
            .map_err(|error| format!("failed to read response: {}", error))?;

        if !status.is_success() {
            return Err(format!("{}: {}", status, body));
        }

        let value: serde_json::Value = serde_json::from_str(&body)
            .map_err(|error| format!("failed to parse OpenRouter response: {}", error))?;
        value
            .get("choices")
            .and_then(|choices| choices.get(0))
            .and_then(|choice| choice.get("message"))
            .and_then(|message| message.get("content"))
            .and_then(|content| content.as_str())
            .map(|content| content.trim().to_string())
            .filter(|content| !content.is_empty())
            .ok_or_else(|| String::from("OpenRouter returned an empty planner response"))
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
        Self::strix_json_request_with(&Self::strix_api_base_url(), token, method, path, payload)
    }

    fn strix_json_request_with(
        base_url: &str,
        token: &str,
        method: &str,
        path: &str,
        payload: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, String> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|error| format!("failed to build HTTP client: {}", error))?;
        let url = format!("{}{}", base_url.trim_end_matches('/'), path);
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
        serde_json::from_str(&body)
            .map_err(|error| format!("failed to parse Strix response: {}", error))
    }

    fn send_strix_chat(
        base_url: &str,
        token: &str,
        query: &str,
        notes: &[Note],
        sender: Sender<ChatStreamUpdate>,
    ) -> Result<(), String> {
        let notes_payload: Vec<_> = notes
            .iter()
            .take(STRIX_NOTES_LIMIT)
            .map(|note| {
                let id = note
                    .remote_id
                    .clone()
                    .unwrap_or_else(|| note.id.to_string());
                serde_json::json!({
                    "id": id,
                    "title": note.title.as_str(),
                    "content": note.content.as_str(),
                })
            })
            .collect();
        let payload = serde_json::json!({
            "question": query,
            "notes": notes_payload,
        });
        let value =
            Self::strix_json_request_with(base_url, token, "POST", "/nest/ask", Some(payload))?;
        let answer = value
            .get("answer")
            .or_else(|| value.get("result").and_then(|result| result.get("answer")))
            .or_else(|| value.get("content"))
            .and_then(|answer| answer.as_str())
            .unwrap_or("Strix returned an empty answer.")
            .to_string();
        let _ = sender.send(ChatStreamUpdate::Delta(answer));
        let _ = sender.send(ChatStreamUpdate::Done);
        Ok(())
    }

    fn send_strix_planner_request(
        base_url: &str,
        token: &str,
        conversation: &[(String, String)],
        notes: &[Note],
    ) -> Result<String, String> {
        let query = conversation
            .iter()
            .map(|(role, content)| format!("{}:\n{}", role, content))
            .collect::<Vec<_>>()
            .join("\n\n");
        let notes_payload: Vec<_> = notes
            .iter()
            .take(STRIX_NOTES_LIMIT)
            .map(|note| {
                let id = note
                    .remote_id
                    .clone()
                    .unwrap_or_else(|| note.id.to_string());
                serde_json::json!({
                    "id": id,
                    "title": note.title.as_str(),
                    "content": note.content.as_str(),
                })
            })
            .collect();
        let payload = serde_json::json!({
            "question": query,
            "notes": notes_payload,
        });
        let value =
            Self::strix_json_request_with(base_url, token, "POST", "/nest/ask", Some(payload))?;
        value
            .get("answer")
            .or_else(|| value.get("result").and_then(|result| result.get("answer")))
            .or_else(|| value.get("content"))
            .and_then(|answer| answer.as_str())
            .map(|answer| answer.trim().to_string())
            .filter(|answer| !answer.is_empty())
            .ok_or_else(|| String::from("Strix returned an empty planner response"))
    }

    fn sync_strix_notes(&mut self) -> Result<usize, String> {
        let remote_notes = self.load_strix_notes("", STRIX_NOTES_LIMIT)?;
        let count = remote_notes.len();
        self.merge_strix_notes(remote_notes);
        self.selected_note = 0;
        Self::save_cached_strix_notes(&self.notes)?;
        self.add_strix_log(format!("Synced {} notes", count));
        Ok(count)
    }

    fn merge_strix_notes(&mut self, remote_notes: Vec<Note>) {
        let existing_by_remote_id: HashMap<String, Note> = self
            .notes
            .iter()
            .filter_map(|note| {
                note.remote_id
                    .as_ref()
                    .map(|remote_id| (remote_id.clone(), note.clone()))
            })
            .collect();
        let mut merged = Vec::with_capacity(remote_notes.len() + self.notes.len());
        let mut remote_ids = Vec::new();

        for mut note in remote_notes {
            if let Some(remote_id) = note.remote_id.clone() {
                if let Some(existing) = existing_by_remote_id.get(&remote_id) {
                    note.id = existing.id;
                    if note.obsidian_path.is_none() {
                        note.obsidian_path = existing.obsidian_path.clone();
                    }
                    if note.folder_id.is_none() {
                        note.folder_id = existing.folder_id;
                    }
                }
                remote_ids.push(remote_id);
            }
            merged.push(note);
        }

        for note in &self.notes {
            let is_matched_remote_note = note
                .remote_id
                .as_ref()
                .map(|remote_id| remote_ids.iter().any(|id| id == remote_id))
                .unwrap_or(false);
            if !is_matched_remote_note {
                merged.push(note.clone());
            }
        }

        for (index, note) in merged.iter_mut().enumerate() {
            note.id = index + 1;
        }
        self.notes = merged;
    }

    fn handle_obsidian_pair(&mut self, target: &str) {
        self.refresh_obsidian_vaults();
        if target.is_empty() {
            if self.obsidian_vaults.len() == 1 {
                let path = self.obsidian_vaults[0].path.clone();
                match self.pair_obsidian_vault(path) {
                    Ok(message) => {
                        self.set_result_panel(
                            "Obsidian paired",
                            vec![
                                message,
                                String::from("Run /obsidian sync to import Markdown notes."),
                            ],
                        );
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
                self.set_result_panel(
                    "Obsidian paired",
                    vec![
                        message,
                        String::from("Run /obsidian sync to import Markdown notes."),
                    ],
                );
                self.last_action = String::from("Paired Obsidian vault.");
            }
            Err(error) => {
                self.set_result_panel("Obsidian pairing failed", vec![error]);
                self.last_action = String::from("Obsidian pairing failed.");
            }
        }
    }

    fn open_vault_picker(&mut self) {
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

    fn pair_obsidian_vault(&mut self, path: PathBuf) -> Result<String, String> {
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

    fn sync_obsidian_notes(&mut self) -> Result<usize, String> {
        let vault_path = self.obsidian_vault_path.clone().ok_or_else(|| {
            String::from("No Obsidian vault is paired. Run /obsidian pair first.")
        })?;
        let files = Self::collect_markdown_files(&vault_path)?;
        let folder_root_id = self.ensure_folder_path(&[String::from("Obsidian")], None);
        let vault_name = Self::vault_display_name(&vault_path);
        let vault_folder_id = self.ensure_folder_path(&[vault_name], Some(folder_root_id));
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
        Ok(imported)
    }

    fn upsert_obsidian_note(
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

    fn obsidian_folder_id(&mut self, relative: &Path, vault_folder_id: usize) -> Option<usize> {
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

    fn ensure_folder_path(&mut self, parts: &[String], parent_id: Option<usize>) -> usize {
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
            last_id = id;
            parent = Some(id);
        }
        last_id
    }

    fn open_obsidian_target(&mut self, target: &str) -> Result<String, String> {
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

    fn obsidian_status_label(&self) -> String {
        self.obsidian_vault_path
            .as_ref()
            .map(|path| format!("paired ({})", Self::vault_display_name(path)))
            .unwrap_or_else(|| String::from("not paired"))
    }

    fn refresh_obsidian_vaults(&mut self) {
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

    fn format_obsidian_vault_lines(&self) -> Vec<String> {
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

    fn resolve_obsidian_vault_target(&self, target: &str) -> Option<PathBuf> {
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

    fn collect_markdown_files(root: &Path) -> Result<Vec<PathBuf>, String> {
        let mut files = Vec::new();
        Self::collect_markdown_files_inner(root, &mut files)?;
        files.sort();
        Ok(files)
    }

    fn collect_markdown_files_inner(dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
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

    fn obsidian_note_title(path: &Path, relative: &Path) -> String {
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

    fn obsidian_note_path_for_title(&self, title: &str) -> Option<PathBuf> {
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

    fn safe_obsidian_filename(title: &str) -> String {
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

    fn discover_obsidian_vaults() -> Vec<ObsidianVault> {
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

    fn obsidian_config_path() -> PathBuf {
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

    fn load_obsidian_vault_path() -> Option<PathBuf> {
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
        std::env::var("ALEPH_OBSIDIAN_VAULT")
            .ok()
            .map(|path| PathBuf::from(Self::expand_home(path.trim())))
            .filter(|path| path.is_dir())
    }

    fn store_obsidian_vault_path(&self, path: &Path) -> Result<(), String> {
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

    fn obsidian_vault_entry() -> Result<Entry, String> {
        Entry::new(OBSIDIAN_SERVICE, OBSIDIAN_ACCOUNT)
            .map_err(|error| format!("failed to open Obsidian credential store: {}", error))
    }

    fn note_save_target_name(target: NoteSaveTarget) -> &'static str {
        match target {
            NoteSaveTarget::Local => "Local",
            NoteSaveTarget::Obsidian => "Obsidian",
            NoteSaveTarget::Strix => "Strix",
        }
    }

    fn note_save_target_config_value(target: NoteSaveTarget) -> &'static str {
        match target {
            NoteSaveTarget::Local => "local",
            NoteSaveTarget::Obsidian => "obsidian",
            NoteSaveTarget::Strix => "strix",
        }
    }

    fn parse_note_save_target(value: &str) -> Option<NoteSaveTarget> {
        match value.trim().to_ascii_lowercase().as_str() {
            "local" => Some(NoteSaveTarget::Local),
            "obsidian" => Some(NoteSaveTarget::Obsidian),
            "strix" => Some(NoteSaveTarget::Strix),
            _ => None,
        }
    }

    fn note_save_target_is_available(
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

    fn ai_provider_config_value(provider: AiProvider) -> &'static str {
        match provider {
            AiProvider::OpenRouter => "openrouter",
            AiProvider::Strix => "strix",
        }
    }

    fn parse_ai_provider(value: &str) -> Option<AiProvider> {
        match value.trim().to_ascii_lowercase().as_str() {
            "openrouter" => Some(AiProvider::OpenRouter),
            "strix" => Some(AiProvider::Strix),
            _ => None,
        }
    }

    fn load_ai_provider() -> Option<AiProvider> {
        fs::read_to_string(Self::ai_provider_path())
            .ok()
            .and_then(|value| Self::parse_ai_provider(&value))
    }

    fn store_ai_provider(&self) -> Result<(), String> {
        Self::write_config_value(
            Self::ai_provider_path(),
            Self::ai_provider_config_value(self.ai_provider),
            "AI provider setting",
        )
    }

    fn load_agent_mode_enabled() -> Option<bool> {
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

    fn store_agent_mode_enabled(&self) -> Result<(), String> {
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

    fn load_note_save_target() -> Option<NoteSaveTarget> {
        fs::read_to_string(Self::note_save_target_path())
            .ok()
            .and_then(|value| Self::parse_note_save_target(&value))
    }

    fn store_note_save_target(&self) -> Result<(), String> {
        Self::write_config_value(
            Self::note_save_target_path(),
            Self::note_save_target_config_value(self.note_save_target),
            "note storage setting",
        )
    }

    fn write_config_value(
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

    fn note_save_target_path() -> PathBuf {
        Self::aleph_config_dir().join(NOTE_SAVE_TARGET_CONFIG)
    }

    fn ai_provider_path() -> PathBuf {
        Self::aleph_config_dir().join(AI_PROVIDER_CONFIG)
    }

    fn agent_mode_path() -> PathBuf {
        Self::aleph_config_dir().join(AGENT_MODE_CONFIG)
    }

    fn aleph_config_dir() -> PathBuf {
        if let Ok(dir) = std::env::var("ALEPH_CONFIG_DIR") {
            return PathBuf::from(dir);
        }
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

    fn obsidian_pairing_path() -> PathBuf {
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

    fn vault_display_name(path: &Path) -> String {
        path.file_name()
            .and_then(|name| name.to_str())
            .map(|name| name.to_string())
            .unwrap_or_else(|| path.display().to_string())
    }

    fn stable_vault_id(path: &Path) -> String {
        let mut hasher = Sha256::new();
        hasher.update(path.display().to_string().as_bytes());
        let digest = hasher.finalize();
        URL_SAFE_NO_PAD.encode(&digest[..9])
    }

    fn expand_home(path: &str) -> String {
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

    fn delete_strix_note(&self, note: &Note) -> Result<(), String> {
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

    fn push_note_to_strix(&mut self, index: usize) -> Result<(), String> {
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

    fn upsert_synced_note(&mut self, mut note: Note) {
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

    fn note_source_label(note: &Note) -> String {
        if let Some(path) = note.obsidian_path.as_ref() {
            return format!("Obsidian: {}", path.display());
        }
        if let Some(remote_id) = note.remote_id.as_deref() {
            return format!("Strix ID: {}", remote_id);
        }
        String::from("Source: local-only")
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
            return PathBuf::from(home)
                .join(".cache")
                .join("aleph")
                .join("strix-notes.json");
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
            .map_err(|error| format!("failed to save OpenRouter API key: {}", error))
    }

    fn clear_openrouter_api_key(&self) {
        if let Ok(entry) = Self::openrouter_key_entry() {
            let _ = entry.delete_credential();
        }
    }

    fn reset_and_clear_all(&mut self) {
        // Clear API keys from keyring
        self.clear_openrouter_api_key();
        self.clear_strix_access_token();

        // Clear all cached data files
        let _ = std::fs::remove_file(Self::strix_cache_path());
        let _ = std::fs::remove_file(Self::ai_provider_path());
        let _ = std::fs::remove_file(Self::note_save_target_path());
        let _ = std::fs::remove_file(Self::agent_mode_path());
        let _ = std::fs::remove_file(Self::obsidian_pairing_path());

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
        self.notes.clear();
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

    fn openrouter_key_entry() -> Result<Entry, String> {
        Entry::new(OPENROUTER_SERVICE, OPENROUTER_ACCOUNT)
            .map_err(|error| format!("failed to open OpenRouter API key store: {}", error))
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
            note.raw_content = self.editor_buffer.clone();
            note.updated_at = updated_at;
        }
        if let Err(error) = self.persist_note(index) {
            self.last_action = format!("Note save failed: {}", error);
        }
        self.save_shimmer_ticks = 4;
    }

    fn persist_note(&mut self, index: usize) -> Result<(), String> {
        match self.note_save_target {
            NoteSaveTarget::Local => Ok(()),
            NoteSaveTarget::Obsidian => {
                self.ensure_note_obsidian_path(index)?;
                self.write_note_to_obsidian(index)
            }
            NoteSaveTarget::Strix => self.push_note_to_strix(index),
        }
    }

    fn ensure_note_obsidian_path(&mut self, index: usize) -> Result<(), String> {
        if self.obsidian_vault_path.is_none() {
            return Err(String::from(
                "Obsidian save target requires a paired vault. Use /obsidian pair first.",
            ));
        }

        if self
            .notes
            .get(index)
            .and_then(|note| note.obsidian_path.as_ref())
            .is_some()
        {
            return Ok(());
        }

        let title = self
            .notes
            .get(index)
            .map(|note| note.title.clone())
            .unwrap_or_else(|| String::from("Untitled note"));
        let path = self
            .obsidian_note_path_for_title(&title)
            .ok_or_else(|| String::from("Unable to choose an Obsidian note path."))?;
        if let Some(note) = self.notes.get_mut(index) {
            note.obsidian_path = Some(path);
        }
        Ok(())
    }

    fn write_note_to_obsidian(&self, index: usize) -> Result<(), String> {
        let Some(note) = self.notes.get(index) else {
            return Ok(());
        };
        let Some(path) = note.obsidian_path.as_ref() else {
            return Ok(());
        };
        fs::write(path, &note.content)
            .map_err(|error| format!("failed to write '{}': {}", path.display(), error))
    }

    fn exit_editor(&mut self) {
        self.save_editor();
        let index = self.editor_note_index.unwrap_or(0);
        let note_title = self
            .notes
            .get(index)
            .map(|n| n.title.clone())
            .unwrap_or_default();

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
            Self::note_source_label(note),
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
            if remote_matches || note.title.eq_ignore_ascii_case(trimmed) || title.contains(&lower)
            {
                Some(index)
            } else {
                None
            }
        })
    }

    fn note_index_by_id(&self, id: usize) -> Option<usize> {
        self.notes
            .iter()
            .enumerate()
            .find_map(|(index, note)| (note.id == id).then_some(index))
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
                format!(
                    "{} {} — {}",
                    id_label,
                    note.title,
                    Self::preview_text(&note.content, 56)
                )
            })
            .collect()
    }

    fn preview_text(content: &str, limit: usize) -> String {
        let collapsed = content.split_whitespace().collect::<Vec<_>>().join(" ");
        let preview = collapsed.trim();

        if preview.chars().count() <= limit {
            return preview.to_string();
        }

        preview
            .chars()
            .take(limit.saturating_sub(1))
            .collect::<String>()
            + "…"
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
            return vec![String::from(
                "No folders created yet. Use /folder create <name>",
            )];
        }

        self.folders
            .iter()
            .map(|folder| {
                let prefix = if folder.parent_id.is_some() { "  " } else { "" };
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
        self.clear_editor_selection();
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
        self.editor_buffer
            .drain(self.editor_cursor - previous..self.editor_cursor);
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
        self.editor_buffer
            .drain(self.editor_cursor..self.editor_cursor + next);
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

    fn select_all_editor(&mut self) {
        let buffer_len = if self.has_live_ai_editor_preview() {
            self.editor_display_buffer().len()
        } else {
            self.editor_buffer.len()
        };
        self.editor_selection.select_all(buffer_len);
    }

    fn clear_editor_selection(&mut self) {
        self.editor_selection.clear();
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

    fn start_title_edit(&mut self) {
        if let Some(index) = self.editor_note_index {
            self.editing_title = true;
            self.title_buffer = self.notes[index].title.clone();
            self.title_cursor = self.title_buffer.len();
            self.last_action = String::from("Editing title. Press Enter to save, Esc to cancel.");
        }
    }

    fn finish_title_edit(&mut self, save: bool) {
        if save && !self.title_buffer.trim().is_empty() {
            if let Some(index) = self.editor_note_index {
                self.notes[index].title = self.title_buffer.trim().to_string();
                self.panel_title = format!("Editing: {}", self.notes[index].title);
                self.last_action = format!("Title updated to: {}", self.notes[index].title);
            }
        } else if !save {
            self.last_action = String::from("Title edit cancelled.");
        }
        self.editing_title = false;
        self.title_buffer.clear();
        self.title_cursor = 0;
    }

    fn handle_title_edit_key(&mut self, key_event: KeyEvent) {
        if key_event.kind != KeyEventKind::Press && key_event.kind != KeyEventKind::Repeat {
            return;
        }
        self.title_cursor = Self::clamp_to_char_boundary(&self.title_buffer, self.title_cursor);
        match key_event.code {
            KeyCode::Enter => {
                self.finish_title_edit(true);
            }
            KeyCode::Esc => {
                self.finish_title_edit(false);
            }
            KeyCode::Backspace => {
                if self.title_cursor > 0 {
                    let previous =
                        Self::previous_char_boundary(&self.title_buffer, self.title_cursor);
                    self.title_buffer.drain(previous..self.title_cursor);
                    self.title_cursor = previous;
                }
            }
            KeyCode::Delete => {
                if self.title_cursor < self.title_buffer.len() {
                    let next = Self::next_char_boundary(&self.title_buffer, self.title_cursor);
                    self.title_buffer.drain(self.title_cursor..next);
                }
            }
            KeyCode::Left => {
                if self.title_cursor > 0 {
                    self.title_cursor =
                        Self::previous_char_boundary(&self.title_buffer, self.title_cursor);
                }
            }
            KeyCode::Right => {
                if self.title_cursor < self.title_buffer.len() {
                    self.title_cursor =
                        Self::next_char_boundary(&self.title_buffer, self.title_cursor);
                }
            }
            KeyCode::Home => {
                self.title_cursor = 0;
            }
            KeyCode::End => {
                self.title_cursor = self.title_buffer.len();
            }
            KeyCode::Char(c) if !key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                self.title_buffer.insert(self.title_cursor, c);
                self.title_cursor += c.len_utf8();
            }
            _ => {}
        }
    }

    fn previous_char_boundary(input: &str, cursor: usize) -> usize {
        let cursor = Self::clamp_to_char_boundary(input, cursor);
        input[..cursor]
            .char_indices()
            .next_back()
            .map(|(index, _)| index)
            .unwrap_or(0)
    }

    fn next_char_boundary(input: &str, cursor: usize) -> usize {
        let cursor = Self::clamp_to_char_boundary(input, cursor);
        input[cursor..]
            .chars()
            .next()
            .map(|character| cursor + character.len_utf8())
            .unwrap_or_else(|| input.len())
    }

    fn clamp_to_char_boundary(input: &str, cursor: usize) -> usize {
        let mut cursor = cursor.min(input.len());
        while cursor > 0 && !input.is_char_boundary(cursor) {
            cursor -= 1;
        }
        cursor
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
        if self.editing_title {
            self.handle_title_edit_key(key_event);
            return;
        }

        if self.search_state.active {
            self.handle_search_key(key_event);
            return;
        }

        if self.ai_overlay_visible {
            match key_event.code {
                KeyCode::Enter
                    if key_event.kind == KeyEventKind::Press && self.has_pending_ai_edit() =>
                {
                    self.apply_pending_ai_edit();
                    return;
                }
                KeyCode::Char('r')
                    if key_event.kind == KeyEventKind::Press
                        && key_event.modifiers.contains(KeyModifiers::CONTROL)
                        && self.has_pending_ai_edit() =>
                {
                    self.reject_pending_ai_edit();
                    return;
                }
                KeyCode::Esc if key_event.kind == KeyEventKind::Press => {
                    if self.has_pending_ai_edit() {
                        self.reject_pending_ai_edit();
                    } else {
                        self.close_ai_overlay();
                    }
                    return;
                }
                KeyCode::Char(' ')
                    if key_event.kind == KeyEventKind::Press
                        && key_event.modifiers.contains(KeyModifiers::CONTROL) =>
                {
                    if self.has_pending_ai_edit() {
                        self.last_action =
                            String::from("Apply or reject the pending AI edits first.");
                        return;
                    }
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
                KeyCode::Char('a')
                    if key_event.kind == KeyEventKind::Press
                        && key_event.modifiers.contains(KeyModifiers::CONTROL) =>
                {
                    self.select_all_editor();
                    return;
                }
                _ => {
                    if self.has_pending_ai_edit() {
                        self.last_action =
                            String::from("Apply or reject the pending AI edits first.");
                        return;
                    }
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
            KeyCode::Char('a')
                if key_event.kind == KeyEventKind::Press
                    && key_event.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                self.select_all_editor();
            }
            KeyCode::Esc if key_event.kind == KeyEventKind::Press => self.exit_editor(),
            KeyCode::Tab if key_event.kind == KeyEventKind::Press => {
                self.start_title_edit();
            }
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
            KeyCode::Char('a')
                if key_event.kind == KeyEventKind::Press
                    && key_event.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                self.select_all_editor();
            }
            KeyCode::Enter if key_event.kind == KeyEventKind::Press => {
                self.save_undo_state();
                self.clear_editor_selection();
                self.insert_editor_character('\n');
            }
            KeyCode::Backspace if key_event.kind == KeyEventKind::Press => {
                self.clear_editor_selection();
                self.editor_backspace()
            }
            KeyCode::Delete if key_event.kind == KeyEventKind::Press => {
                self.clear_editor_selection();
                self.editor_delete()
            }
            KeyCode::Left
                if matches!(key_event.kind, KeyEventKind::Press | KeyEventKind::Repeat) =>
            {
                self.clear_editor_selection();
                self.editor_move_left()
            }
            KeyCode::Right
                if matches!(key_event.kind, KeyEventKind::Press | KeyEventKind::Repeat) =>
            {
                self.clear_editor_selection();
                self.editor_move_right()
            }
            KeyCode::Up
                if matches!(key_event.kind, KeyEventKind::Press | KeyEventKind::Repeat)
                    && !key_event.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                self.clear_editor_selection();
                self.editor_move_up()
            }
            KeyCode::Down
                if matches!(key_event.kind, KeyEventKind::Press | KeyEventKind::Repeat)
                    && !key_event.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                self.clear_editor_selection();
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
            KeyCode::Home if key_event.kind == KeyEventKind::Press => {
                self.clear_editor_selection();
                self.editor_cursor = 0
            }
            KeyCode::End if key_event.kind == KeyEventKind::Press => {
                self.clear_editor_selection();
                self.editor_cursor = self.editor_buffer.len()
            }
            KeyCode::Char('z')
                if key_event.kind == KeyEventKind::Press
                    && key_event.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                self.clear_editor_selection();
                self.undo();
            }
            KeyCode::Char('y')
                if key_event.kind == KeyEventKind::Press
                    && key_event.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                self.clear_editor_selection();
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
                    self.ai_input_buffer
                        .drain(self.ai_input_cursor - prev..self.ai_input_cursor);
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
                    self.ai_input_buffer
                        .drain(self.ai_input_cursor..self.ai_input_cursor + next);
                }
            }
            KeyCode::Left
                if matches!(key_event.kind, KeyEventKind::Press | KeyEventKind::Repeat) =>
            {
                if self.ai_input_cursor > 0 {
                    let prev = self.ai_input_buffer[..self.ai_input_cursor]
                        .chars()
                        .next_back()
                        .map(|c| c.len_utf8())
                        .unwrap_or(1);
                    self.ai_input_cursor -= prev;
                }
            }
            KeyCode::Right
                if matches!(key_event.kind, KeyEventKind::Press | KeyEventKind::Repeat) =>
            {
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
        self.pending_ai_edit = None;
        self.ai_draft_create_title = None;
        self.last_action = String::from("Opened AI note editor.");
    }

    fn close_ai_overlay(&mut self) {
        self.ai_overlay_visible = false;
        self.ai_overlay_pulse_ticks = 0;
        self.thinking = false;
        self.thinking_ticks_remaining = 0;
        self.ghost_result = None;
        self.pending_ai_edit = None;
        self.ai_draft_create_title = None;
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
        self.last_action = String::from("Choose a Strix account or model provider.");
    }

    fn open_settings_panel(&mut self) {
        self.panel_mode = PanelMode::Settings;
        self.panel_title = String::from("Settings");
        self.panel_lines.clear();
        self.settings_selected = 0;
        self.last_action = String::from("Open settings to manage connections and preferences.");
    }

    fn handle_settings_key(&mut self, key_event: KeyEvent) {
        if key_event.kind != KeyEventKind::Press && key_event.kind != KeyEventKind::Repeat {
            return;
        }
        match key_event.code {
            KeyCode::Esc => {
                self.panel_mode = PanelMode::Commands;
                self.panel_title = String::from("Commands");
                self.panel_lines.clear();
                self.last_action = String::from("Closed settings.");
            }
            KeyCode::Up => {
                if self.settings_selected > 0 {
                    self.settings_selected -= 1;
                }
            }
            KeyCode::Down => {
                if self.settings_selected < 6 {
                    self.settings_selected += 1;
                }
            }
            KeyCode::Enter => {
                match self.settings_selected {
                    0 => {
                        let next_provider = match self.ai_provider {
                            AiProvider::OpenRouter => AiProvider::Strix,
                            AiProvider::Strix => AiProvider::OpenRouter,
                        };

                        self.set_ai_provider(next_provider);

                        match next_provider {
                            AiProvider::OpenRouter => {
                                if self.is_openrouter_connected() {
                                    self.rebuild_chat_render_cache();
                                    self.last_action =
                                        String::from("Switched model provider to OpenRouter.");
                                } else if !self.start_openrouter_browser_login() {
                                    self.set_result_panel(
                                        "OpenRouter provider failed",
                                        vec![String::from(
                                            "Unable to start the browser-based OpenRouter authorization flow.",
                                        )],
                                    );
                                    self.last_action =
                                        String::from("OpenRouter provider setup failed.");
                                }
                            }
                            AiProvider::Strix => {
                                if self.is_strix_connected() {
                                    self.last_action =
                                        String::from("Switched model provider to Strix.");
                                } else if !self.start_strix_browser_login() {
                                    self.set_result_panel(
                                        "Strix login failed",
                                        vec![String::from(
                                            "Unable to start the browser-based Strix login flow.",
                                        )],
                                    );
                                    self.last_action = String::from("Strix login failed.");
                                }
                            }
                        }
                    }
                    1 => {
                        self.toggle_agent_mode();
                    }
                    2 => {
                        self.cycle_note_save_target();
                    }
                    3 => {
                        // Pair Obsidian vault
                        self.open_vault_picker();
                    }
                    4 => {
                        // Sign out / Logout
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
                        self.last_action = String::from("Signed out.");
                    }
                    5 => {
                        // Reset & Clear Cache
                        self.reset_and_clear_all();
                        self.panel_mode = PanelMode::Commands;
                        self.panel_title = String::from("Commands");
                        self.panel_lines.clear();
                        self.last_action = String::from("Reset complete. All data cleared.");
                    }
                    6 => {
                        // Close settings
                        self.panel_mode = PanelMode::Commands;
                        self.panel_title = String::from("Commands");
                        self.panel_lines.clear();
                        self.last_action = String::from("Closed settings.");
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
                        // OpenRouter: start browser authorization for a model-provider API key
                        self.panel_mode = PanelMode::Commands;
                        if !self.start_openrouter_browser_login() {
                            self.set_result_panel(
                                "OpenRouter provider failed",
                                vec![String::from("Unable to start the browser-based OpenRouter authorization flow.")],
                            );
                            self.last_action = String::from("OpenRouter provider setup failed.");
                        }
                    }
                    1 => {
                        self.panel_mode = PanelMode::Commands;
                        if !self.start_strix_browser_login() {
                            self.set_result_panel(
                                "Strix login failed",
                                vec![String::from(
                                    "Unable to start the browser-based Strix login flow.",
                                )],
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

    fn handle_note_list_key(&mut self, key_event: KeyEvent) {
        if key_event.kind != KeyEventKind::Press && key_event.kind != KeyEventKind::Repeat {
            return;
        }
        match key_event.code {
            KeyCode::Esc => {
                self.note_list_pending_delete = None;
                self.panel_mode = PanelMode::Commands;
                self.panel_title = String::from("Commands");
                self.panel_lines.clear();
                self.last_action = String::from("Exited note list.");
            }
            KeyCode::Up => {
                if self.note_list_selected > 0 {
                    self.note_list_pending_delete = None;
                    self.note_list_selected -= 1;
                    self.last_action = format!("Selected note {}", self.note_list_selected + 1);
                }
            }
            KeyCode::Down => {
                if self.note_list_selected + 1 < self.note_list_indices.len() {
                    self.note_list_pending_delete = None;
                    self.note_list_selected += 1;
                    self.last_action = format!("Selected note {}", self.note_list_selected + 1);
                }
            }
            KeyCode::Enter => {
                if self.note_list_delete_is_pending() {
                    self.confirm_or_stage_note_delete();
                    return;
                }
                if let Some(&note_index) = self.note_list_indices.get(self.note_list_selected) {
                    self.open_note_editor(note_index);
                }
            }
            KeyCode::Delete | KeyCode::Backspace if key_event.kind == KeyEventKind::Press => {
                self.confirm_or_stage_note_delete();
            }
            KeyCode::Char('d') | KeyCode::Char('D')
                if key_event.kind == KeyEventKind::Press && self.note_list_delete_is_pending() =>
            {
                self.confirm_or_stage_note_delete();
            }
            KeyCode::Char('c') if key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                self.request_quit();
            }
            _ => {}
        }
    }

    fn confirm_or_stage_note_delete(&mut self) {
        let Some(&note_index) = self.note_list_indices.get(self.note_list_selected) else {
            self.last_action = String::from("No note selected to delete.");
            return;
        };
        let note_title = self
            .notes
            .get(note_index)
            .map(|note| note.title.clone())
            .unwrap_or_else(|| String::from("Untitled note"));
        if self.note_list_pending_delete == Some(note_index) {
            match self.delete_note_at_index(note_index) {
                Ok(title) => {
                    self.open_note_list_panel();
                    self.last_action = format!("Deleted note: {}", title);
                }
                Err(error) => {
                    self.note_list_pending_delete = None;
                    self.last_action = format!("Delete failed: {}", error);
                }
            }
        } else {
            self.note_list_pending_delete = Some(note_index);
            self.last_action = format!(
                "Press Delete, Enter, or d again to delete '{}'. Esc or move to cancel.",
                note_title
            );
        }
    }

    fn handle_vault_picker_key(&mut self, key_event: KeyEvent) {
        if key_event.kind != KeyEventKind::Press && key_event.kind != KeyEventKind::Repeat {
            return;
        }
        match key_event.code {
            KeyCode::Esc => {
                self.panel_mode = PanelMode::Commands;
                self.panel_title = String::from("Commands");
                self.panel_lines.clear();
                self.last_action = String::from("Cancelled Obsidian pairing.");
            }
            KeyCode::Up => {
                if self.obsidian_vault_selected > 0 {
                    self.obsidian_vault_selected -= 1;
                    self.last_action =
                        format!("Selected vault {}", self.obsidian_vault_selected + 1);
                }
            }
            KeyCode::Down => {
                if self.obsidian_vault_selected + 1 < self.obsidian_vaults.len() {
                    self.obsidian_vault_selected += 1;
                    self.last_action =
                        format!("Selected vault {}", self.obsidian_vault_selected + 1);
                }
            }
            KeyCode::Enter => {
                if let Some(vault) = self
                    .obsidian_vaults
                    .get(self.obsidian_vault_selected)
                    .cloned()
                {
                    match self.pair_obsidian_vault(vault.path) {
                        Ok(message) => {
                            self.panel_mode = PanelMode::Commands;
                            self.set_result_panel(
                                "Obsidian paired",
                                vec![
                                    message,
                                    String::from("Run /obsidian sync to import Markdown notes."),
                                ],
                            );
                            self.last_action = String::from("Paired Obsidian vault.");
                        }
                        Err(error) => {
                            self.set_result_panel("Obsidian pairing failed", vec![error]);
                            self.last_action = String::from("Obsidian pairing failed.");
                        }
                    }
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

        let provider = self.ai_provider;
        let openrouter_api_key = self.openrouter_api_key.clone();
        let strix_access_token = self.strix_access_token.clone();
        let draft_create_title = self.ai_draft_create_title.clone();

        let editor_content = self.editor_buffer.clone();
        self.ghost_streaming = true;
        self.ghost_result = None;
        self.pending_ai_edit = None;
        self.thinking = true;
        self.thinking_ticks_remaining = 20;

        // Build a conversation for the ghost editor
        let system_prompt = String::from(
            "You are Aleph's note-writing agent. You operate inside a Markdown note editor, not a chat window. \
             The user gives current note content plus an instruction. Return ONLY the complete note content that should exist after the action. \
             No explanations, no preamble, no code fences, no JSON, no commentary about what you changed. \
             For a new empty note, write a useful complete draft with a concrete title heading only when it improves the note. \
             For edits, preserve the user's meaning, useful details, markdown structure, links, lists, and factual claims unless the instruction asks to change them. \
             For append/add/insert requests, integrate the requested material into the note instead of replacing unrelated content. \
             For rewrite/improve/fix/clean-up requests, make the smallest coherent full-note rewrite that satisfies the instruction. \
             If the instruction is ambiguous, choose the most likely writing/editing action and produce the resulting note content."
        );

        let title_context = draft_create_title
            .as_deref()
            .map(|title| format!("New note title: {}\n\n", title))
            .unwrap_or_default();
        let user_msg = format!(
            "{}Current note content:\n---\n{}\n---\n\nInstruction: {}",
            title_context, editor_content, instruction
        );
        let strix_instruction = format!("{}\n\n{}", system_prompt, user_msg);

        let conversation = vec![
            (String::from("system"), system_prompt),
            (String::from("user"), user_msg),
        ];

        let (sender, receiver) = mpsc::channel();
        self.ghost_stream_rx = Some(receiver);

        match provider {
            AiProvider::OpenRouter => {
                let Some(api_key) = openrouter_api_key else {
                    self.ghost_result = Some(String::from("OpenRouter is not configured as a model provider. Run /login openrouter first."));
                    self.ghost_streaming = false;
                    self.thinking = false;
                    self.thinking_ticks_remaining = 0;
                    self.ghost_stream_rx = None;
                    return;
                };
                thread::spawn(move || {
                    if let Err(error) = Self::send_openrouter_chat_streaming(
                        &api_key,
                        &conversation,
                        sender.clone(),
                    ) {
                        let _ = sender.send(ChatStreamUpdate::Error(error));
                    }
                });
            }
            AiProvider::Strix => {
                let Some(access_token) = strix_access_token else {
                    self.ghost_result = Some(String::from(
                        "Strix is not connected. Run /login strix first.",
                    ));
                    self.ghost_streaming = false;
                    self.thinking = false;
                    self.thinking_ticks_remaining = 0;
                    self.ghost_stream_rx = None;
                    return;
                };
                let base_url = Self::strix_api_base_url();
                let notes = vec![Note {
                    id: 1,
                    remote_id: None,
                    obsidian_path: None,
                    title: String::from("Current note"),
                    content: editor_content,
                    raw_content: String::new(),
                    updated_at: String::new(),
                    folder_id: None,
                }];
                thread::spawn(move || {
                    if let Err(error) = Self::send_strix_chat(
                        &base_url,
                        &access_token,
                        &strix_instruction,
                        &notes,
                        sender.clone(),
                    ) {
                        let _ = sender.send(ChatStreamUpdate::Error(error));
                    }
                });
            }
        }

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

                    if let Some(ref result) = self.ghost_result {
                        let proposed = result.trim().to_string();
                        if proposed.is_empty() {
                            self.ghost_result =
                                Some(String::from("AI returned an empty proposal."));
                            self.last_action = String::from("AI note edit returned no changes.");
                        } else if proposed == self.editor_buffer {
                            self.ghost_result = Some(String::from("No changes proposed."));
                            self.last_action = String::from("AI note edit found no changes.");
                        } else if let Some(title) = self.ai_draft_create_title.clone() {
                            let diff_lines = Self::build_line_diff("", &proposed);
                            self.pending_ai_edit = Some(AiEditProposal {
                                note_index: None,
                                title: Some(title),
                                instruction: String::from("AI note create"),
                                proposed,
                                diff_lines,
                            });
                            self.last_action = String::from(
                                "AI drafted a note. Press Enter to create it or Ctrl+R to reject.",
                            );
                        } else if let Some(note_index) = self.editor_note_index {
                            let diff_lines = Self::build_line_diff(&self.editor_buffer, &proposed);
                            self.pending_ai_edit = Some(AiEditProposal {
                                note_index: Some(note_index),
                                title: None,
                                instruction: String::from("AI note edit"),
                                proposed,
                                diff_lines,
                            });
                            self.last_action = String::from(
                                "AI proposed edits. Press Enter to apply or Ctrl+R to reject.",
                            );
                        }
                    }
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

    fn apply_pending_ai_edit(&mut self) {
        let Some(proposal) = self.pending_ai_edit.take() else {
            return;
        };

        let Some(note_index) = proposal.note_index else {
            let title = proposal
                .title
                .clone()
                .unwrap_or_else(|| String::from("AI draft"));
            match self.create_note_from_content(&title, &proposal.proposed) {
                Ok(index) => {
                    self.selected_note = index;
                    self.open_note_editor(index);
                    self.ghost_result = None;
                    self.last_action = format!("Created AI note: {}", title);
                    self.close_ai_overlay();
                }
                Err(error) => {
                    self.ghost_result = Some(format!("Create failed: {}", error));
                    self.last_action = String::from("AI note create failed.");
                }
            }
            return;
        };

        if self.editor_note_index != Some(note_index) {
            self.open_note_editor(note_index);
        }

        self.save_undo_state();
        self.editor_buffer = proposal.proposed;
        self.editor_cursor = self.editor_buffer.len();
        self.ghost_result = None;
        self.save_editor();
        self.last_action = String::from("Applied AI note edits.");
        self.close_ai_overlay();
    }

    fn reject_pending_ai_edit(&mut self) {
        self.pending_ai_edit = None;
        self.ghost_result = None;
        self.ghost_streaming = false;
        self.ghost_stream_rx = None;
        self.thinking = false;
        self.thinking_ticks_remaining = 0;
        self.last_action = String::from("Rejected AI note edits.");
    }

    fn build_line_diff(original: &str, proposed: &str) -> Vec<String> {
        let old_lines: Vec<&str> = original.lines().collect();
        let new_lines: Vec<&str> = proposed.lines().collect();
        let mut table = vec![vec![0usize; new_lines.len() + 1]; old_lines.len() + 1];

        for old_index in (0..old_lines.len()).rev() {
            for new_index in (0..new_lines.len()).rev() {
                table[old_index][new_index] = if old_lines[old_index] == new_lines[new_index] {
                    table[old_index + 1][new_index + 1] + 1
                } else {
                    table[old_index + 1][new_index].max(table[old_index][new_index + 1])
                };
            }
        }

        let mut diff = Vec::new();
        let mut old_index = 0;
        let mut new_index = 0;
        while old_index < old_lines.len() && new_index < new_lines.len() {
            if old_lines[old_index] == new_lines[new_index] {
                diff.push(format!("  {}", old_lines[old_index]));
                old_index += 1;
                new_index += 1;
            } else if table[old_index + 1][new_index] >= table[old_index][new_index + 1] {
                diff.push(format!("- {}", old_lines[old_index]));
                old_index += 1;
            } else {
                diff.push(format!("+ {}", new_lines[new_index]));
                new_index += 1;
            }
        }
        while old_index < old_lines.len() {
            diff.push(format!("- {}", old_lines[old_index]));
            old_index += 1;
        }
        while new_index < new_lines.len() {
            diff.push(format!("+ {}", new_lines[new_index]));
            new_index += 1;
        }

        if diff.is_empty() && !proposed.is_empty() {
            diff.push(format!("+ {}", proposed));
        }
        diff
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

    fn ctrl(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::CONTROL,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
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

        app.open_note_list_panel();

        assert_eq!(app.panel_lines.len(), 1);
        assert!(app.panel_lines[0].contains("[研究ノート集]"));
    }

    #[test]
    fn settings_round_trip_to_config() {
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

        assert_eq!(App::load_note_save_target(), Some(NoteSaveTarget::Obsidian));
        assert_eq!(App::load_ai_provider(), Some(AiProvider::Strix));
        assert_eq!(App::load_agent_mode_enabled(), Some(false));

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
        static ENV_LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
        let _guard = ENV_LOCK
            .get_or_init(|| std::sync::Mutex::new(()))
            .lock()
            .unwrap();
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
        let original_count = app.notes.len();

        app.open_note_list_panel();
        app.handle_note_list_key(press(KeyCode::Delete));
        app.handle_note_list_key(press(KeyCode::Down));

        assert_eq!(app.notes.len(), original_count);
        assert!(!app.note_list_delete_is_pending());
    }

    #[test]
    fn note_list_delete_removes_obsidian_file() {
        let root =
            std::env::temp_dir().join(format!("aleph-note-delete-test-{}", App::now_millis()));
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
            diff_lines: App::build_line_diff(
                "",
                "# Launch Planning\n\nShip the smallest useful path.",
            ),
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
}
