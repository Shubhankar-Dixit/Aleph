# Aleph TUI Notes

- `src/app.rs` and `src/ui.rs` now own the local command surface, note read/write/edit flows, and the embedded bottom-pane note editor.
- Keep `COMMANDS` aligned with `aleph_idea.md` and `strix-agents.md` when adding, renaming, or removing commands.

## Build & Test

- `cargo build` — compile (CI runs `cargo build --verbose`).
- `cargo test` — run unit tests (CI runs `cargo test --verbose`).
- `cargo run` — launch the TUI.
- `cargo run -- <area> <action>` — run a CLI subcommand without entering the TUI.
- CI workflows: `.github/workflows/rust.yml` (build + test on push/PR to `main`), `.github/workflows/release.yml` (cargo-dist release on version tags, publishes Homebrew formula to `Shubhankar-Dixit/homebrew-tap`).

## CLI Subcommands

`run_cli_command` in `src/app/core_accessors.rs` dispatches non-TUI usage:

- `aleph notes list|search <query>|read <id>|write <id> <content>|append <id> <content>|create <title> <content>`
- `aleph room list|show [name]|use <name>|next|prev`
- `aleph obsidian pair [path]|vaults|sync|status|open [target]`
- `aleph trail show|search <query>`
- `aleph daemon start|run|status|stop`
- `aleph sync` — pull Strix notes.
- Pass `-` as a content arg to read from stdin (`expand_stdin_args` in `src/main.rs`).

## Module Layout

- `src/app/` — command dispatch (`commands.rs`, `commands_notes.rs`), notes editor, obsidian, strix, rooms, trail, temporal forks, agent routing, AI edit, auth/chat, editor input, model types, core accessors, tests.
- `src/ui/` — chat settings, diff view, editor, image preview, markdown rendering, panels, tests.

## Environment Variables

Config and cache paths can be overridden for testing and isolation:

- `ALEPH_CONFIG_DIR` — base dir for config files (rooms, settings, forks, obsidian pairing).
- `ALEPH_CACHE_DIR` — base dir for Strix note cache.
- `ALEPH_NOTES_PATH` — local notes JSON path.
- `ALEPH_MEMORIES_PATH` — local memories JSON path.
- `ALEPH_STRIX_CACHE` — Strix cached notes JSON path.
- `ALEPH_FORKS_PATH` — temporal fork snapshots JSON path.
- `ALEPH_ROOMS_PATH` — rooms config JSON path.
- `ALEPH_TRAIL_PATH` — Trail event log path.
- `ALEPH_DAEMON_STATE_PATH` — Trail daemon state JSON path.
- `OBSIDIAN_CONFIG_PATH` — custom Obsidian desktop config (`obsidian.json`) path.

## Testing Conventions

- Tests live in `src/app/tests.rs` and `src/ui/tests.rs`.
- Use `env_lock()` (OnceLock Mutex) when tests set environment variables to avoid cross-test interference.
- Key events are constructed with `press()`, `repeat()`, and `ctrl()` helpers in `src/app/tests.rs`.
- `clear-notes` is a hidden command (not in `COMMANDS`) that resets note state and caches; tested explicitly.

## Temporal Forks

- Git-like snapshot system in `src/app/temporal_forks.rs`; snapshots notes, folders, memories, selected note, and repo context.
- Auto-snapshots before note append, create, delete, save, move; folder create/delete; memory save; AI edit apply.
- TUI commands: `/path save|list|show|return`. `world` and `fork` are aliases (e.g., `/world save`, `/fork checkout`).
