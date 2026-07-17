# Aleph TUI Notes

## Module layout

- The `App` struct lives in `src/app.rs`; its behavior is split across `src/app/*` modules. `src/ui.rs` plus `src/ui/*` own rendering.
- The local command surface is `COMMANDS` in `src/app/commands.rs` (re-exported from `src/app.rs`); `CommandSpec` is defined in `src/app/model.rs`.
- TUI `/`-command dispatch and note read/write/edit flows live in `src/app/commands_notes.rs`; the bottom-pane note editor lives in `src/app/notes_editor.rs` + `src/app/editor_input.rs`.
- CLI dispatch (`aleph <area> ...`) is `run_cli_command` in `src/app/core_accessors.rs`, which fans out to `run_room_cli_command`, `run_obsidian_cli_command`, `run_trail_cli_command`, and `run_daemon_cli_command`.
- Keep `COMMANDS` in `src/app/commands.rs` as the source of truth when adding, renaming, or removing commands.
  - TODO: `aleph_idea.md` and `strix-agents.md` are referenced for command alignment but are not tracked in this repo (listed in `Cargo.toml` `exclude`); treat them as local-only and don't depend on them in a fresh clone.

## Build & test

- `cargo build` — compile the `strix-aleph` binary (CI uses `cargo build --verbose`).
- `cargo test` — run the unit tests in `src/app/tests.rs` (CI uses `cargo test --verbose`).
- No clippy/fmt step is configured in CI (`.github/workflows/rust.yml`); releases are driven by `cargo-dist` via `dist-workspace.toml`.

## CLI surface (non-TUI)

Run without the TUI by passing a non-`tui` first arg; `main.rs` routes to `App::run_cli_command`. A `-` arg is replaced with stdin contents (`expand_stdin_args`):

- `aleph notes list|search <query>|read <id|title>|write <id|title> <content>|append <id|title> <content>|create <title> <content>`
- `aleph room list|show <name>|use <name>|next|prev` (alias: `rooms`)
- `aleph obsidian pair [<path|number|name>]|vaults|sync|status|open [<note>]`
- `aleph trail show|list|search <query>`
- `aleph daemon start|run|status|stop`
- `aleph sync` — pull Strix notes into the current session

See `README.md` for Obsidian pairing examples and `/settings`/environment overrides.
