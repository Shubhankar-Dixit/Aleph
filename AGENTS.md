# Aleph TUI Notes

- `src/app.rs` and `src/ui.rs` now own the local command surface, note read/write/edit flows, and the embedded bottom-pane note editor.
- Keep `COMMANDS` aligned with `aleph_idea.md` and `strix-agents.md` when adding, renaming, or removing commands.

## Workflows

- Run the TUI with `cargo run` or `cargo run -- tui`.
- Run one-shot CLI commands with `cargo run -- <command>`; for example, `cargo run -- obsidian vaults` or `cargo run -- obsidian sync`.
- Run the Rust test suite with `cargo test` from the repository root after a Rust 1.80+ toolchain is configured.
