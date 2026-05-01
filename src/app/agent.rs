use super::*;

#[allow(dead_code)]
impl App {
    pub(super) fn try_start_agent_action(&mut self, query: &str) -> bool {
        let decision = self.plan_agent_action(query);
        match decision.action {
            AgentAction::CreateNote | AgentAction::EditNote => {
                self.stage_agent_action(query, decision);
                true
            }
            AgentAction::ReadNote
            | AgentAction::SearchNotes
            | AgentAction::ListMemories
            | AgentAction::SearchMemories => {
                self.run_agent_context_action(query, decision);
                true
            }
            AgentAction::Chat => false,
        }
    }

    pub(super) fn stage_agent_action(&mut self, query: &str, decision: AgentDecision) {
        self.panel_mode = PanelMode::AiChat;
        self.chat_scroll_offset = 0;
        self.push_chat_message("user", query.trim());

        if decision.action == AgentAction::EditNote && decision.note_index.is_none() {
            self.pending_agent_query = None;
            self.pending_agent_decision = None;
            self.push_chat_message(
                "assistant",
                "I think this is note work, but I need a target note. Name the note, select one with `/note list`, or ask me to create a new note.",
            );
            self.last_action = String::from("Agent needs a note target.");
            return;
        }

        let message = self.agent_permission_message(&decision);
        self.pending_agent_query = Some(query.trim().to_string());
        self.pending_agent_decision = Some(decision);
        self.push_chat_message("assistant", message);
        self.last_action = String::from("Agent action waiting for permission.");
    }

    pub(super) fn agent_permission_message(&self, decision: &AgentDecision) -> String {
        match decision.action {
            AgentAction::CreateNote => {
                let title = decision.title.as_deref().unwrap_or("AI draft");
                format!(
                    "I can create a new note titled `{}` and draft it in the editor. Press Enter to allow, type `no` to cancel, or type a different instruction.",
                    title
                )
            }
            AgentAction::EditNote => {
                let note_title = decision
                    .note_index
                    .and_then(|index| self.notes.get(index))
                    .map(|note| note.title.as_str())
                    .unwrap_or("the selected note");
                format!(
                    "I can edit `{}` using the note-writing agent. Press Enter to allow, type `no` to cancel, or type a different instruction.",
                    note_title
                )
            }
            AgentAction::ReadNote
            | AgentAction::SearchNotes
            | AgentAction::ListMemories
            | AgentAction::SearchMemories => String::new(),
            AgentAction::Chat => String::new(),
        }
    }

    pub(super) fn confirm_pending_agent_action(&mut self) -> bool {
        let Some(decision) = self.pending_agent_decision.take() else {
            return false;
        };
        let query = self.pending_agent_query.take().unwrap_or_default();
        match decision.action {
            AgentAction::CreateNote => self.start_note_create_agent(&query, decision.title),
            AgentAction::EditNote => self.start_note_edit_agent(&query, decision),
            AgentAction::ReadNote
            | AgentAction::SearchNotes
            | AgentAction::ListMemories
            | AgentAction::SearchMemories => {
                self.run_agent_context_action(&query, decision);
                true
            }
            AgentAction::Chat => false,
        }
    }

    pub(super) fn cancel_pending_agent_action(&mut self) {
        self.pending_agent_query = None;
        self.pending_agent_decision = None;
        self.push_chat_message("assistant", "Cancelled the pending note action.");
        self.last_action = String::from("Cancelled pending agent action.");
    }

    pub(super) fn is_affirmative_agent_permission(input: &str) -> bool {
        matches!(
            input.trim().to_lowercase().as_str(),
            "y" | "yes" | "ok" | "okay" | "do it" | "allow" | "approve" | "confirm" | "go"
        )
    }

    pub(super) fn is_negative_agent_permission(input: &str) -> bool {
        matches!(
            input.trim().to_lowercase().as_str(),
            "n" | "no" | "nope" | "cancel" | "stop" | "don't" | "dont" | "reject"
        )
    }

