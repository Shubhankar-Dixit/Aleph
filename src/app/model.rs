use std::path::PathBuf;

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
    pub obsidian_path: Option<PathBuf>,
    pub title: String,
    pub content: String,
    pub raw_content: String,
    pub updated_at: String,
    pub folder_id: Option<usize>,
}

#[derive(Clone)]
pub struct ObsidianVault {
    pub id: String,
    pub name: String,
    pub path: PathBuf,
    pub source: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PanelMode {
    Commands,
    NoteEditor,
    FullEditor,
    AiChat,
    LoginPicker,
    NoteList,
    VaultPicker,
    Settings,
}

#[derive(Clone)]
pub struct ChatMessage {
    pub role: String, // "user" or "assistant"
    pub content: String,
    pub timestamp: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum CursorStyle {
    Block,
    Line,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NoteSaveTarget {
    Local,
    Obsidian,
    Strix,
}

#[derive(Clone)]
pub struct EditorState {
    pub buffer: String,
    pub cursor: usize,
    pub scroll_offset: usize,
}

#[derive(Clone)]
pub struct AiEditProposal {
    pub note_index: Option<usize>,
    pub title: Option<String>,
    pub instruction: String,
    pub proposed: String,
    pub diff_lines: Vec<String>,
}

#[derive(Clone)]
pub struct SearchState {
    pub query: String,
    pub matches: Vec<usize>,
    pub current_match: Option<usize>,
    pub active: bool,
}
