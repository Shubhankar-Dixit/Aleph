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
pub struct Room {
    pub name: String,
    pub project_paths: Vec<String>,
    pub tags: Vec<String>,
    pub filters: Vec<String>,
    pub accent: [u8; 3],
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
    pub strix_sync_pending: bool,
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
    RoomList,
    VaultPicker,
    Settings,
    ObsidianSyncConfirm,
    PathList,
}

#[derive(Clone)]
pub struct ChatMessage {
    pub role: String, // "user" or "assistant"
    pub content: String,
    pub timestamp: String,
    /// Seconds from turn start until the first streamed token arrived.
    pub thought_seconds: Option<f32>,
    /// Total seconds the turn took, filled in when the stream completes.
    pub turn_seconds: Option<f32>,
    /// Agent run that owns this message. Older/system messages may have none.
    pub run_id: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunPhase {
    Planning,
    Acting,
    WaitingApproval,
    Streaming,
    Completed,
    Failed,
    Cancelled,
}

impl RunPhase {
    pub fn can_transition_to(self, next: Self) -> bool {
        use RunPhase::*;
        match self {
            Planning => matches!(
                next,
                Acting | WaitingApproval | Streaming | Completed | Failed | Cancelled
            ),
            Acting => matches!(
                next,
                WaitingApproval | Streaming | Completed | Failed | Cancelled
            ),
            WaitingApproval => matches!(next, Acting | Streaming | Completed | Failed | Cancelled),
            Streaming => matches!(next, Completed | Failed | Cancelled),
            Completed | Failed | Cancelled => false,
        }
    }

    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RepositoryContextSource {
    Live,
    Snapshot,
    Unavailable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunContextSnapshot {
    pub scope: String,
    pub room: String,
    pub room_summary: String,
    pub selected_note: Option<String>,
    pub relevant_notes: Vec<String>,
    pub relevant_memories: usize,
    pub relevant_trail_events: usize,
    pub notes_available: usize,
    pub memories_available: usize,
    pub local_tools_available: bool,
    pub provider: String,
    pub provider_online: bool,
    pub repository_source: RepositoryContextSource,
    pub repository_summary: Option<String>,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepStatus {
    Pending,
    Running,
    Completed,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunStep {
    pub label: String,
    pub target: Option<String>,
    pub status: StepStatus,
    pub summary: Option<String>,
    pub error: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApprovalRequest {
    pub operation: String,
    pub target: String,
    pub effect: String,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChangeStatus {
    Proposed,
    Applied,
    Rejected,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunChange {
    pub target: String,
    pub summary: String,
    pub status: ChangeStatus,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RunOutcome {
    Completed { summary: String },
    Failed { error: String },
    Cancelled { reason: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentRun {
    pub id: u64,
    pub request: String,
    pub phase: RunPhase,
    pub context: RunContextSnapshot,
    pub steps: Vec<RunStep>,
    pub approval: Option<ApprovalRequest>,
    pub changes: Vec<RunChange>,
    pub outcome: Option<RunOutcome>,
}

#[derive(Clone)]
pub struct ActivityEntry {
    pub timestamp: String,
    pub label: String,
}

#[derive(Clone)]
pub struct RepoContext {
    pub cwd: String,
    pub branch: Option<String>,
    pub head: Option<String>,
    pub dirty_files: Vec<String>,
}

#[derive(Clone)]
pub struct TemporalFork {
    pub id: String,
    pub parent_id: Option<String>,
    pub label: String,
    pub reason: String,
    pub created_at: String,
    pub notes: Vec<Note>,
    pub folders: Vec<Folder>,
    pub memories: Vec<String>,
    pub selected_note: usize,
    pub activity_context: Vec<ActivityEntry>,
    pub chat_context: Vec<ChatMessage>,
    pub repo_context: Option<RepoContext>,
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

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AgentContextScope {
    CurrentFolder,
    ActiveRoom,
    Global,
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

#[derive(Clone, Default)]
pub struct SearchState {
    pub query: String,
    pub matches: Vec<usize>,
    pub current_match: Option<usize>,
    pub active: bool,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Selection {
    pub start: usize,
    pub end: usize,
    pub active: bool,
}

impl Selection {
    pub fn clear(&mut self) {
        self.active = false;
        self.start = 0;
        self.end = 0;
    }

    pub fn select_all(&mut self, len: usize) {
        self.start = 0;
        self.end = len;
        self.active = len > 0;
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum EditorSaveStatus {
    #[default]
    Clean,
    Unsaved,
    Saved,
    Failed(String),
}