    pub(super) fn plan_agent_action(&self, query: &str) -> AgentDecision {
        self.plan_agent_action_locally(query)
    }

    pub(super) fn plan_agent_action_locally(&self, query: &str) -> AgentDecision {
        if Self::looks_like_how_to_question(&query.to_lowercase()) {
            return AgentDecision {
                action: AgentAction::Chat,
                note_index: None,
                title: None,
                search_query: None,
                rationale: String::from("question"),
            };
        }

        let target_note = self.resolve_agent_note_target(query);
        if self.looks_like_note_read_request(query) {
            return AgentDecision {
                action: AgentAction::ReadNote,
                note_index: target_note,
                title: None,
                search_query: Self::infer_agent_search_query(query),
                rationale: String::from("read-note"),
            };
        }
        if Self::looks_like_note_search_request(query) {
            return AgentDecision {
                action: AgentAction::SearchNotes,
                note_index: None,
                title: None,
                search_query: Self::infer_agent_search_query(query),
                rationale: String::from("search-notes"),
            };
        }
        if Self::looks_like_memory_list_request(query)
            && Self::infer_agent_search_query(query).is_none()
        {
            return AgentDecision {
                action: AgentAction::ListMemories,
                note_index: None,
                title: None,
                search_query: None,
                rationale: String::from("list-memories"),
            };
        }
        if Self::looks_like_memory_search_request(query) {
            return AgentDecision {
                action: AgentAction::SearchMemories,
                note_index: None,
                title: None,
                search_query: Self::infer_agent_search_query(query),
                rationale: String::from("search-memories"),
            };
        }
        if Self::looks_like_note_edit_request(query) || self.should_work_on_existing_note(query) {
            let rationale = if target_note.is_some() {
                "edit-target"
            } else {
                "edit-missing-target"
            };
            return AgentDecision {
                action: AgentAction::EditNote,
                note_index: target_note,
                title: None,
                search_query: None,
                rationale: String::from(rationale),
            };
        }
        if Self::looks_like_note_create_request(query) {
            return AgentDecision {
                action: AgentAction::CreateNote,
                note_index: None,
                title: Self::infer_note_title_from_request(query),
                search_query: None,
                rationale: String::from("create"),
            };
        }
        AgentDecision {
            action: AgentAction::Chat,
            note_index: None,
            title: None,
            search_query: None,
            rationale: String::from("chat"),
        }
    }

    pub(super) fn plan_agent_action_with_provider(&self, query: &str) -> Option<AgentDecision> {
        let messages = self.agent_planner_conversation(query);
        let result = match self.ai_provider {
            AiProvider::OpenRouter => {
                let api_key = self.openrouter_api_key.as_deref()?;
                Self::send_openrouter_chat_blocking(api_key, &messages)
            }
            AiProvider::Strix => {
                let access_token = self.strix_access_token.as_deref()?;
                Self::send_strix_planner_request(
                    &Self::strix_api_base_url(),
                    access_token,
                    &messages,
                    &self.notes,
                )
            }
        };

        result
            .ok()
            .and_then(|content| self.parse_agent_planner_response(&content, query))
    }

