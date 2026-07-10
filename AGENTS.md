# Aleph TUI Notes

## Module Ownership

- `src/app.rs` — `App` struct, constants, module re-exports, `ChatStreamUpdate` / `AgentAction` / `AgentDecision` types.
- `src/ui.rs` — top-level `draw()`; delegates to `src/ui/` submodules.
- `src/app/commands.rs` — `COMMANDS` list and `THINKING_FRAMES`. Keep aligned with `aleph_idea.md` and `strix-agents.md` when adding, renaming, or removing commands.
- `src/app/commands_notes.rs` — `execute_command()` (TUI command dispatch), `parse_command()`, `clear_notes_state()`, note-list panel builder.
- `src/app/input.rs` — key/mouse handling, prompt normalization, `expand_command_alias()`, `command_has_subcommands()`.
- `src/app/core_accessors.rs` — `run_cli_command()` (headless CLI dispatch), settings panel, path helpers, config persistence.
- `src/app/agent.rs` — agent planner (`plan_agent_action`), agent loop, read/search/save-memory workspace actions, ranked note & memory search.
- `src/app/ai_edit.rs` — AI-powered note editing (ghost/diff overlay).
- `src/app/auth_chat.rs` — OpenRouter & Strix auth flows, chat streaming.
- `src/app/rooms.rs` — room CRUD, switching, cycling.
- `src/app/strix.rs` — Strix API client, sync, note CRUD over HTTP, credential storage, `reset_and_clear_all`.
- `src/app/obsidian.rs` — Obsidian vault discovery, pairing, sync, `obsidian://` URI open, config persistence helpers.
- `src/app/trail.rs` — local cognitive telemetry (append/read/search trail events), daemon lifecycle.
- `src/app/temporal_forks.rs` — decision-point snapshots (save/list/show/checkout forks).
- `src/app/notes_editor.rs` / `src/app/editor_input.rs` — bottom-pane note editor, undo/redo, selection, settings panel key handling.
- `src/app/model.rs` — public data types (`Note`, `Folder`, `Room`, `CommandSpec`, `PanelMode`, `AiProvider`, `NoteSaveTarget`, etc.).

## Build & Run

```bash
cargo run                              # launch TUI
cargo run -- <cli-subcommand> ...      # headless CLI mode
cargo build --verbose                  # CI build
cargo test --verbose                   # CI test (uses env-lock mutex for isolation)
```

Rust edition 2021, MSRV 1.80, package name `strix-aleph`.

## CLI Subcommands (headless)

```
aleph notes list                                    # tab-separated: source, title, preview
aleph notes search <query>
aleph notes read <id|title>
aleph notes write <id|title> <content>              # '-' reads content from stdin
aleph notes append <id|title> <content>
aleph notes create [title] [content]                # creates on Strix; also writes to Obsidian if paired
aleph room list
aleph room show [name]                              # inspect a room; defaults to active room
aleph room use <name>
aleph room all                                      # switch to global "All" room
aleph room next
aleph room prev
aleph obsidian status                               # default subcommand; shows pairing and vault count
aleph obsidian vaults
aleph obsidian pair [path|number|name]
aleph obsidian sync
aleph obsidian open [target]
aleph trail [show|list|search <query>]
aleph daemon [status|start|run|stop]
aleph sync                                          # shorthand for Strix sync
```

## TUI Command Aliases

Defined in `src/app/input.rs` `expand_command_alias()`:

| Alias | Expands to |
|---|---|
| `/find` | `/search` |
| `/open` | `/note read` |
| `/read` | `/note read` |
| `/edit` | `/note edit` |
| `/new` | `/note create` |
| `/create` | `/note create` |
| `/list` `/ls` | `/note list` |
| `/path now` | `/path save` |
| `/path open` `/path read` | `/path show` |
| `/path checkout` `/path back` | `/path return` |
| `/world save` `/world now` | `/path save` |
| `/world list` | `/path list` |
| `/world show` `/world open` `/world read` | `/path show` |
| `/world return` `/world checkout` `/world back` | `/path return` |
| `/fork now` | `/path save` |
| `/fork list` | `/path list` |
| `/fork read` | `/path show` |
| `/fork checkout` | `/path return` |
| `/rooms` | `/room` |
| `/rooms list|show|use|next|prev` | `/room list|show|use|next|prev` |

