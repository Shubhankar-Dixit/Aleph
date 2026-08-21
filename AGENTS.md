# Aleph TUI Notes

- `src/app.rs` and `src/ui.rs` are the module roots; logic lives in submodules under `src/app/` (agent, ai_edit, auth_chat, commands, commands_notes, core_accessors, editor_input, input, model, notes_editor, obsidian, rooms, strix, temporal_forks, tests, trail) and `src/ui/` (chat_settings, diff, editor, image_preview, markdown, panels, tests).
- Keep `COMMANDS` (in `src/app/commands.rs`) aligned with `aleph_idea.md` and `strix-agents.md` when adding, renaming, or removing commands.
- The UI leans into an "aesthetic anime inspired" design. Temporal Forks (a.k.a. "worlds" or "paths") are visualized as a neon/pastel Tokyo metro-map layout with parallel tracks rather than plain JSON/text lists. Prefer rich `ratatui` paradigms (color palettes, bold styles) over plain `panel_lines`.

## Agentic Chat Direction

- Aleph agent mode should feel like a local coding/computer-use agent, not a chatbot with occasional tool calls.
- Agent runs follow a compact loop: plan, act, observe, synthesize, then either continue or ask permission.
- Do not dump raw tool output into the chat transcript. Show short progress lines in chat; keep larger observations in context for synthesis.
- Prefer one provider synthesis call after local context gathering instead of repeated planning/reflection calls.
- Read-only tools can run immediately. Write actions must stay explicit and approval-gated.
- Local advantages should be used first: notes, memories, rooms, repo status, Trail events, Obsidian paths, and provider/runtime status.
- When adding agent tools, give them typed inputs/outputs and concise summaries so they compose in a loop.

## Build & Test

- `cargo run` — launch the TUI (or `cargo run -- tui`).
- `cargo build --verbose` / `cargo test --verbose` — CI build and test commands (matches `.github/workflows/rust.yml`).
- Rust edition 2021, MSRV 1.80 (see `Cargo.toml`). Package name `strix-aleph`, binary `aleph`.

## CLI Subcommands

`aleph <area> …` (also via `cargo run -- <area> …`). Dispatched in `run_cli_command` (`src/app/core_accessors.rs`); `trail`/`daemon` helpers live in `src/app/trail.rs`.

- `aleph notes [list]` / `aleph notes search <query>` / `aleph notes read <id>` / `aleph notes write <id> <content|->` / `aleph notes append <id> <content|->` / `aleph notes create [title] [content]` (`-` reads content from stdin)
- `aleph room [list]` / `aleph room show [name]` / `aleph room use <name>` / `aleph room next` / `aleph room prev` (a bare `aleph room <name>` also switches)
- `aleph obsidian [status]` / `aleph obsidian pair [path|number|name]` / `aleph obsidian vaults` (or `list`) / `aleph obsidian sync` / `aleph obsidian open [title]`
- `aleph trail [show]` / `aleph trail search <query>`
- `aleph daemon [status]` / `aleph daemon start` / `aleph daemon run` / `aleph daemon stop`
- `aleph sync` — pull Strix notes into the current session

Note: `path` is a TUI slash command (`/path`), not a CLI subcommand.

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
| `STRIX_AUTH_BASE_URL` | Override Strix auth base URL |
| `STRIX_ACCESS_TOKEN` | Fallback Strix access token when keychain storage is unavailable |

## Agent Actions

`AgentAction` (in `src/app.rs`) and `AgentDecision` drive the agent loop in `src/app/agent.rs`.

- Actions: `Chat`, `CreateNote`, `EditNote`, `ReadNote`, `SearchNotes`, `SaveMemory`, `ListMemories`, `SearchMemories`, `WorkspaceStatus`, `SearchTrail`.
- Read-only actions (`ReadNote`, `SearchNotes`, `ListMemories`, `SearchMemories`, `WorkspaceStatus`, `SearchTrail`) run immediately through the agent loop.
- Write actions (`CreateNote`, `EditNote`, `SaveMemory`) are staged and require explicit Enter approval before applying.
- Planning has a local fast path (`plan_agent_action_locally`) and a provider path (`plan_agent_action_with_provider`).

<!-- TODO: Document `RunPhase`/`StepStatus` agent run lifecycle once those types land on main (currently only on feature branches). -->

## TUI Commands

Slash commands are defined in `COMMANDS` (`src/app/commands.rs`) and handled in `src/app/commands_notes.rs`. Notable entries: `/login`, `/status`, `/search`, `/note` (and `/note list|read|create|append|edit|move`), `/memory` (and `/memory list|save|search`), `/room` (and `/room list|show|use|next|prev`), `/folder` (and `/folder list|create|delete|notes|tree`), `/obsidian` (and `/obsidian pair|vaults|sync|status|open`), `/path` (and `/path save|list|show|return`), `/trail` (and `/trail search`), `/daemon` (and `/daemon start|status|stop`), `/sync`, `/ask`, `/mode agent`, `/mode chat`, `/workspace`, `/settings`, `/doctor`, `/config`, `/logout`. `/serve mcp` is a stub in this build.

## Testing Patterns

- Tests live in `src/app/tests.rs` and `src/ui/tests.rs`; run with `cargo test`.
- `env_lock()` serializes tests that mutate environment variables.
- `test_note()` / `seed_test_notes()` helpers build isolated note fixtures.
- Tests isolate filesystem state by setting `ALEPH_*_PATH` env vars to temp directories and cleaning up afterward.
- `#[cfg(test)]` gates test-only behavior (e.g., `App::new()` skips keychain/network calls under test).

## CI & Release

- `.github/workflows/rust.yml` — builds and tests (`cargo build --verbose`, `cargo test --verbose`) on push/PR to `main`.
- `.github/workflows/release.yml` — auto-generated cargo-dist release workflow, triggered by version tags; publishes shell, powershell, npm (`@strixlabs`), and homebrew (`Shubhankar-Dixit/homebrew-tap`) installers. Config in `dist-workspace.toml`.
