# Aleph

## Base TUI Scaffold

Run the first visible shell with:

```bash
cargo run
```

The current codebase is a Ratatui terminal client with local notes, Obsidian pairing, Strix sync, and AI chat/editing paths in progress.

## Obsidian Pairing

Aleph pairs with Obsidian vaults directly through the local Markdown filesystem instead of depending on Obsidian CLI. The Obsidian desktop config is used only to discover vaults, and `obsidian://` URIs are used when you ask Aleph to open a vault or note in Obsidian.

```bash
cargo run -- obsidian vaults
cargo run -- obsidian pair              # auto-pairs if exactly one vault is found
cargo run -- obsidian pair ~/Notes/Vault
cargo run -- obsidian sync
cargo run -- obsidian open "Daily note"
```

Inside the TUI, use `/obsidian pair`, pick a detected vault, then run `/obsidian sync`. Imported Obsidian notes show up in `/note list`, can be searched with `/search`, and edits to imported Markdown notes are written back to the vault. New `/note create` notes are also created as Markdown files when a vault is paired.

## Strix And AI

Use `/login strix` to connect a Strix account. When Strix is connected, `/sync` pulls Strix notes and note writes can be pushed back to Strix. Use `/login openrouter <key>` to configure OpenRouter as an optional model provider; OpenRouter is not Aleph's app login identity.

Use `/settings` to inspect the current Strix status, selected model provider, Obsidian vault, sync targets, credential storage, editor mode, and useful environment overrides.

Useful overrides:

- Pair Obsidian explicitly with `/obsidian pair`; Aleph no longer auto-pairs from environment variables after reset.
- `ALEPH_CONFIG_DIR=/path/to/config` changes the file fallback used when keychain storage is unavailable.
- `OBSIDIAN_CONFIG_PATH=/path/to/obsidian.json` points discovery at a custom desktop config.