## Hidden / Internal Commands

- `/clear-notes` — clears all local notes, Strix cache, and unpairs Obsidian. Not listed in `COMMANDS`; handled directly in `commands_notes.rs`.

## Settings Panel

Accessible via `/settings` or `/config`; 9 interactive options (Up/Down to navigate, Enter to apply):

| # | Action |
|---|---|
| 0 | Switch AI provider (OpenRouter ↔ Strix) |
| 1 | Cycle to next room |
| 2 | Toggle agent mode (agent/chat) |
| 3 | Cycle note save target (Local → Obsidian → Strix) |
| 4 | Toggle editor images |
| 5 | Pair Obsidian vault (opens vault picker) |
| 6 | Sign out / logout |
| 7 | Reset & clear all data (calls `reset_and_clear_all`) |
| 8 | Close settings |

## Agent Mode

- `/mode agent` enables Codex-style agent routing; `/mode chat` disables it.
- Agent planner (`plan_agent_action_locally` in `agent.rs`) classifies input into: `Chat`, `CreateNote`, `EditNote`, `ReadNote`, `SearchNotes`, `SaveMemory`, `ListMemories`, `SearchMemories`, `WorkspaceStatus`, `SearchTrail`.
- Write actions (`CreateNote`, `EditNote`) stage a pending decision and require Enter-confirmation before Aleph modifies notes.
- Read-only actions run an agent loop that gathers observations then optionally synthesizes through the connected AI provider.
- The agent can also use an AI-provider-backed planner (`plan_agent_action_with_provider`) that sends the note list and user input to the model for JSON-structured routing.

## AI Note Editing

- `/agent edit <instruction>` opens the note editor with an AI overlay that streams proposed edits from the connected provider.
- The ghost editor (`ghost_submit_instruction` in `ai_edit.rs`) sends the current note content and instruction; returns ONLY the full replacement text.
- A diff view (`build_line_diff`) is shown; Enter applies, Ctrl+R rejects.
- For new notes (`ai_draft_create_title` set), the AI drafts from scratch and creates the note on approval.

## Temporal Forks

- Auto-forks are created before destructive actions (note save/append/move, folder create/delete, memory save).
- Manual forks: `/path save`, `/fork now`, `/world save`.
- Restore: `/path return`, `/fork checkout`, `/world return`.
- Each fork captures notes, folders, memories, recent activity log, chat context, and git repo state.
- Fork data is stored in `temporal-forks.json` under the config directory.

## Environment Variables

| Variable | Purpose |
|---|---|
| `ALEPH_CONFIG_DIR` | Base config directory fallback (used by most path helpers) |
| `ALEPH_CACHE_DIR` | Cache directory (Strix cache) |
| `ALEPH_NOTES_PATH` | Override local notes file path |
| `ALEPH_STRIX_CACHE` | Override Strix cache file path |
| `ALEPH_MEMORIES_PATH` | Override memories file path |
| `ALEPH_TRAIL_PATH` | Override trail file path |
| `ALEPH_DAEMON_STATE_PATH` | Override daemon state file path |
| `ALEPH_FORKS_PATH` | Override temporal forks file path |
| `ALEPH_ROOMS_PATH` | Override rooms file path |
| `OPENROUTER_API_KEY` | Fallback OpenRouter API key when not stored in keyring |
| `OBSIDIAN_CONFIG_PATH` | Override Obsidian desktop config for vault discovery |

## CI Workflows

- **Rust** (`.github/workflows/rust.yml`): runs `cargo build --verbose && cargo test --verbose` on push/PR to `main`.
- **Release** (`.github/workflows/release.yml`): cargo-dist v0.31.0 auto-release on version tags; publishes shell/PowerShell/npm/Homebrew installers to 5 targets; Homebrew formula pushed to `Shubhankar-Dixit/homebrew-tap`.

## Testing Conventions

- Tests live in `src/app/tests.rs` and `src/ui/tests.rs` (same-module `#[cfg(test)]`).
- Tests must acquire `env_lock()` mutex before setting `ALEPH_*` env vars to avoid cross-test interference.
- Isolate with `ALEPH_CONFIG_DIR` and `ALEPH_CACHE_DIR` pointing into `temp_dir()`.
- Run: `cargo test`
