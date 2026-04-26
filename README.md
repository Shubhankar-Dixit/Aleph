# Aleph

Aleph is a local-first command line bridge for pairing your knowledge base with Obsidian vaults.

The Obsidian CLI is useful for asking a running Obsidian app to open vaults and files, but Aleph does not need Obsidian to be running to understand a vault. It reads the vault directly, builds an Aleph manifest, and only uses Obsidian's URI protocol when you explicitly want to open a note in the app.

## Install locally

```bash
npm install
npm link
```

You can also run the CLI without linking:

```bash
node ./bin/aleph.mjs obsidian pair
```

## Pair an Obsidian vault

Run one command and pick the vault Aleph should use:

```bash
aleph obsidian pair
```

If your vault is not registered in Obsidian's app config, point Aleph at it directly:

```bash
aleph obsidian pair ~/Documents/MyVault
```

To search common folders for vaults before showing the picker:

```bash
aleph obsidian pair --scan
```

Pairing writes Aleph's config to your OS config directory and creates a local manifest at:

```text
<vault>/.aleph/obsidian-manifest.json
```

The manifest contains note paths, titles, tags, wiki links, backlinks, task lists, word counts, and frontmatter so Aleph can reason over the vault without depending on Obsidian CLI.

## Commands

```bash
aleph obsidian vaults
aleph obsidian status
aleph obsidian sync
aleph obsidian search "project roadmap"
aleph obsidian capture "Follow up with the design partner" --title "Design partner follow-up"
aleph obsidian open "Projects/Aleph.md"
```

Use `--vault <name|id|path>` with `status`, `sync`, `search`, `capture`, or `open` when more than one vault is paired.

## Why this path instead of wrapping Obsidian CLI?

- Works even when Obsidian is closed.
- Produces structured data Aleph can consume directly.
- Avoids coupling Aleph automation to the installed Obsidian app version.
- Keeps note creation and sync local to the vault filesystem.
- Still supports Obsidian-native opening through `obsidian://open`.

## Configuration

Aleph reads these optional environment variables:

- `ALEPH_HOME` sets the directory used for Aleph config.
- `ALEPH_CONFIG` sets the exact config file path.
- `OBSIDIAN_CONFIG` points to an Obsidian `obsidian.json` file when auto-discovery needs help.

## Development

```bash
npm test
npm run lint
```