    pub(super) fn agent_planner_conversation(&self, query: &str) -> Vec<(String, String)> {
        let system = String::from(
            "You are Aleph's agent planner. Decide what Aleph should do with the user's chat input. \
             You may choose exactly one action: chat, create_note, edit_note, read_note, search_notes, list_memories, search_memories. \
             Use read_note when the user asks to open, read, inspect, summarize, or answer from a specific note. \
             Use search_notes when the user asks to find notes, look across notes, or identify notes about a topic. \
             Use list_memories or search_memories when the user asks what Aleph remembers or asks to go through memories. \
             Use edit_note when the user asks to work on, continue, improve, rewrite, append to, organize, or otherwise change an existing/current note. \
             Use create_note when the user wants new durable writing and no existing note is the right target. \
             Use chat for questions, explanations, brainstorming without durable write intent, or when you need to ask a clarification. \
             Return ONLY compact JSON with this schema: {\"action\":\"chat|create_note|edit_note|read_note|search_notes|list_memories|search_memories\",\"note_id\":number|null,\"title\":string|null,\"query\":string|null,\"rationale\":string}. \
             Do not write prose outside JSON.",
        );

        let mut notes = Vec::new();
        for (index, note) in self.notes.iter().enumerate().take(40) {
            notes.push(format!(
                "- id={}{} title=\"{}\" preview=\"{}\"",
                note.id,
                if index == self.selected_note {
                    " selected=true"
                } else {
                    ""
                },
                note.title.replace('"', "'"),
                Self::preview_text(&note.content, 120).replace('"', "'")
            ));
        }

        let selected = self
            .notes
            .get(self.selected_note)
            .map(|note| format!("id={} title=\"{}\"", note.id, note.title.replace('"', "'")))
            .unwrap_or_else(|| String::from("none"));
        let user = format!(
            "Selected note: {}\n\nAvailable notes:\n{}\n\nUser input:\n{}",
            selected,
            notes.join("\n"),
            query
        );

        vec![
            (String::from("system"), system),
            (String::from("user"), user),
        ]
    }

    pub(super) fn parse_agent_planner_response(
        &self,
        content: &str,
        query: &str,
    ) -> Option<AgentDecision> {
        let json = Self::extract_json_object(content)?;
        let value: serde_json::Value = serde_json::from_str(json).ok()?;
        let action = match value.get("action")?.as_str()?.trim() {
            "chat" => AgentAction::Chat,
            "create_note" => AgentAction::CreateNote,
            "edit_note" => AgentAction::EditNote,
            "read_note" => AgentAction::ReadNote,
            "search_notes" => AgentAction::SearchNotes,
            "list_memories" => AgentAction::ListMemories,
            "search_memories" => AgentAction::SearchMemories,
            _ => return None,
        };
        let note_index = value
            .get("note_id")
            .and_then(|id| {
                id.as_u64()
                    .and_then(|id| self.note_index_by_id(id as usize))
                    .or_else(|| {
                        id.as_str()
                            .and_then(|target| self.resolve_note_index(target))
                    })
            })
            .or_else(|| {
                value
                    .get("title")
                    .and_then(|title| title.as_str())
                    .and_then(|title| self.resolve_note_index(title))
            })
            .or_else(|| {
                if action == AgentAction::EditNote {
                    self.resolve_agent_note_target(query)
                } else if action == AgentAction::ReadNote {
                    self.resolve_agent_note_target(query)
                } else {
                    None
                }
            });
        let title = value
            .get("title")
            .and_then(|title| title.as_str())
            .map(str::trim)
            .filter(|title| !title.is_empty())
            .map(|title| title.chars().take(80).collect::<String>());
        let rationale = value
            .get("rationale")
            .and_then(|rationale| rationale.as_str())
            .map(str::trim)
            .filter(|rationale| !rationale.is_empty())
            .unwrap_or("model-plan")
            .chars()
            .take(120)
            .collect::<String>();
        let search_query = value
            .get("query")
            .and_then(|query| query.as_str())
            .map(str::trim)
            .filter(|query| !query.is_empty())
            .map(|query| query.chars().take(120).collect::<String>())
            .or_else(|| Self::infer_agent_search_query(query));

        Some(AgentDecision {
            action,
            note_index,
            title,
            search_query,
            rationale,
        })
    }

    pub(super) fn extract_json_object(content: &str) -> Option<&str> {
        let trimmed = content.trim();
        if trimmed.starts_with('{') && trimmed.ends_with('}') {
            return Some(trimmed);
        }

        let start = trimmed.find('{')?;
        let end = trimmed.rfind('}')?;
        (start < end).then_some(&trimmed[start..=end])
    }

