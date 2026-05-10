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
        name: "search",
        description: "Search notes and memories",
    },
    CommandSpec {
        name: "note",
        description: "List notes; type /note to see note actions",
    },
    CommandSpec {
        name: "memory",
        description: "List memories; type /memory to see memory actions",
    },
    CommandSpec {
        name: "room",
        description: "List rooms or switch scope (usage: /room <name> | /room all)",
    },
    CommandSpec {
        name: "rooms",
        description: "Alias for /room; list rooms or switch by name",
    },
    CommandSpec {
        name: "recall",
        description: "Show recent note activity",
    },
    CommandSpec {
        name: "trail",
        description: "Open Aleph Trail: local cognitive telemetry for this workspace",
    },
    CommandSpec {
        name: "path",
        description: "List saved thinking paths",
    },
    CommandSpec {
        name: "daemon",
        description: "Control the local Aleph Trail daemon",
    },
    CommandSpec {
        name: "settings",
        description: "Show useful connection, sync, editor, and AI settings",
    },
    CommandSpec {
        name: "folder",
        description: "List folders; type /folder to see folder actions",
    },
    CommandSpec {
        name: "obsidian",
        description: "Show Obsidian pairing status and available vault actions",
    },
    CommandSpec {
        name: "logout",
        description: "Sign out",
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
        name: "sync",
        description: "Pull notes from Strix into the current Aleph session",
    },
    CommandSpec {
        name: "ask",
        description: "Ask the selected AI provider a question",
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
        name: "serve mcp",
        description: "Start the MCP server",
    },
    CommandSpec {
        name: "room list",
        description: "Interactively list rooms; Enter switches and Delete removes",
    },
    CommandSpec {
        name: "room show",
        description: "Inspect a room's project paths, tags, filters, and recent sessions",
    },
    CommandSpec {
        name: "room use",
        description: "Switch to a room by name",
    },
    CommandSpec {
        name: "room next",
        description: "Cycle to the next room",
    },
    CommandSpec {
        name: "room prev",
        description: "Cycle to the previous room",
    },
    CommandSpec {
        name: "trail search",
        description: "Search the local workspace trail",
    },
    CommandSpec {
        name: "daemon start",
        description: "Start the local Aleph Trail daemon for this workspace",
    },
    CommandSpec {
        name: "daemon status",
        description: "Show Aleph Trail daemon state and heartbeat",
    },
    CommandSpec {
        name: "daemon stop",
        description: "Request the Aleph Trail daemon to stop",
    },
    CommandSpec {
        name: "path save",
        description: "Save this decision point so you can explore another path",
    },
    CommandSpec {
        name: "path list",
        description: "List saved paths and decision points",
    },
    CommandSpec {
        name: "path show",
        description: "Inspect a saved path by name or id",
    },
    CommandSpec {
        name: "path return",
        description: "Return Aleph to a saved path",
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
];

pub const THINKING_FRAMES: [&str; 16] = [
    "◌", "ॐ", "Ω", "Ψ", "Д", "Ж", "א", "⌘", "⚛", "ᚠ", "ᛟ", "ꙮ", "Ξ", "Δ", "Ц", "Ш",
];
