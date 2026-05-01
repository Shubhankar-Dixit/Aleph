use super::model::CommandSpec;

pub const COMMANDS: &[CommandSpec] = &[
    CommandSpec {
        name: "login",
        description: "Connect Strix or configure a model provider (usage: /login strix | /login openrouter <key>)",
    },
    CommandSpec {
        name: "status",
        description: "Show session, note, and runtime health",
    },
    CommandSpec {
        name: "sync",
        description: "Pull notes from Strix into the current Aleph session",
    },
    CommandSpec {
        name: "doctor",
        description: "Run local diagnostics",
    },
    CommandSpec {
        name: "config",
        description: "Inspect local runtime configuration",
    },
    CommandSpec {
        name: "settings",
        description: "Show useful connection, sync, editor, and AI settings",
    },
    CommandSpec {
        name: "mode agent",
        description: "Use Codex-style agent routing for note actions",
    },
    CommandSpec {
        name: "mode chat",
        description: "Use plain chat responses without taking note actions",
    },
    CommandSpec {
        name: "logout",
        description: "Sign out",
    },
    CommandSpec {
        name: "obsidian pair",
        description: "Pair a local Obsidian vault (usage: /obsidian pair | /obsidian pair <path|number|name>)",
    },
    CommandSpec {
        name: "obsidian vaults",
        description: "List detected Obsidian vaults",
    },
    CommandSpec {
        name: "obsidian sync",
        description: "Import Markdown notes from the paired Obsidian vault",
    },
    CommandSpec {
        name: "obsidian status",
        description: "Show the paired Obsidian vault and discovery config",
    },
    CommandSpec {
        name: "obsidian open",
        description: "Open the paired vault or selected note in Obsidian",
    },
    CommandSpec {
        name: "search",
        description: "Search notes and memories",
    },
    CommandSpec {
        name: "recall",
        description: "Show recent note activity",
    },
    CommandSpec {
        name: "ask",
        description: "Ask the selected AI provider a question",
    },
    CommandSpec {
        name: "agent edit",
        description: "Natural-language note edits use the AI editor, show a diff, and require approval",
    },
    CommandSpec {
        name: "note list",
        description: "List local notes",
    },
    CommandSpec {
        name: "note read",
        description: "Read a note by id, index, or title",
    },
    CommandSpec {
        name: "note create",
        description: "Create a note and open the editor (usage: /note create <title> :: <body>)",
    },
    CommandSpec {
        name: "note append",
        description: "Append text (usage: /note append <text> | /note append <note> :: <text>)",
    },
    CommandSpec {
        name: "note edit",
        description: "Edit the selected note in the bottom pane",
    },
    CommandSpec {
        name: "note move",
        description: "Move a note to a folder",
    },
    CommandSpec {
        name: "folder list",
        description: "List all folders",
    },
    CommandSpec {
        name: "folder create",
        description: "Create a new folder",
    },
    CommandSpec {
        name: "folder delete",
        description: "Delete a folder",
    },
    CommandSpec {
        name: "folder notes",
        description: "List notes in a folder",
    },
    CommandSpec {
        name: "folder tree",
        description: "Show folder hierarchy",
    },
    CommandSpec {
        name: "memory list",
        description: "List local memories",
    },
    CommandSpec {
        name: "memory save",
        description: "Save a local memory",
    },
    CommandSpec {
        name: "memory search",
        description: "Search stored memories",
    },
    CommandSpec {
        name: "serve mcp",
        description: "Start the MCP server",
    },
];

pub const THINKING_FRAMES: [&str; 16] = [
    "◌", "ॐ", "Ω", "Ψ", "Д", "Ж", "א", "⌘", "⚛", "ᚠ", "ᛟ", "ꙮ", "Ξ", "Δ", "Ц", "Ш",
];
