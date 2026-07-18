use super::*;

pub(super) struct NoteSearchResult {
    pub(super) index: usize,
    score: usize,
    snippets: Vec<String>,
}

pub(super) struct MemorySearchResult {
    pub(super) index: usize,
    score: usize,
}

#[allow(dead_code)]
impl App {
    pub(super) fn try_start_agent_action(&mut self, query: &str) -> bool {
        if self
            .active_agent_run()
            .is_some_and(|run| !run.phase.is_terminal())
        {
            self.last_action = String::from("Aleph is still working on the previous request.");
            return false;
        }
        if self.chat_stream_rx.is_some() {
            self.last_action = String::from("Aleph is still answering the previous message.");
            return false;
        }
        if self.agent_plan_rx.is_some() {
            self.last_action = String::from("Aleph is still planning the previous message.");
            return false;
        }

        self.begin_run(query, RunPhase::Planning);
        let decision = self.plan_agent_action(query);
        match decision.action {
            AgentAction::CreateNote | AgentAction::EditNote | AgentAction::SaveMemory => {
                self.stage_agent_action(query, decision);
                true
            }
            AgentAction::ReadNote
            | AgentAction::SearchNotes
            | AgentAction::ListMemories
            | AgentAction::SearchMemories
            | AgentAction::WorkspaceStatus
            | AgentAction::SearchTrail => {
                self.run_agent_loop(query, decision);
                true
            }
            // The local heuristics only route unambiguous phrasings. When
            // they fall through to the generic chat bucket, ask the model
            // planner (in the background) before giving up on tool use.
            AgentAction::Chat if decision.rationale == "chat" => {
                self.start_agent_model_plan(query) || self.start_agent_model_loop(query)
            }
            AgentAction::Chat => self.start_agent_model_loop(query),
        }
    }

    /// Spawn the provider-backed planner in the background. Returns false when
    /// no provider is connected (caller falls back to a plain chat turn).
    pub(super) fn start_agent_model_plan(&mut self, query: &str) -> bool {
        if self
            .active_agent_run()
            .is_some_and(|run| !run.phase.is_terminal() && run.request != query.trim())
        {
            self.last_action = String::from("Aleph is still working on the previous request.");
            return false;
        }
        if self.chat_stream_rx.is_some() {
            self.last_action = String::from("Aleph is still answering the previous message.");
            return false;
        }
        if !(self.is_openrouter_connected() || self.is_strix_connected()) {
            return false;
        }

        self.begin_run(query, RunPhase::Planning);
        let messages = self.agent_planner_conversation(query);
        let provider = self.ai_provider;
        let openrouter_api_key = self.openrouter_api_key.clone();
        let strix_access_token = self.strix_access_token.clone();
        let strix_notes = if provider == AiProvider::Strix {
            self.notes.clone()
        } else {
            Vec::new()
        };

        self.panel_mode = PanelMode::AiChat;
        self.follow_chat_tail();
        self.push_chat_message("user", query.trim());
        self.thinking = true;
        self.thinking_status = String::from("choosing the next action");
        self.thinking_ticks_remaining = 20;
        self.add_activity("Asking the model planner for the next action.");
        self.agent_plan_query = Some(query.trim().to_string());

        let (sender, receiver) = mpsc::channel();
        self.agent_plan_rx = Some(receiver);
        thread::spawn(move || {
            let result = match provider {
                AiProvider::OpenRouter => match openrouter_api_key {
                    Some(api_key) => Self::send_openrouter_chat_blocking(&api_key, &messages),
                    None => Err(String::from("OpenRouter is not configured.")),
                },
                AiProvider::Strix => match strix_access_token {
                    Some(token) => Self::send_strix_planner_request(
                        &Self::strix_api_base_url(),
                        &token,
                        &messages,
                        &strix_notes,
                    )
                    .or_else(|error| match openrouter_api_key {
                        Some(api_key) => Self::send_openrouter_chat_blocking(&api_key, &messages)
                            .map_err(|fallback| format!("{} | {}", error, fallback)),
                        None => Err(error),
                    }),
                    None => Err(String::from("Strix is not connected.")),
                },
            };
            let _ = sender.send(result);
        });

        true
    }

    pub(super) fn process_agent_plan(&mut self) {
        let result = match self.agent_plan_rx.as_ref() {
            Some(receiver) => receiver.try_recv(),
            None => return,
        };

        match result {
            Ok(outcome) => {
                self.agent_plan_rx = None;
                self.thinking = false;
                self.thinking_status.clear();
                self.thinking_ticks_remaining = 0;
                let query = self.agent_plan_query.take().unwrap_or_default();
                let decision = outcome
                    .ok()
                    .and_then(|content| self.parse_agent_planner_response(&content, &query));
                self.execute_planned_agent_decision(&query, decision);
            }
            Err(TryRecvError::Empty) => {
                self.thinking = true;
                self.thinking_status = String::from("choosing the next action");
            }
            Err(TryRecvError::Disconnected) => {
                self.agent_plan_rx = None;
                self.thinking = false;
                self.thinking_status.clear();
                self.thinking_ticks_remaining = 0;
                let query = self.agent_plan_query.take().unwrap_or_default();
                if !query.is_empty() {
                    self.start_chat_turn_without_user_message(query);
                }
            }
        }
    }

    fn execute_planned_agent_decision(&mut self, query: &str, decision: Option<AgentDecision>) {
        let Some(decision) = decision else {
            self.add_activity("Planner gave no usable plan; answering directly.");
            self.start_chat_turn_without_user_message(query.to_string());
            return;
        };

        self.add_activity(format!(
            "Planner chose {} ({}).",
            Self::agent_action_label(decision.action),
            decision.rationale
        ));
        match decision.action {
            AgentAction::CreateNote | AgentAction::EditNote | AgentAction::SaveMemory => {
                self.stage_agent_action_inner(query, decision, false);
            }
            AgentAction::ReadNote
            | AgentAction::SearchNotes
            | AgentAction::ListMemories
            | AgentAction::SearchMemories
            | AgentAction::WorkspaceStatus
            | AgentAction::SearchTrail => {
                self.run_agent_loop_inner(query, decision, false);
            }
            AgentAction::Chat => {
                self.start_chat_turn_without_user_message(query.to_string());
            }
        }
    }

    pub(super) fn stage_agent_action(&mut self, query: &str, decision: AgentDecision) {
        self.stage_agent_action_inner(query, decision, true);
    }

    fn stage_agent_action_inner(
        &mut self,
        query: &str,
        decision: AgentDecision,
        push_user_message: bool,
    ) {
        self.panel_mode = PanelMode::AiChat;
        self.follow_chat_tail();
        if push_user_message {
            self.push_chat_message("user", query.trim());
        }
        self.add_activity(format!(
            "Aleph chose {} from the local request.",
            Self::agent_action_label(decision.action)
        ));

        if decision.action == AgentAction::EditNote && decision.note_index.is_none() {
            self.pending_agent_query = None;
            self.pending_agent_decision = None;
            self.push_chat_message(
                "assistant",
                "I can help edit a note, but I need the target first. Name the note, select one with `/note list`, or ask me to draft a new one.",
            );
            self.last_action = String::from("Agent needs a note target.");
            let _ = self.complete_run("A write target is required before Aleph can continue.");
            return;
        }

        let message = self.agent_permission_message(&decision);
        self.pending_agent_query = Some(query.trim().to_string());
        self.pending_agent_decision = Some(decision);
        let decision = self
            .pending_agent_decision
            .as_ref()
            .expect("decision was stored");
        let approval = self.approval_request_for_decision(decision);
        let change = RunChange {
            target: approval.target.clone(),
            summary: approval.effect.clone(),
            status: ChangeStatus::Proposed,
        };
        let _ = self.request_approval(approval, change);
        self.push_chat_message("assistant", message);
        self.last_action = String::from("Agent action waiting for permission.");
        self.add_activity("Waiting for permission before writing.");
    }

