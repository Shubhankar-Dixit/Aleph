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
use crossterm::event::{
    KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use keyring::Entry;
use rand::{rngs::OsRng, RngCore};
use ratatui::prelude::{Color, Line, Modifier, Span, Style};
use reqwest::blocking::Client;
use sha2::{Digest, Sha256};

mod agent;
mod ai_edit;
mod auth_chat;
mod commands;
mod commands_notes;
mod core_accessors;
mod editor_input;
mod input;
pub mod model;
mod notes_editor;
mod obsidian;
mod strix;
mod temporal_forks;

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
const EDITOR_IMAGES_CONFIG: &str = "editor-images";
const OBSIDIAN_PAIRING_DISABLED_CONFIG: &str = "obsidian-vault-disabled";
const STRIX_TOKEN_CONFIG: &str = "strix-access-token";
const MAX_CHAT_MESSAGES: usize = 24;
const CHAT_TEXT: Color = Color::Rgb(142, 144, 158);
const CHAT_MUTED: Color = Color::Rgb(104, 107, 122);
const CHAT_ACCENT: Color = Color::Rgb(136, 129, 176);
const CHAT_ACCENT_SOFT: Color = Color::Rgb(112, 108, 148);

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
    ReadNote,
    SearchNotes,
    ListMemories,
    SearchMemories,
}

struct AgentDecision {
    action: AgentAction,
    note_index: Option<usize>,
    title: Option<String>,
    search_query: Option<String>,
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
    editor_images_enabled: bool,
    editor_cursor_style: CursorStyle,
    editor_selection: Selection,
    undo_stack: VecDeque<EditorState>,
    redo_stack: VecDeque<EditorState>,
    search_state: SearchState,
    chat_messages: Vec<ChatMessage>,
    activity_log: VecDeque<ActivityEntry>,
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
    thinking_status: String,
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
    expanded_folders: Vec<usize>,
    temporal_forks: Vec<TemporalFork>,
    current_fork_id: Option<String>,
}

#[cfg(test)]
mod tests;