    pub(super) fn run_agent_context_action(&mut self, query: &str, decision: AgentDecision) {
        self.panel_mode = PanelMode::AiChat;
        self.chat_scroll_offset = 0;
        self.push_chat_message("user", query.trim());

        let response = match decision.action {
            AgentAction::ReadNote => self.agent_read_note_response(&decision),
            AgentAction::SearchNotes => self.agent_search_notes_response(&decision, query),
            AgentAction::ListMemories => self.agent_list_memories_response(),
            AgentAction::SearchMemories => self.agent_search_memories_response(&decision, query),
            _ => String::from("That agent action is not available here."),
        };

        self.push_chat_message("assistant", response);
        self.last_action = format!("Agent: {}", decision.rationale);
    }

    pub(super) fn agent_workspace_context(&self) -> String {
        let selected = self
            .notes
            .get(self.selected_note)
            .map(|note| {
                format!(
                    "Selected note: #{} `{}`\n{}",
                    note.id,
                    note.title,
                    Self::preview_text(&note.content, 900)
                )
            })
            .unwrap_or_else(|| String::from("Selected note: none"));

        let notes = self
            .notes
            .iter()
            .take(12)
            .map(|note| {
                format!(
                    "- #{} `{}`: {}",
                    note.id,
                    note.title,
                    Self::preview_text(&note.content, 180)
                )
            })
            .collect::<Vec<_>>();

        let memories = self
            .memories
            .iter()
            .take(12)
            .enumerate()
            .map(|(index, memory)| format!("- memory {}: {}", index + 1, memory))
            .collect::<Vec<_>>();

        format!(
            "Workspace context:\n{}\n\nRecent note index:\n{}\n\nMemories:\n{}",
            selected,
            if notes.is_empty() {
                String::from("- none")
            } else {
                notes.join("\n")
            },
            if memories.is_empty() {
                String::from("- none")
            } else {
                memories.join("\n")
            }
        )
    }

    pub(super) fn agent_read_note_response(&self, decision: &AgentDecision) -> String {
        let Some(index) = decision.note_index.or_else(|| self.current_note_index()) else {
            return String::from("I could not find a note to read.");
        };
        let Some(note) = self.notes.get(index) else {
            return String::from("I could not find that note.");
        };

        let source = Self::note_source_label(note);
        format!(
            "Read note `#{}`: `{}`\n{}\n\n{}",
            note.id,
            note.title,
            source,
            Self::preview_text(&note.content, 1200)
        )
    }

    pub(super) fn agent_search_notes_response(
        &self,
        decision: &AgentDecision,
        original_query: &str,
    ) -> String {
        let query = decision
            .search_query
            .as_deref()
            .unwrap_or(original_query)
            .trim()
            .to_lowercase();
        let mut matches = self
            .notes
            .iter()
            .filter(|note| {
                query.is_empty()
                    || note.title.to_lowercase().contains(&query)
                    || note.content.to_lowercase().contains(&query)
            })
            .take(8)
            .map(|note| {
                format!(
                    "- `#{}` `{}`: {}",
                    note.id,
                    note.title,
                    Self::preview_text(&note.content, 180)
                )
            })
            .collect::<Vec<_>>();

        if matches.is_empty() {
            return format!("I did not find notes matching `{}`.", query);
        }

        matches.insert(0, format!("Found {} note match(es):", matches.len()));
        matches.join("\n")
    }

    pub(super) fn agent_list_memories_response(&self) -> String {
        if self.memories.is_empty() {
            return String::from("There are no saved memories yet.");
        }

        let mut lines = self
            .memories
            .iter()
            .take(12)
            .enumerate()
            .map(|(index, memory)| format!("{}. {}", index + 1, memory))
            .collect::<Vec<_>>();
        lines.insert(0, format!("Saved memories ({}):", self.memories.len()));
        lines.join("\n")
    }

    pub(super) fn agent_search_memories_response(
        &self,
        decision: &AgentDecision,
        original_query: &str,
    ) -> String {
        let query = decision
            .search_query
            .as_deref()
            .unwrap_or(original_query)
            .trim()
            .to_lowercase();
        let mut matches = self
            .memories
            .iter()
            .filter(|memory| query.is_empty() || memory.to_lowercase().contains(&query))
            .take(12)
            .cloned()
            .collect::<Vec<_>>();

        if matches.is_empty() {
            return format!("I did not find memories matching `{}`.", query);
        }

        matches.insert(0, format!("Found {} memory match(es):", matches.len()));
        matches.join("\n")
    }