    pub(super) fn agent_permission_message(&self, decision: &AgentDecision) -> String {
        match decision.action {
            AgentAction::CreateNote => {
                if let Some(title) = decision.title.as_deref() {
                    format!(
                        "I can draft a new note titled `{}`. Press Enter to approve, or type `no`.",
                        title
                    )
                } else {
                    String::from(
                        "I can draft this as a new note. Press Enter to approve, or type `no`.",
                    )
                }
            }
            AgentAction::EditNote => {
                let note_title = decision
                    .note_index
                    .and_then(|index| self.notes.get(index))
                    .map(|note| note.title.as_str())
                    .unwrap_or("the selected note");
                format!(
                    "I can revise `{}`. Press Enter to approve, or type `no`.",
                    note_title
                )
            }
            AgentAction::SaveMemory => {
                let memory = decision.search_query.as_deref().unwrap_or("this memory");
                format!(
                    "I can save this memory: `{}`. Press Enter to approve, or type `no`.",
                    Self::preview_text(memory, 120)
                )
            }
            AgentAction::ReadNote
            | AgentAction::SearchNotes
            | AgentAction::ListMemories
            | AgentAction::SearchMemories
            | AgentAction::WorkspaceStatus
            | AgentAction::SearchTrail => String::new(),
            AgentAction::Chat => String::new(),
        }
    }

    pub(super) fn confirm_pending_agent_action(&mut self) -> bool {
        let Some(decision) = self.pending_agent_decision.take() else {
            return false;
        };
        let query = self.pending_agent_query.take().unwrap_or_default();
        if self.approve_request().is_err() {
            return false;
        }
        match decision.action {
            AgentAction::CreateNote => self.start_note_create_agent(&query, decision.title),
            AgentAction::EditNote => self.start_note_edit_agent(&query, decision),
            AgentAction::ReadNote
            | AgentAction::SearchNotes
            | AgentAction::SaveMemory
            | AgentAction::ListMemories
            | AgentAction::SearchMemories
            | AgentAction::WorkspaceStatus
            | AgentAction::SearchTrail => {
                self.run_agent_loop_inner(&query, decision, false);
                true
            }
            AgentAction::Chat => false,
        }
    }

    pub(super) fn cancel_pending_agent_action(&mut self) {
        self.pending_agent_query = None;
        self.pending_agent_decision = None;
        let _ = self.reject_request("The user rejected the proposed write.");
        self.push_chat_message("assistant", "Cancelled the pending action.");
        self.last_action = String::from("Cancelled pending agent action.");
    }

