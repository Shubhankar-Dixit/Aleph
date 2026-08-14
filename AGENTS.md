# Aleph TUI Notes

- `src/app.rs` and `src/ui.rs` are the module roots; logic lives in submodules under `src/app/` (agent, agent_run, ai_edit, auth_chat, chat_composer, commands, commands_notes, core_accessors, editor_input, input, model, notes_editor, obsidian, rooms, strix, temporal_forks, tests, trail, transcript) and `src/ui/` (chat_settings, diff, editor, image_preview, markdown, panels, tests).
- Keep `COMMANDS` (in `src/app/commands.rs`) aligned with `aleph_idea.md` and `strix-agents.md` when adding, renaming, or removing commands.
- The UI language of Aleph leans into an "aesthetic anime inspired" design logic. Temporal Forks (a.k.a. "worlds" or "paths") are visualized as a neon/pastel Tokyo metro-map layout with parallel tracks and sleek line drawing elements rather than plain JSON/text lists. When updating panels, use rich `ratatui` UI paradigms (color palettes, bold styles) over plain `panel_lines`.

## Agentic Chat Direction

- Aleph agent mode should feel like a local coding/computer-use agent, not a chatbot with occasional tool calls.
- Agent runs should follow a compact loop: plan, act, observe, synthesize, then either continue or ask permission.
- Do not dump raw tool output into the chat transcript. Show short progress lines in chat, keep larger observations in context for synthesis.
- Prefer one provider synthesis call after local context gathering instead of repeated planning/reflection calls.
- Read-only tools can run immediately. Write actions must stay explicit and approval-gated.
- Local advantages should be used first: notes, memories, rooms, repo status, Trail events, Obsidian paths, and provider/runtime status.
- When adding agent tools, give them typed inputs/outputs and concise summaries so they can compose in a loop.

## Build & Test

- `cargo run` — launch the TUI (or `cargo run -- tui`).
- `cargo build --verbose` / `cargo test --verbose` — CI build and test commands (matches `.agents/workflows/rust.yml`).
- Rust edition 2021, MSRV 1.80 (see `Cargo.toml`).

## CLI Subcommands

`aleph <area> …` (also via `cargo run -- <area> …`). Areas are dispatched in `run_cli_command` (`src/app/core_accessors.rs`).

- `aleph notes [list]` / `aleph notes search <query>` / `aleph notes read <id>` / `aleph notes write <id> -` (stdin content)
- `aleph room [list]` / `aleph room show [name]` / `aleph room use <name>` / `aleph room next` / `aleph room prev` / `aleph room all`
- `aleph obsidian [status]` / `aleph obsidian vaults` / `aleph obsidian pair [path]` / `aleph obsidian sync` / `aleph obsidian open <title>`
- `aleph path [list]` / `aleph path save <label>` / `aleph path show <name>` / `aleph path return <name>`
- `aleph trail [show]` / `aleph trail search <query>`
- `aleph daemon [status]` / `aleph daemon start` / `aleph daemon run` / `aleph daemon stop`
- `aleph sync` — pull Strix notes into the current session

## Environment Variables

| Variable | Purpose |
|---|---|
| `ALEPH_CONFIG_DIR` | Config directory used when keychain storage is unavailable |
| `ALEPH_CACHE_DIR` | Cache directory root |
| `ALEPH_NOTES_PATH` | Local notes JSON file path |
| `ALEPH_MEMORIES_PATH` | Local memories file path |
| `ALEPH_FORKS_PATH` | Temporal forks JSON file path |
| `ALEPH_TRAIL_PATH` | Trail JSONL file path |
| `ALEPH_DAEMON_STATE_PATH` | Daemon state file path |
| `ALEPH_STRIX_CACHE` | Strix sync cache JSON path |
| `ALEPH_ROOMS_PATH` | Rooms state JSON path |
| `OBSIDIAN_CONFIG_PATH` | Custom Obsidian desktop config path for vault discovery |
| `STRIX_API_BASE_URL` | Override Strix API base URL |
| `STRIX_AUTH_BASE_URL` | Override Strix auth base URL (localhost URLs fall back to production) |
| `STRIX_LOCAL_AUTH_BASE_URL` | Explicit local dev Strix auth base URL (localhost URLs allowed) |

## Agent Run Lifecycle

Run phases (`RunPhase` in `src/app/model.rs`): `Planning → Acting → WaitingApproval → Streaming → Completed | Failed | Cancelled`. Transitions are guarded by `can_transition_to`.

Agent actions (`AgentAction`): `Chat`, `CreateNote`, `EditNote`, `ReadNote`, `SearchNotes`, `SaveMemory`, `ListMemories`, `SearchMemories`, `WorkspaceStatus`, `SearchTrail`.

- Read-only actions (`ReadNote`, `SearchNotes`, `ListMemories`, `SearchMemories`, `WorkspaceStatus`, `SearchTrail`) run immediately through the progressive step loop.
- Write actions (`CreateNote`, `EditNote`, `SaveMemory`) are staged and require explicit Enter approval before applying.
- Run steps cycle `Pending → Running → Completed | Failed` (`StepStatus`).

## Testing Patterns

- Tests live in `src/app/tests.rs` and `src/ui/tests.rs`; run with `cargo test`.
- `env_lock()` serializes tests that mutate environment variables.
- `advance_execution(app, n)` drives the async agent execution loop in tests.
- Tests isolate filesystem state by setting `ALEPH_*_PATH` env vars to temp directories and cleaning up afterward.
- `#[cfg(test)]` gates test-only behavior (e.g., the agent workspace worker returns `Ok(None)` under test).

## CI & Release

- `.agents/workflows/rust.yml` — builds and tests on push/PR to `main`.
- `.agents/workflows/release.yml` — cargo-dist release workflow.
- `dist-workspace.toml` — cross-platform release config (shell, PowerShell, npm, Homebrew installers).