    pub(super) fn start_note_create_agent(&mut self, query: &str, title: Option<String>) -> bool {
        let title = title.unwrap_or_else(|| String::from("AI draft"));
        self.panel_mode = PanelMode::FullEditor;
        self.panel_title = format!("Drafting: {}", title);
        self.panel_lines.clear();
        self.editor_note_index = None;
        self.editor_buffer.clear();
        self.editor_cursor = 0;
        self.editor_scroll_offset = 0;
        self.open_ai_overlay();
        self.ai_draft_create_title = Some(title.clone());
        self.ai_input_buffer = query.trim().to_string();
        self.ai_input_cursor = self.ai_input_buffer.len();
        self.ghost_submit_instruction();
        self.last_action = format!("AI is drafting a new note: {}", title);
        true
    }

    pub(super) fn looks_like_note_create_request(query: &str) -> bool {
        let lower = query.to_lowercase();
        if Self::looks_like_how_to_question(&lower) {
            return false;
        }

        let mentions_note = lower.contains("note")
            || lower.contains("notes")
            || lower.contains("draft")
            || lower.contains("write-up")
            || lower.contains("writeup");
        let direct_note_create = [
            "write a note",
            "write me a note",
            "write notes",
            "create a note",
            "create note",
            "make a note",
            "make note",
            "draft a note",
            "draft note",
            "compose a note",
            "compose note",
            "new note",
            "add a note",
            "take a note",
            "write-up",
            "writeup",
            "turn this into a note",
            "save this as a note",
            "write this down",
            "capture this",
        ]
        .iter()
        .any(|needle| lower.contains(needle));
        if direct_note_create {
            return true;
        }

        let starts_like_write_task = [
            "write ",
            "draft ",
            "compose ",
            "outline ",
            "prepare ",
            "make ",
            "create ",
            "generate ",
            "can you write ",
            "please write ",
            "can you draft ",
            "please draft ",
        ]
        .iter()
        .any(|prefix| lower.trim_start().starts_with(prefix));

        let content_shape = [
            " about ",
            " on ",
            " for ",
            " explaining ",
            " covering ",
            " that ",
        ]
        .iter()
        .any(|needle| lower.contains(needle));

        starts_like_write_task && (mentions_note || content_shape)
    }

    pub(super) fn looks_like_note_read_request(&self, query: &str) -> bool {
        let lower = query.to_lowercase();
        let mentions_specific_note = lower.contains("current note")
            || lower.contains("selected note")
            || lower.contains("this note")
            || lower.contains("that note")
            || self.find_note_mentioned_in_text(query).is_some();
        let wants_read = [
            "read",
            "open",
            "show",
            "inspect",
            "summarize",
            "what does",
            "what's in",
            "what is in",
        ]
        .iter()
        .any(|needle| lower.contains(needle));

        wants_read && mentions_specific_note
    }

    pub(super) fn looks_like_note_search_request(query: &str) -> bool {
        let lower = query.to_lowercase();
        let mentions_notes = lower.contains("note") || lower.contains("notes");
        let wants_search = [
            "search",
            "find",
            "look through",
            "go through",
            "scan",
            "which notes",
            "notes about",
            "anything about",
            "where did i write",
        ]
        .iter()
        .any(|needle| lower.contains(needle));

        mentions_notes && wants_search
    }

    pub(super) fn looks_like_memory_list_request(query: &str) -> bool {
        let lower = query.to_lowercase();
        let mentions_memories = lower.contains("memory") || lower.contains("memories");
        let wants_list = [
            "list",
            "show",
            "what do you remember",
            "what have you remembered",
            "go through",
            "review",
        ]
        .iter()
        .any(|needle| lower.contains(needle));

        mentions_memories && wants_list
    }