    fn approval_request_for_decision(&self, decision: &AgentDecision) -> ApprovalRequest {
        match decision.action {
            AgentAction::CreateNote => {
                let target = decision.title.as_deref().unwrap_or("new note");
                ApprovalRequest {
                    operation: String::from("Create note"),
                    target: target.to_string(),
                    effect: format!("Draft and open a new note named `{}`.", target),
                }
            }
            AgentAction::EditNote => {
                let target = decision
                    .note_index
                    .and_then(|index| self.notes.get(index))
                    .map(|note| note.title.clone())
                    .unwrap_or_else(|| String::from("selected note"));
                ApprovalRequest {
                    operation: String::from("Edit note"),
                    target: target.clone(),
                    effect: format!("Prepare proposed revisions for `{}`.", target),
                }
            }
            AgentAction::SaveMemory => ApprovalRequest {
                operation: String::from("Save memory"),
                target: String::from("local memories"),
                effect: format!(
                    "Save `{}` to Aleph's local memory store.",
                    Self::preview_text(
                        decision.search_query.as_deref().unwrap_or("this memory"),
                        120
                    )
                ),
            },
            _ => ApprovalRequest {
                operation: String::from("Write"),
                target: String::from("workspace"),
                effect: String::from("Apply the proposed workspace change."),
            },
        }
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
                search_query: self.infer_agent_search_query_for_turn(query),
                rationale: String::from("read-note"),
            };
        }
        if Self::looks_like_note_search_request(query) {
            return AgentDecision {
                action: AgentAction::SearchNotes,
                note_index: None,
                title: None,
                search_query: self.infer_agent_search_query_for_turn(query),
                rationale: String::from("search-notes"),
            };
        }
        if self.looks_like_followup_lookup_request(query) {
            return AgentDecision {
                action: AgentAction::SearchNotes,
                note_index: None,
                title: None,
                search_query: self.infer_agent_search_query_for_turn(query),
                rationale: String::from("follow-up-search"),
            };
        }
        if Self::looks_like_memory_list_request(query) {
            return AgentDecision {
                action: AgentAction::ListMemories,
                note_index: None,
                title: None,
                search_query: None,
                rationale: String::from("list-memories"),
            };
        }
        if Self::looks_like_memory_save_request(query) {
            return AgentDecision {
                action: AgentAction::SaveMemory,
                note_index: None,
                title: None,
                search_query: Self::infer_memory_text_from_request(query),
                rationale: String::from("save-memory"),
            };
        }
        if Self::looks_like_memory_search_request(query) {
            return AgentDecision {
                action: AgentAction::SearchMemories,
                note_index: None,
                title: None,
                search_query: self.infer_agent_search_query_for_turn(query),
                rationale: String::from("search-memories"),
            };
        }
        if Self::looks_like_workspace_status_request(query) {
            return AgentDecision {
                action: AgentAction::WorkspaceStatus,
                note_index: None,
                title: None,
                search_query: None,
                rationale: String::from("workspace-status"),
            };
        }
        if Self::looks_like_trail_search_request(query) {
            return AgentDecision {
                action: AgentAction::SearchTrail,
                note_index: None,
                title: None,
                search_query: self.infer_agent_search_query_for_turn(query),
                rationale: String::from("search-trail"),
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
        if Self::looks_like_direct_smalltalk(query) {
            return AgentDecision {
                action: AgentAction::Chat,
                note_index: None,
                title: None,
                search_query: None,
                rationale: String::from("smalltalk"),
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
             You may choose exactly one action: chat, create_note, edit_note, read_note, search_notes, save_memory, list_memories, search_memories, workspace_status, search_trail. \
             Use read_note when the user asks to open, read, inspect, summarize, or answer from a specific note. \
             Use search_notes when the user asks to find notes, look across notes, or identify notes about a topic. \
             Use save_memory when the user explicitly asks Aleph to remember, save as memory, or keep a durable preference/fact. \
             Use list_memories or search_memories when the user asks what Aleph remembers or asks to go through memories. \
             Use workspace_status when the user asks Aleph to inspect the current computer/workspace, repo state, files changed, daemon state, connections, or local runtime status. \
             Use search_trail when the user asks for recent work, local activity, workspace trail, timeline, traces, or what happened before. \
             Use edit_note when the user asks to work on, continue, improve, rewrite, append to, organize, or otherwise change an existing/current note. \
             Use create_note when the user wants new durable writing and no existing note is the right target. \
             Use chat for questions, explanations, brainstorming without durable write intent, or when you need to ask a clarification. \
             Return ONLY compact JSON with this schema: {\"action\":\"chat|create_note|edit_note|read_note|search_notes|save_memory|list_memories|search_memories|workspace_status|search_trail\",\"note_id\":number|null,\"title\":string|null,\"query\":string|null,\"rationale\":string}. \
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
            "Selected note: {}\n\nRecent conversation:\n{}\n\nAvailable notes:\n{}\n\nUser input:\n{}",
            selected,
            self.recent_agent_chat_context(8),
            notes.join("\n"),
            query
        );

        vec![
            (String::from("system"), system),
            (String::from("user"), user),
        ]
    }

    fn recent_agent_chat_context(&self, limit: usize) -> String {
        let mut messages = self
            .chat_messages
            .iter()
            .rev()
            .filter(|message| !message.content.trim().is_empty())
            .take(limit)
            .map(|message| {
                format!(
                    "- {}: {}",
                    message.role,
                    Self::preview_text(message.content.trim(), 220)
                )
            })
            .collect::<Vec<_>>();
        messages.reverse();
        if messages.is_empty() {
            String::from("- none")
        } else {
            messages.join("\n")
        }
    }

    fn infer_agent_search_query_for_turn(&self, query: &str) -> Option<String> {
        let inferred = Self::infer_agent_search_query(query);
        if inferred
            .as_deref()
            .is_some_and(Self::is_followup_placeholder_query)
        {
            return self.infer_search_query_from_recent_chat().or(inferred);
        }
        inferred.or_else(|| self.infer_search_query_from_recent_chat())
    }

    fn is_followup_placeholder_query(query: &str) -> bool {
        matches!(
            query.trim().to_lowercase().as_str(),
            "it" | "that" | "this" | "them" | "those" | "the same thing" | "same thing"
        )
    }

    fn infer_search_query_from_recent_chat(&self) -> Option<String> {
        self.chat_messages
            .iter()
            .rev()
            .filter(|message| message.role == "user")
            .filter_map(|message| Self::infer_agent_search_query(&message.content))
            .find(|query| !Self::is_followup_placeholder_query(query))
    }

    fn looks_like_followup_lookup_request(&self, query: &str) -> bool {
        let cleaned = Self::infer_agent_search_query(query);
        let Some(cleaned) = cleaned.as_deref() else {
            return false;
        };
        if !Self::is_followup_placeholder_query(cleaned) {
            return false;
        }
        let lower = query.to_lowercase();
        let asks_lookup = ["find", "search", "show", "open", "read", "look up"]
            .iter()
            .any(|needle| lower.contains(needle));
        asks_lookup && self.infer_search_query_from_recent_chat().is_some()
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
            "save_memory" => AgentAction::SaveMemory,
            "list_memories" => AgentAction::ListMemories,
            "search_memories" => AgentAction::SearchMemories,
            "workspace_status" => AgentAction::WorkspaceStatus,
            "search_trail" => AgentAction::SearchTrail,
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
                if matches!(action, AgentAction::EditNote | AgentAction::ReadNote) {
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
            .or_else(|| {
                if action == AgentAction::SaveMemory {
                    Self::infer_memory_text_from_request(query)
                } else {
                    Self::infer_agent_search_query(query)
                }
            });

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
        self.follow_chat_tail();
        self.push_chat_message("user", query.trim());
        self.add_activity("Reading local context.");

        let response = match decision.action {
            AgentAction::ReadNote => self.agent_read_note_response(&decision),
            AgentAction::SearchNotes => self.agent_search_notes_response(&decision, query),
            AgentAction::SaveMemory => self.agent_save_memory_response(&decision, query),
            AgentAction::ListMemories => self.agent_list_memories_response(),
            AgentAction::SearchMemories => self.agent_search_memories_response(&decision, query),
            AgentAction::WorkspaceStatus => self.agent_workspace_status_response(),
            AgentAction::SearchTrail => self.agent_search_trail_response(&decision, query),
            _ => String::from("That agent action is not available here."),
        };

        self.push_chat_message("assistant", response);
        self.last_action = format!("Agent: {}", Self::agent_action_label(decision.action));
        self.add_activity("Returned local context.");
        if decision.action == AgentAction::SaveMemory {
            let _ = self.mark_proposed_changes(ChangeStatus::Applied);
        }
        let _ = self.complete_run(if decision.action == AgentAction::SaveMemory {
            "Saved the approved memory change."
        } else {
            "Completed from local context. No changes were made."
        });
    }

    fn run_agent_loop(&mut self, query: &str, decision: AgentDecision) {
        self.run_agent_loop_inner(query, decision, true);
    }

    fn run_agent_loop_inner(
        &mut self,
        query: &str,
        decision: AgentDecision,
        push_user_message: bool,
    ) {
        self.panel_mode = PanelMode::AiChat;
        self.follow_chat_tail();
        if push_user_message {
            self.push_chat_message("user", query.trim());
        }
        self.add_activity("Preparing local context steps.");
        self.last_action = format!("Aleph agent: {}", Self::agent_action_label(decision.action));

        let plan = self.agent_loop_plan(query, &decision);
        let queued = plan.iter().map(|step| {
            (
                step.progress_line().to_string(),
                Some(step.label().to_string()),
            )
        });
        if let Err(error) = self.queue_run_steps(queued) {
            let _ = self.fail_run(error);
            return;
        }

        self.agent_execution_generation = self.agent_execution_generation.wrapping_add(1);
        let generation = self.agent_execution_generation;
        let run_id = self.active_run_id.unwrap_or_default();
        let pending_provider = self.agent_loop_should_synthesize(&decision, query)
            && (self.is_openrouter_connected() || self.is_strix_connected());
        self.pending_agent_execution = Some(PendingAgentExecution {
            run_id,
            generation,
            query: query.trim().to_string(),
            decision,
            steps: plan,
            current_step: 0,
            phase: AgentExecutionPhase::StartStep,
            observations: Vec::new(),
            pending_provider,
        });
    }

    /// Advances at most one visible local-agent transition. The terminal loop
    /// calls this once per application iteration, independently of animation
    /// ticks, so Pending, Running, and terminal step states can each render.
    pub(super) fn process_agent_execution(&mut self) {
        let Some(mut execution) = self.pending_agent_execution.take() else {
            self.discard_stale_agent_worker_results();
            return;
        };

        let run_is_current = self.active_run_id == Some(execution.run_id)
            && self
                .agent_run(execution.run_id)
                .is_some_and(|run| !run.phase.is_terminal());
        if !run_is_current || execution.generation != self.agent_execution_generation {
            self.discard_stale_agent_worker_results();
            return;
        }
        if self
            .agent_run(execution.run_id)
            .is_some_and(|run| run.phase == RunPhase::WaitingApproval)
        {
            self.pending_agent_execution = Some(execution);
            return;
        }

        match execution.phase {
            AgentExecutionPhase::StartStep => {
                if execution.current_step >= execution.steps.len() {
                    execution.phase = AgentExecutionPhase::Finish;
                } else if let Err(error) = self.start_queued_step(execution.current_step) {
                    let _ = self.fail_run(error);
                    return;
                } else {
                    let step = execution.steps[execution.current_step];
                    self.add_activity(step.progress_line());
                    execution.phase = AgentExecutionPhase::ExecuteStep;
                }
                self.pending_agent_execution = Some(execution);
            }
            AgentExecutionPhase::ExecuteStep => {
                let step = execution.steps[execution.current_step];
                if step == AgentLoopStep::InspectWorkspace {
                    let sender = self.agent_worker_tx.clone();
                    let run_id = execution.run_id;
                    let generation = execution.generation;
                    let step_index = execution.current_step;
                    thread::spawn(move || {
                        #[cfg(test)]
                        let result = Ok(None);
                        #[cfg(not(test))]
                        let result = std::panic::catch_unwind(Self::capture_repo_context)
                            .map_err(|_| String::from("repository inspection worker panicked"));
                        let _ = sender.send(AgentWorkerResult {
                            run_id,
                            generation,
                            step_index,
                            result,
                        });
                    });
                    execution.phase = AgentExecutionPhase::WaitingWorker;
                    self.pending_agent_execution = Some(execution);
                    return;
                }

                let observation =
                    self.run_agent_loop_step(step, &execution.query, &execution.decision);
                if let Some(error) = Self::agent_observation_error(&observation) {
                    let _ = self.fail_step(execution.current_step, error);
                    return;
                }
                self.finish_progressive_agent_step(&mut execution, observation);
                self.pending_agent_execution = Some(execution);
            }
            AgentExecutionPhase::WaitingWorker => match self.agent_worker_rx.try_recv() {
                Ok(result)
                    if result.run_id == execution.run_id
                        && result.generation == execution.generation
                        && result.step_index == execution.current_step =>
                {
                    match result.result {
                        Ok(repo) => {
                            let observation = self.agent_workspace_worker_observation(
                                &execution.query,
                                repo.as_ref(),
                            );
                            self.finish_progressive_agent_step(&mut execution, observation);
                            self.pending_agent_execution = Some(execution);
                        }
                        Err(error) => {
                            let _ = self.fail_step(execution.current_step, error);
                        }
                    }
                }
                Ok(_) => {
                    // A cancelled or replaced run can finish after a newer one
                    // starts. Its typed result is intentionally ignored.
                    self.pending_agent_execution = Some(execution);
                }
                Err(TryRecvError::Empty) => {
                    self.pending_agent_execution = Some(execution);
                }
                Err(TryRecvError::Disconnected) => {
                    let _ = self.fail_step(
                        execution.current_step,
                        "The local inspection worker disconnected.",
                    );
                }
            },
            AgentExecutionPhase::Finish => self.finish_progressive_agent_execution(execution),
        }
    }

    fn finish_progressive_agent_step(
        &mut self,
        execution: &mut PendingAgentExecution,
        observation: AgentObservation,
    ) {
        self.add_activity(observation.summary.clone());
        if self
            .complete_step(execution.current_step, observation.summary.clone())
            .is_err()
        {
            return;
        }
        execution.observations.push(observation);
        execution.current_step += 1;
        execution.phase = if execution.current_step < execution.steps.len() {
            AgentExecutionPhase::StartStep
        } else {
            AgentExecutionPhase::Finish
        };
    }

    fn finish_progressive_agent_execution(&mut self, execution: PendingAgentExecution) {
        if execution.pending_provider {
            self.add_activity("Writing an answer from the local findings.");
            let context = self.agent_observations_context(&execution.observations);
            if self.start_chat_turn_with_user_message_and_context(
                execution.query.clone(),
                false,
                Some(context),
            ) {
                self.add_activity("Local findings are in the provider context.");
                return;
            }
            let _ = self.fail_run("Aleph could not start provider synthesis.");
            return;
        }

        let final_answer = self.agent_loop_final_answer(
            &execution.query,
            &execution.decision,
            &execution.observations,
        );
        self.push_chat_message("assistant", final_answer);
        self.add_activity("Answered from local context.");
        if execution.decision.action == AgentAction::SaveMemory {
            let _ = self.mark_proposed_changes(ChangeStatus::Applied);
            let _ = self.complete_run("Saved the approved memory change.");
        } else {
            let _ = self.complete_run("Completed from local context. No changes were made.");
        }
    }

    fn agent_observation_error(observation: &AgentObservation) -> Option<String> {
        observation
            .detail
            .strip_prefix("Memory save failed:")
            .map(|error| error.trim().to_string())
    }

    fn agent_workspace_worker_observation(
        &self,
        query: &str,
        repo: Option<&RepoContext>,
    ) -> AgentObservation {
        let mut detail = vec![
            format!("Workspace context for query: `{}`", query.trim()),
            format!("- agent context: {}", self.agent_context_scope_label()),
            format!("- room: {}", self.active_room_label()),
            format!("- notes: {}", self.notes.len()),
            format!("- memories: {}", self.memories.len()),
        ];
        Self::push_repo_context_lines(&mut detail, repo.cloned());
        let detail = detail.join("\n");
        AgentObservation {
            step: AgentLoopStep::InspectWorkspace,
            summary: Self::agent_observation_summary(AgentLoopStep::InspectWorkspace, &detail),
            progress: AgentLoopStep::InspectWorkspace.progress_line().to_string(),
            detail,
        }
    }

    pub(super) fn terminate_pending_execution(&mut self) {
        if self.pending_agent_execution.take().is_some() {
            self.agent_execution_generation = self.agent_execution_generation.wrapping_add(1);
        }
    }

    pub(super) fn cancel_foreground_run(&mut self, reason: &str) {
        let has_active_run = self
            .active_agent_run()
            .is_some_and(|run| !run.phase.is_terminal());
        if !has_active_run {
            return;
        }
        self.chat_stream_rx = None;
        self.agent_plan_rx = None;
        self.agent_plan_query = None;
        self.streaming_buffer.clear();
        self.streaming_active = false;
        self.thinking = false;
        self.thinking_status.clear();
        self.thinking_ticks_remaining = 0;
        let _ = self.cancel_run(reason);
    }

    fn discard_stale_agent_worker_results(&mut self) {
        while self.agent_worker_rx.try_recv().is_ok() {}
    }

    fn agent_action_label(action: AgentAction) -> &'static str {
        match action {
            AgentAction::Chat => "chat",
            AgentAction::CreateNote => "create note",
            AgentAction::EditNote => "edit note",
            AgentAction::ReadNote => "read note",
            AgentAction::SearchNotes => "search notes",
            AgentAction::SaveMemory => "save memory",
            AgentAction::ListMemories => "list memories",
            AgentAction::SearchMemories => "search memories",
            AgentAction::WorkspaceStatus => "workspace status",
            AgentAction::SearchTrail => "trail search",
        }
    }

    fn agent_loop_plan(&self, query: &str, decision: &AgentDecision) -> Vec<AgentLoopStep> {
        let mut plan = Vec::new();
        match decision.action {
            AgentAction::WorkspaceStatus => {
                plan.push(AgentLoopStep::InspectWorkspace);
                plan.push(AgentLoopStep::CheckDaemon);
            }
            AgentAction::SearchTrail => {
                plan.push(AgentLoopStep::SearchTrail);
                plan.push(AgentLoopStep::InspectWorkspace);
            }
            AgentAction::SearchNotes => {
                plan.push(AgentLoopStep::SearchNotes);
                if Self::agent_query_wants_broad_context(query) {
                    plan.push(AgentLoopStep::SearchMemories);
                    plan.push(AgentLoopStep::SearchTrail);
                }
            }
            AgentAction::ReadNote => {
                plan.push(AgentLoopStep::ReadNote);
                if Self::agent_query_wants_broad_context(query) {
                    plan.push(AgentLoopStep::SearchMemories);
                }
            }
            AgentAction::ListMemories => {
                plan.push(AgentLoopStep::ListMemories);
            }
            AgentAction::SearchMemories => {
                plan.push(AgentLoopStep::SearchMemories);
                if Self::agent_query_wants_broad_context(query) {
                    plan.push(AgentLoopStep::SearchNotes);
                }
            }
            AgentAction::SaveMemory => {
                plan.push(AgentLoopStep::NormalizeMemory);
                plan.push(AgentLoopStep::SaveMemory);
            }
            AgentAction::CreateNote | AgentAction::EditNote | AgentAction::Chat => {
                plan.push(AgentLoopStep::DecideNextAction);
            }
        }

        plan
    }

    fn run_agent_loop_step(
        &mut self,
        step: AgentLoopStep,
        query: &str,
        decision: &AgentDecision,
    ) -> AgentObservation {
        let detail = match step {
            AgentLoopStep::InspectWorkspace => self.agent_workspace_context_for_query(query),
            AgentLoopStep::CheckDaemon => self.daemon_status_lines().join("\n"),
            AgentLoopStep::SearchTrail => self.agent_search_trail_response(decision, query),
            AgentLoopStep::SearchNotes => self.agent_search_notes_response(decision, query),
            AgentLoopStep::ReadNote => self.agent_read_note_response(decision),
            AgentLoopStep::ListMemories => self.agent_list_memories_response(),
            AgentLoopStep::SearchMemories => self.agent_search_memories_response(decision, query),
            AgentLoopStep::NormalizeMemory => decision
                .search_query
                .as_deref()
                .and_then(Self::normalize_memory_text)
                .map(|memory| format!("Memory candidate: {}", memory))
                .unwrap_or_else(|| String::from("No durable memory text found.")),
            AgentLoopStep::SaveMemory => self.agent_save_memory_response(decision, query),
            AgentLoopStep::DecideNextAction => String::from("No local step was needed."),
        };
        AgentObservation {
            step,
            summary: Self::agent_observation_summary(step, &detail),
            progress: step.progress_line().to_string(),
            detail,
        }
    }

    fn agent_loop_final_answer(
        &self,
        query: &str,
        decision: &AgentDecision,
        observations: &[AgentObservation],
    ) -> String {
        if decision.action == AgentAction::WorkspaceStatus {
            return observations
                .iter()
                .map(|observation| observation.detail.trim())
                .filter(|detail| !detail.is_empty())
                .collect::<Vec<_>>()
                .join("\n\n");
        }

        if let Some(primary) = Self::primary_agent_observation(decision, observations) {
            let detail = primary.detail.trim();
            if !detail.is_empty() {
                return Self::preview_text(detail, 1800);
            }
        }

        format!("I did not find anything useful for `{}`.", query.trim())
    }

    fn primary_agent_observation<'a>(
        decision: &AgentDecision,
        observations: &'a [AgentObservation],
    ) -> Option<&'a AgentObservation> {
        let preferred = match decision.action {
            AgentAction::WorkspaceStatus => AgentLoopStep::InspectWorkspace,
            AgentAction::SearchTrail => AgentLoopStep::SearchTrail,
            AgentAction::SearchNotes => AgentLoopStep::SearchNotes,
            AgentAction::ReadNote => AgentLoopStep::ReadNote,
            AgentAction::ListMemories => AgentLoopStep::ListMemories,
            AgentAction::SearchMemories => AgentLoopStep::SearchMemories,
            AgentAction::SaveMemory => AgentLoopStep::SaveMemory,
            AgentAction::CreateNote | AgentAction::EditNote | AgentAction::Chat => {
                AgentLoopStep::DecideNextAction
            }
        };
        observations
            .iter()
            .find(|observation| observation.step == preferred)
            .or_else(|| observations.last())
    }

    fn agent_loop_should_synthesize(&self, decision: &AgentDecision, query: &str) -> bool {
        matches!(
            decision.action,
            AgentAction::ReadNote
                | AgentAction::SearchNotes
                | AgentAction::SearchMemories
                | AgentAction::SearchTrail
        ) || Self::agent_query_wants_broad_context(query)
    }

    fn agent_loop_step_marker(index: usize) -> &'static str {
        match index {
            0 => "Acted",
            1 => "Observed",
            _ => "Continued",
        }
    }

    fn agent_observation_summary(step: AgentLoopStep, detail: &str) -> String {
        let non_empty = detail
            .lines()
            .filter(|line| !line.trim().is_empty())
            .count();
        if matches!(
            step,
            AgentLoopStep::SearchNotes
                | AgentLoopStep::SearchMemories
                | AgentLoopStep::SearchTrail
                | AgentLoopStep::ReadNote
        ) && (detail.contains("I did not find") || detail.contains("No "))
        {
            return format!("{} found nothing directly relevant.", step.label());
        }
        if step == AgentLoopStep::SearchNotes {
            return Self::first_count_phrase(detail, "Found")
                .unwrap_or_else(|| format!("Note search reviewed {} lines.", non_empty));
        }
        if step == AgentLoopStep::SearchMemories {
            return Self::first_count_phrase(detail, "Found")
                .unwrap_or_else(|| format!("Memory search reviewed {} lines.", non_empty));
        }
        if step == AgentLoopStep::SearchTrail {
            return format!("Trail search reviewed {} lines.", non_empty);
        }
        if step == AgentLoopStep::InspectWorkspace {
            return format!("Workspace check captured {} facts.", non_empty);
        }
        if step == AgentLoopStep::CheckDaemon {
            return format!("Daemon check captured {} facts.", non_empty);
        }
        Self::preview_text(detail, 120)
    }

    fn first_count_phrase(detail: &str, prefix: &str) -> Option<String> {
        detail
            .lines()
            .map(str::trim)
            .find(|line| line.starts_with(prefix))
            .map(|line| line.trim_end_matches(':').to_string())
    }

    fn agent_observations_context(&self, observations: &[AgentObservation]) -> String {
        let mut lines = vec![String::from(
            "Agent observations from local tools. Use these as evidence, synthesize directly, and do not repeat raw dumps unless needed.",
        )];
        for (index, observation) in observations.iter().enumerate() {
            lines.push(format!(
                "\nStep {}: {}\nProgress: {}\nSummary: {}\nObservation:\n{}",
                index + 1,
                observation.step.label(),
                observation.progress,
                observation.summary,
                Self::preview_text(&observation.detail, 2800)
            ));
        }
        lines.join("\n")
    }

    pub(super) fn agent_query_wants_broad_context(query: &str) -> bool {
        let lower = query.to_lowercase();
        [
            "everything",
            "all",
            "go through",
            "look through",
            "review",
            "what do you know",
            "what happened",
            "recent",
            "context",
            "workspace",
            "computer",
            "project",
        ]
        .iter()
        .any(|needle| lower.contains(needle))
    }

    pub(super) fn start_agent_model_loop(&mut self, query: &str) -> bool {
        self.begin_run(query, RunPhase::Planning);
        self.panel_mode = PanelMode::AiChat;
        self.follow_chat_tail();
        self.push_chat_message("user", query.trim());
        self.add_activity("Handing chat off to the selected provider.");
        self.start_chat_turn_without_user_message(query.trim().to_string())
    }

    pub(super) fn agent_workspace_context(&self) -> String {
        self.agent_workspace_context_for_query("")
    }

    pub(super) fn agent_workspace_context_for_query(&self, query: &str) -> String {
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

        let ranked_notes = self.ranked_note_matches(query, 12);
        let notes = if ranked_notes.is_empty() {
            self.notes
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
                .collect::<Vec<_>>()
        } else {
            ranked_notes
                .iter()
                .map(|result| {
                    let note = &self.notes[result.index];
                    let source = Self::note_source_label(note);
                    let snippets = if result.snippets.is_empty() {
                        Self::preview_text(&note.content, 220)
                    } else {
                        result.snippets.join(" ... ")
                    };
                    format!(
                        "- #{} `{}` [{}] score={} {}",
                        note.id, note.title, source, result.score, snippets
                    )
                })
                .collect::<Vec<_>>()
        };

        let memory_matches = self.ranked_memory_matches(query, 8);
        let memories = if memory_matches.is_empty() {
            self.memories
                .iter()
                .take(12)
                .enumerate()
                .map(|(index, memory)| format!("- memory {}: {}", index + 1, memory))
                .collect::<Vec<_>>()
        } else {
            memory_matches
                .iter()
                .map(|result| {
                    format!(
                        "- memory {} score={}: {}",
                        result.index + 1,
                        result.score,
                        self.memories[result.index]
                    )
                })
                .collect::<Vec<_>>()
        };

        let workspace_lines = self.workspace_context_lines();
        let trail_matches = self.trail_lines(Some(query));
        let trail = trail_matches
            .into_iter()
            .take(8)
            .collect::<Vec<_>>()
            .join("\n");

        format!(
            "Workspace context for query: `{}`\n{}\n\nComputer/workspace:\n{}\n\nRelevant notes:\n{}\n\nRelevant memories:\n{}\n\nRelevant trail:\n{}",
            query.trim(),
            selected,
            workspace_lines.join("\n"),
            if notes.is_empty() {
                String::from("- none")
            } else {
                notes.join("\n")
            },
            if memories.is_empty() {
                String::from("- none")
            } else {
                memories.join("\n")
            },
            if trail.trim().is_empty() {
                String::from("- none")
            } else {
                trail
            },
        )
    }

    pub(super) fn workspace_context_lines(&self) -> Vec<String> {
        let mut lines = vec![
            format!("- agent context: {}", self.agent_context_scope_label()),
            format!("- room: {}", self.active_room_label()),
            format!("- room scope: {}", self.room_scope_summary()),
            format!("- notes: {}", self.notes.len()),
            format!("- memories: {}", self.memories.len()),
            format!("- canvases: {}", self.canvases.len()),
            format!("- obsidian: {}", self.obsidian_status_label()),
            format!("- note save target: {}", self.note_save_target_label()),
            format!("- trail: {}", Self::trail_path().display()),
        ];

        lines.push(format!(
            "- OpenRouter: {}",
            if self.is_openrouter_connected() {
                "connected"
            } else {
                "offline"
            }
        ));
        lines.push(format!(
            "- Strix: {}",
            if self.is_strix_connected() {
                "connected"
            } else {
                "offline"
            }
        ));

        match self.agent_context_scope {
            AgentContextScope::CurrentFolder => {
                let cwd = std::env::current_dir()
                    .map(|path| path.display().to_string())
                    .unwrap_or_else(|_| String::from("unknown"));
                lines.push(format!("- cwd: {}", cwd));
                Self::push_repo_context_lines(&mut lines, Self::capture_repo_context());
            }
            AgentContextScope::ActiveRoom => {
                let paths = &self.active_room_ref().project_paths;
                if paths.is_empty() {
                    lines.push(String::from("- room paths: none"));
                } else {
                    lines.push(format!("- room paths: {}", paths.len()));
                    for path in paths.iter().take(4) {
                        lines.push(format!("  - {}", path));
                    }
                    let first_existing_path = paths
                        .iter()
                        .map(Path::new)
                        .find(|path| path.exists() && path.is_dir());
                    if let Some(path) = first_existing_path {
                        Self::push_repo_context_lines(
                            &mut lines,
                            Self::capture_repo_context_for_path(path),
                        );
                    } else {
                        lines.push(String::from("- git: no existing room path available"));
                    }
                }
            }
            AgentContextScope::Global => {
                lines.push(String::from("- folder context: global Aleph context only"));
                lines.push(String::from("- git: skipped"));
            }
        }

        lines
    }

    fn push_repo_context_lines(lines: &mut Vec<String>, repo: Option<RepoContext>) {
        if let Some(repo) = repo {
            lines.push(format!("- git cwd: {}", repo.cwd));
            if let Some(branch) = repo.branch {
                lines.push(format!("- git branch: {}", branch));
            }
            if let Some(head) = repo.head {
                lines.push(format!("- git head: {}", head));
            }
            if repo.dirty_files.is_empty() {
                lines.push(String::from("- git dirty files: none"));
            } else {
                lines.push(format!("- git dirty files: {}", repo.dirty_files.len()));
                lines.extend(
                    repo.dirty_files
                        .iter()
                        .take(8)
                        .map(|file| format!("  - {}", file)),
                );
            }
        } else {
            lines.push(String::from("- git: unavailable"));
        }
    }

    pub(super) fn ranked_note_matches(&self, query: &str, limit: usize) -> Vec<NoteSearchResult> {
        let cleaned = Self::infer_agent_search_query(query).unwrap_or_default();
        let lower_query = cleaned.to_lowercase();
        let query_terms = Self::agent_search_terms(&lower_query);
        let mut matches = self
            .notes
            .iter()
            .enumerate()
            .filter_map(|(index, note)| {
                let title = note.title.to_lowercase();
                let content = note.content.to_lowercase();
                let phrase_match = !lower_query.is_empty()
                    && (title.contains(&lower_query) || content.contains(&lower_query));
                let title_hits = query_terms
                    .iter()
                    .filter(|term| title.contains(term.as_str()))
                    .count();
                let content_hits = query_terms
                    .iter()
                    .filter(|term| content.contains(term.as_str()))
                    .count();
                let term_hits = title_hits + content_hits;
                let selected_boost = usize::from(index == self.selected_note);

                if !lower_query.is_empty()
                    && !phrase_match
                    && term_hits == 0
                    && !query_terms.is_empty()
                {
                    return None;
                }

                let score = (usize::from(phrase_match) * 10)
                    + (title_hits * 5)
                    + content_hits
                    + selected_boost;
                let snippets = Self::note_match_snippets(note, &lower_query, &query_terms, 2);
                Some(NoteSearchResult {
                    index,
                    score,
                    snippets,
                })
            })
            .collect::<Vec<_>>();

        matches.sort_by(|left, right| {
            right.score.cmp(&left.score).then_with(|| {
                self.notes[left.index]
                    .title
                    .cmp(&self.notes[right.index].title)
            })
        });
        matches.truncate(limit);
        matches
    }

    pub(super) fn ranked_memory_matches(
        &self,
        query: &str,
        limit: usize,
    ) -> Vec<MemorySearchResult> {
        let cleaned = Self::infer_agent_search_query(query).unwrap_or_default();
        let lower_query = cleaned.to_lowercase();
        let query_terms = Self::agent_search_terms(&lower_query);
        let mut matches = self
            .memories
            .iter()
            .enumerate()
            .filter_map(|(index, memory)| {
                let lower = memory.to_lowercase();
                let phrase_match = !lower_query.is_empty() && lower.contains(&lower_query);
                let term_hits = query_terms
                    .iter()
                    .filter(|term| lower.contains(term.as_str()))
                    .count();
                if !lower_query.is_empty()
                    && !phrase_match
                    && term_hits == 0
                    && !query_terms.is_empty()
                {
                    return None;
                }
                Some(MemorySearchResult {
                    index,
                    score: (usize::from(phrase_match) * 8) + term_hits,
                })
            })
            .collect::<Vec<_>>();

        matches.sort_by(|left, right| {
            right
                .score
                .cmp(&left.score)
                .then_with(|| left.index.cmp(&right.index))
        });
        matches.truncate(limit);
        matches
    }

    pub(super) fn note_match_snippets(
        note: &Note,
        query: &str,
        terms: &[String],
        limit: usize,
    ) -> Vec<String> {
        let mut snippets = Vec::new();
        for line in note.content.lines() {
            let lower = line.to_lowercase();
            let matched = (!query.is_empty() && lower.contains(query))
                || terms.iter().any(|term| lower.contains(term.as_str()));
            if matched {
                snippets.push(Self::preview_text(line.trim(), 220));
            }
            if snippets.len() >= limit {
                break;
            }
        }
        if snippets.is_empty() {
            snippets.push(Self::preview_text(&note.content, 220));
        }
        snippets
    }

    pub(super) fn agent_read_note_response(&self, decision: &AgentDecision) -> String {
        let inferred = decision.search_query.as_deref().and_then(|query| {
            self.ranked_note_matches(query, 1)
                .first()
                .map(|result| result.index)
        });
        let Some(index) = decision
            .note_index
            .or(inferred)
            .or_else(|| self.current_note_index())
        else {
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
            Self::preview_text(&note.content, 2400)
        )
    }

    pub(super) fn agent_save_memory_response(
        &mut self,
        decision: &AgentDecision,
        original_query: &str,
    ) -> String {
        let memory = decision
            .search_query
            .as_deref()
            .unwrap_or(original_query)
            .trim();
        let Some(memory) = Self::normalize_memory_text(memory) else {
            return String::from("I could not find a memory to save.");
        };

        match self.save_memory_text(&memory) {
            Ok(()) => {
                if self.is_strix_connected() {
                    format!("Saved memory locally: {}", memory)
                } else {
                    format!(
                        "Saved memory locally because Strix memories are not connected: {}",
                        memory
                    )
                }
            }
            Err(error) => format!("Memory save failed: {}", error),
        }
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
            .to_string();
        let matches = self.ranked_note_matches(&query, 8);

        if matches.is_empty() {
            return format!("I did not find notes matching `{}`.", query);
        }

        let mut lines = Vec::with_capacity(matches.len() + 1);
        lines.push(format!("Found {} note match(es):", matches.len()));
        for result in matches {
            let note = &self.notes[result.index];
            lines.push(format!(
                "- `#{}` `{}` score={} [{}]\n  {}",
                note.id,
                note.title,
                result.score,
                Self::note_source_label(note),
                result.snippets.join("\n  ")
            ));
        }
        lines.join("\n")
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
            .to_string();
        let matches = self.ranked_memory_matches(&query, 12);

        if matches.is_empty() {
            return format!("I did not find memories matching `{}`.", query);
        }

        let mut lines = Vec::with_capacity(matches.len() + 1);
        lines.push(format!("Found {} memory match(es):", matches.len()));
        lines.extend(matches.into_iter().map(|result| {
            format!(
                "- memory {} score={}: {}",
                result.index + 1,
                result.score,
                self.memories[result.index]
            )
        }));
        lines.join("\n")
    }

    pub(super) fn agent_workspace_status_response(&self) -> String {
        let mut lines = vec![String::from("Workspace status:")];
        lines.extend(self.workspace_context_lines());
        lines.push(String::new());
        lines.push(String::from("Daemon:"));
        lines.extend(
            self.daemon_status_lines()
                .into_iter()
                .take(8)
                .map(|line| format!("- {}", line)),
        );
        lines.join("\n")
    }

    pub(super) fn agent_search_trail_response(
        &self,
        decision: &AgentDecision,
        original_query: &str,
    ) -> String {
        let query = decision
            .search_query
            .as_deref()
            .unwrap_or(original_query)
            .trim()
            .to_string();
        let lines = self.trail_lines(Some(&query));
        if lines
            .iter()
            .any(|line| line == "No trail events match that search.")
        {
            return format!("I did not find trail events matching `{}`.", query);
        }

        let mut output = vec![format!("Trail search for `{}`:", query)];
        output.extend(lines.into_iter().take(16));
        output.join("\n")
    }

    pub(super) fn save_memory_text(&mut self, memory: &str) -> Result<(), String> {
        let memory = memory.trim();
        if memory.is_empty() {
            return Err(String::from("Memory text was empty."));
        }
        if self
            .memories
            .iter()
            .any(|existing| existing.eq_ignore_ascii_case(memory))
        {
            return Ok(());
        }

        self.create_auto_temporal_fork("Before memory save");
        self.memories.push(memory.to_string());
        if let Err(error) = Self::save_local_memories(&self.memories) {
            self.memories.pop();
            return Err(error);
        }
        let _ = self.append_trail_event(
            "memory",
            format!("Saved memory: {}.", Self::preview_text(memory, 80)),
            Vec::new(),
            TrailImportance::High,
        );
        Ok(())
    }

    pub(super) fn normalize_memory_text(text: &str) -> Option<String> {
        let mut memory = text.trim();
        for prefix in [
            "remember that",
            "remember:",
            "remember",
            "save memory:",
            "save memory",
            "memory save",
            "please remember that",
            "please remember",
        ] {
            if memory.to_lowercase().starts_with(prefix) {
                memory = memory[prefix.len()..].trim();
                break;
            }
        }
        let memory = memory.trim_matches(|c: char| c == '"' || c == '\'' || c.is_whitespace());
        (!memory.is_empty()).then(|| memory.chars().take(500).collect())
    }

    pub(super) fn looks_like_memory_save_request(query: &str) -> bool {
        let lower = query.trim_start().to_lowercase();
        [
            "remember that ",
            "remember:",
            "please remember that ",
            "please remember ",
            "save memory ",
            "save memory:",
            "memory save ",
        ]
        .iter()
        .any(|prefix| lower.starts_with(prefix))
    }

    pub(super) fn infer_memory_text_from_request(query: &str) -> Option<String> {
        Self::normalize_memory_text(query)
    }

    pub(super) fn start_note_create_agent(&mut self, query: &str, title: Option<String>) -> bool {
        self.panel_mode = PanelMode::FullEditor;
        self.panel_title = title
            .as_deref()
            .map(|title| format!("Drafting: {}", title))
            .unwrap_or_else(|| String::from("Drafting note"));
        self.panel_lines.clear();
        self.editor_note_index = None;
        self.editor_buffer.clear();
        self.editor_cursor = 0;
        self.editor_scroll_offset = 0;
        self.open_ai_overlay();
        self.ai_draft_create_title = title.clone();
        self.ai_input_buffer = query.trim().to_string();
        self.ai_input_cursor = self.ai_input_buffer.len();
        self.ghost_submit_instruction();
        self.last_action = title
            .as_deref()
            .map(|title| format!("AI is drafting a new note: {}", title))
            .unwrap_or_else(|| String::from("AI is drafting a new note."));
        self.add_activity(
            title
                .as_deref()
                .map(|title| format!("Drafting new note: {}.", title))
                .unwrap_or_else(|| String::from("Drafting new note.")),
        );
        true
    }

    pub(super) fn looks_like_note_create_request(query: &str) -> bool {
        let lower = query.to_lowercase();
        if Self::looks_like_how_to_question(&lower) {
            return false;
        }

        let mentions_note = lower.contains("note")
            || lower.contains("notes")
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
            "write this down as a note",
            "capture this as a note",
        ]
        .iter()
        .any(|needle| lower.contains(needle));
        if direct_note_create {
            return true;
        }

        mentions_note
            && [
                "write ",
                "draft ",
                "compose ",
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
            .any(|prefix| lower.trim_start().starts_with(prefix))
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
        if Self::looks_like_note_create_request(query) {
            return false;
        }
        let mentions_notes = lower.contains("note") || lower.contains("notes");
        let wants_search = [
            "search",
            "find",
            "look through",
            "go through",
            "scan",
            "which notes",
            "notes about",
            "note about",
            "anything about",
            "where did i write",
            "talks about",
            "talk about",
            "similar to",
            "like this",
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

    pub(super) fn looks_like_workspace_status_request(query: &str) -> bool {
        let lower = query.to_lowercase();
        if Self::looks_like_how_to_question(&lower) {
            return false;
        }

        let mentions_workspace = [
            "workspace",
            "computer",
            "machine",
            "repo",
            "repository",
            "git",
            "files",
            "folder",
            "directory",
            "runtime",
            "daemon",
            "status",
        ]
        .iter()
        .any(|needle| lower.contains(needle));
        let asks_inspect = [
            "status",
            "inspect",
            "what is open",
            "what am i working on",
            "where am i",
            "current",
            "changed",
            "dirty",
            "health",
            "diagnose",
            "diagnostic",
        ]
        .iter()
        .any(|needle| lower.contains(needle));

        mentions_workspace && asks_inspect
    }

    pub(super) fn looks_like_trail_search_request(query: &str) -> bool {
        let lower = query.to_lowercase();
        if Self::looks_like_how_to_question(&lower) {
            return false;
        }

        let mentions_trail = [
            "trail",
            "timeline",
            "recent work",
            "recent activity",
            "activity",
            "what happened",
            "last time",
            "before",
        ]
        .iter()
        .any(|needle| lower.contains(needle));
        let wants_search = [
            "search",
            "find",
            "show",
            "review",
            "go through",
            "what",
            "recent",
            "history",
        ]
        .iter()
        .any(|needle| lower.contains(needle));

        mentions_trail && wants_search
    }

    pub(super) fn infer_agent_search_query(query: &str) -> Option<String> {
        let trimmed = query.trim();
        let lower = trimmed.to_lowercase();
        for marker in [
            " about ",
            " on ",
            " for ",
            " matching ",
            " containing ",
            " called ",
            " named ",
            " titled ",
            " talks about ",
            " talk about ",
            " similar to ",
            " like ",
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

        let cleaned = Self::clean_agent_search_query(trimmed);
        (!cleaned.is_empty()).then_some(cleaned)
    }

    pub(super) fn clean_agent_search_query(query: &str) -> String {
        let mut words = query
            .split_whitespace()
            .map(|word| {
                word.trim_matches(|c: char| {
                    c == '"'
                        || c == '\''
                        || c == '`'
                        || c == '.'
                        || c == ','
                        || c == '?'
                        || c == '!'
                        || c == ';'
                        || c == ':'
                })
            })
            .filter(|word| !word.is_empty())
            .collect::<Vec<_>>();

        while let Some(first) = words.first().map(|word| word.to_lowercase()) {
            let drop_count = match first.as_str() {
                "find" | "search" | "scan" | "show" | "open" | "read" => 1,
                "look"
                    if words
                        .get(1)
                        .is_some_and(|word| word.eq_ignore_ascii_case("through")) =>
                {
                    2
                }
                "go" if words
                    .get(1)
                    .is_some_and(|word| word.eq_ignore_ascii_case("through")) =>
                {
                    2
                }
                _ => 0,
            };
            if drop_count == 0 {
                break;
            }
            words.drain(0..drop_count.min(words.len()));
        }

        let mut cleaned = words.join(" ").trim().to_string();
        loop {
            let lower = cleaned.to_lowercase();
            let mut changed = false;
            for prefix in [
                "a note that i have ",
                "the note that i have ",
                "notes that i have ",
                "a note ",
                "the note ",
                "notes ",
                "that i have ",
                "i have ",
                "on ",
                "about ",
                "for ",
                "of ",
            ] {
                if lower.starts_with(prefix) {
                    cleaned = cleaned[prefix.len()..].trim().to_string();
                    changed = true;
                    break;
                }
            }
            if !changed {
                break;
            }
        }

        cleaned
    }

    pub(super) fn agent_search_terms(query: &str) -> Vec<String> {
        query
            .split(|c: char| !c.is_alphanumeric())
            .map(str::to_lowercase)
            .filter(|word| {
                word.len() > 2
                    && !matches!(
                        word.as_str(),
                        "the"
                            | "and"
                            | "from"
                            | "that"
                            | "this"
                            | "have"
                            | "note"
                            | "notes"
                            | "about"
                            | "with"
                            | "for"
                            | "one"
                            | "not"
                            | "exact"
                            | "stuff"
                            | "like"
                    )
            })
            .collect()
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
            String::from("Untitled note")
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
        self.add_activity(format!("Reading note: {}.", self.notes[index].title));
        self.add_activity("Preparing AI edit proposal.");
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
        if Self::looks_like_note_create_request(query) {
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

    pub(super) fn looks_like_direct_smalltalk(query: &str) -> bool {
        let lower = query
            .trim()
            .trim_matches(|character: char| matches!(character, '?' | '!' | '.' | ','))
            .to_lowercase();
        [
            "hi",
            "hello",
            "hey",
            "how are you",
            "how's it going",
            "whats up",
            "what's up",
            "who are you",
            "what can you do",
            "tell me about yourself",
            "good morning",
            "good afternoon",
            "good evening",
        ]
        .iter()
        .any(|needle| lower == *needle || lower.starts_with(&format!("{} ", needle)))
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
