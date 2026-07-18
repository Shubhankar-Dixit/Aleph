# Aleph TUI Notes

- `src/app.rs` and `src/ui.rs` now own the local command surface, note read/write/edit flows, and the embedded bottom-pane note editor.
- Keep `COMMANDS` aligned with `aleph_idea.md` and `strix-agents.md` when adding, renaming, or removing commands.
- The UI language of Aleph leans into an "aesthetic anime inspired" design logic. Temporal Forks (a.k.a. "worlds" or "paths") are visualized as a neon/pastel Tokyo metro-map layout with parallel tracks and sleek line drawing elements rather than plain JSON/text lists. When updating panels, use rich `ratatui` UI paradigms (color palettes, bold styles) over plain `panel_lines`.

## Agentic Chat Direction

- Aleph agent mode should feel like a local coding/computer-use agent, not a chatbot with occasional tool calls.
- Agent runs should follow a compact loop: plan, act, observe, synthesize, then either continue or ask permission.
- Do not dump raw tool output into the chat transcript. Show short progress lines in chat, keep larger observations in context for synthesis.
- Prefer one provider synthesis call after local context gathering instead of repeated planning/reflection calls.
- Read-only tools can run immediately. Write actions must stay explicit and approval-gated.
- Local advantages should be used first: notes, memories, rooms, repo status, Trail events, Obsidian paths, and provider/runtime status.
- When adding agent tools, give them typed inputs/outputs and concise summaries so they can compose in a loop.