    pub(super) fn looks_like_memory_search_request(query: &str) -> bool {
        let lower = query.to_lowercase();
        let mentions_memories = lower.contains("memory") || lower.contains("memories");
        let wants_search = [
            "search",
            "find",
            "look through",
            "go through",
            "scan",
            "anything about",
            "remember about",
        ]
        .iter()
        .any(|needle| lower.contains(needle));

        mentions_memories && wants_search
    }

    pub(super) fn infer_agent_search_query(query: &str) -> Option<String> {
        let trimmed = query.trim();
        let lower = trimmed.to_lowercase();
        for marker in [
            " about ",
            " for ",
            " matching ",
            " containing ",
            " called ",
            " named ",
            " titled ",
        ] {
            if let Some((_, rest)) = lower.split_once(marker) {
                let start = trimmed.len().saturating_sub(rest.len());
                let candidate = trimmed[start..]
                    .split(['.', '?', '!', ';'])
                    .next()
                    .unwrap_or_default()
                    .trim_matches(|c: char| c == '"' || c == '\'' || c.is_whitespace());
                if !candidate.is_empty() {
                    return Some(candidate.chars().take(120).collect());
                }
            }
        }

        None
    }

    pub(super) fn infer_note_title_from_request(query: &str) -> Option<String> {
        let trimmed = query.trim();
        for marker in [" titled ", " called ", " named "] {
            if let Some((_, rest)) = trimmed.to_lowercase().split_once(marker) {
                let start = trimmed.len().saturating_sub(rest.len());
                let title = trimmed[start..]
                    .split(['.', ',', ';'])
                    .next()
                    .unwrap_or_default()
                    .trim_matches(|c: char| c == '"' || c == '\'' || c.is_whitespace());
                if !title.is_empty() {
                    return Some(title.chars().take(80).collect());
                }
            }
        }

        for marker in [" about ", " on "] {
            if let Some((_, rest)) = trimmed.to_lowercase().split_once(marker) {
                let start = trimmed.len().saturating_sub(rest.len());
                let topic = trimmed[start..]
                    .split(['.', ';'])
                    .next()
                    .unwrap_or_default()
                    .trim_matches(|c: char| c == '"' || c == '\'' || c.is_whitespace());
                if !topic.is_empty() {
                    return Some(Self::title_case_note_topic(topic));
                }
            }
        }

        None
    }

    pub(super) fn title_case_note_topic(topic: &str) -> String {
        let words = topic
            .split_whitespace()
            .take(8)
            .map(|word| {
                let mut chars = word.chars();
                match chars.next() {
                    Some(first) => {
                        let mut titled = first.to_uppercase().collect::<String>();
                        titled.push_str(chars.as_str());
                        titled
                    }
                    None => String::new(),
                }
            })
            .filter(|word| !word.is_empty())
            .collect::<Vec<_>>();

        if words.is_empty() {
            String::from("AI draft")
        } else {
            words.join(" ")
        }
    }

    pub(super) fn start_note_edit_agent(&mut self, query: &str, decision: AgentDecision) -> bool {
        let Some(index) = decision.note_index else {
            self.set_result_panel(
                "AI note edit",
                vec![
                    String::from("I decided this is note work, but I need a target note."),
                    String::from(
                        "Name a note, select one with /note list, or ask me to create a new note.",
                    ),
                ],
            );
            self.last_action = String::from("AI note edit needs a note target.");
            return true;
        };

        self.open_note_editor(index);
        self.open_ai_overlay();
        self.ai_input_buffer = query.trim().to_string();
        self.ai_input_cursor = self.ai_input_buffer.len();
        self.ghost_submit_instruction();
        self.last_action = format!(
            "AI is preparing edits for note: {} ({})",
            self.notes[index].title, decision.rationale
        );
        true
    }

    pub(super) fn should_work_on_existing_note(&self, query: &str) -> bool {
        let lower = query.to_lowercase();
        if Self::looks_like_how_to_question(&lower) {
            return false;
        }

        let asks_for_work = [
            "work on",
            "keep working on",
            "continue",
            "finish",
            "develop",
            "refine",
            "iterate on",
            "take another pass",
            "make progress",
        ]
        .iter()
        .any(|needle| lower.contains(needle));
        let references_existing_context = [
            "existing note",
            "current note",
            "selected note",
            "this note",
            "that note",
            "the note",
            "existing draft",
            "current draft",
            "this draft",
            "it",
            "this",
        ]
        .iter()
        .any(|needle| lower.contains(needle));

        asks_for_work && references_existing_context
    }

    pub(super) fn looks_like_note_edit_request(query: &str) -> bool {
        let lower = query.to_lowercase();
        if Self::looks_like_how_to_question(&lower) {
            return false;
        }

        let mentions_note = lower.contains("note")
            || lower.contains("notes")
            || lower.contains("this doc")
            || lower.contains("current doc")
            || lower.contains("current note")
            || lower.contains("selected note")
            || lower.contains("this note")
            || lower.contains("draft");
        let references_current_text = [
            "this",
            "current",
            "selected",
            "existing",
            "the note",
            "my note",
            "the draft",
            "my draft",
        ]
        .iter()
        .any(|needle| lower.contains(needle));
        let wants_edit = [
            "edit",
            "rewrite",
            "update",
            "change",
            "append",
            "add",
            "insert",
            "write",
            "draft",
            "improve",
            "fix",
            "clean up",
            "summarize",
            "turn this into",
            "make this",
            "make it",
            "expand",
            "shorten",
            "polish",
            "refactor",
            "convert",
        ]
        .iter()
        .any(|needle| lower.contains(needle));

        (mentions_note || references_current_text) && wants_edit
    }

    pub(super) fn looks_like_how_to_question(lower: &str) -> bool {
        let trimmed = lower.trim_start();
        [
            "how do i ",
            "how can i ",
            "how should i ",
            "what is ",
            "what are ",
            "why does ",
            "why is ",
            "can you explain ",
            "explain how ",
        ]
        .iter()
        .any(|prefix| trimmed.starts_with(prefix))
    }

    pub(super) fn resolve_agent_note_target(&self, query: &str) -> Option<usize> {
        let lower = query.to_lowercase();
        if let Some(index) = self.find_note_mentioned_in_text(query) {
            return Some(index);
        }

        for marker in ["note ", "notes ", "doc ", "draft "] {
            if let Some(pos) = lower.find(marker) {
                let candidate = query[pos + marker.len()..]
                    .split(['.', ',', ':', ';'])
                    .next()
                    .unwrap_or_default()
                    .trim_matches(|c: char| c == '"' || c == '\'' || c.is_whitespace());
                if let Some(index) = self.resolve_note_index(candidate) {
                    return Some(index);
                }
            }
        }
        self.current_note_index()
    }

    pub(super) fn find_note_mentioned_in_text(&self, query: &str) -> Option<usize> {
        let lower = query.to_lowercase();
        for token in lower.split_whitespace() {
            let normalized = token
                .trim_matches(|character: char| !character.is_ascii_alphanumeric())
                .trim_start_matches('#');
            if let Ok(note_id) = normalized.parse::<usize>() {
                if let Some(index) = self
                    .notes
                    .iter()
                    .enumerate()
                    .find_map(|(index, note)| (note.id == note_id).then_some(index))
                {
                    return Some(index);
                }
            }
        }

        self.notes
            .iter()
            .enumerate()
            .filter_map(|(index, note)| {
                let title = note.title.to_lowercase();
                let title_words = title.split_whitespace().count();
                let remote_match = note
                    .remote_id
                    .as_deref()
                    .map(|remote_id| lower.contains(&remote_id.to_lowercase()))
                    .unwrap_or(false);

                if remote_match || (!title.is_empty() && lower.contains(&title)) {
                    Some((index, title_words))
                } else {
                    None
                }
            })
            .max_by_key(|(_, title_words)| *title_words)
            .map(|(index, _)| index)
    }
}
