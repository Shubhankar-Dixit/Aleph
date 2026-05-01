use super::*;

#[allow(dead_code)]
impl App {
    pub(super) fn ghost_submit_instruction(&mut self) {
        let instruction = self.ai_input_buffer.trim().to_string();
        if instruction.is_empty() {
            return;
        }

        let provider = self.ai_provider;
        let openrouter_api_key = self.openrouter_api_key.clone();
        let strix_access_token = self.strix_access_token.clone();
        let draft_create_title = self.ai_draft_create_title.clone();

        let editor_content = self.editor_buffer.clone();
        self.ghost_streaming = true;
        self.ghost_result = None;
        self.pending_ai_edit = None;
        self.thinking = true;
        self.thinking_status = String::from("Aleph is editing...");
        self.thinking_ticks_remaining = 20;

        // Build a conversation for the ghost editor
        let system_prompt = String::from(
            "You are Aleph's note-writing agent. You operate inside a Markdown note editor, not a chat window. \
             The user gives current note content plus an instruction. Return ONLY the complete note content that should exist after the action. \
             No explanations, no preamble, no code fences, no JSON, no commentary about what you changed. \
             For a new empty note, write a useful complete draft with a concrete title heading only when it improves the note. \
             For edits, preserve the user's meaning, useful details, markdown structure, links, lists, and factual claims unless the instruction asks to change them. \
             For append/add/insert requests, integrate the requested material into the note instead of replacing unrelated content. \
             For rewrite/improve/fix/clean-up requests, make the smallest coherent full-note rewrite that satisfies the instruction. \
             If the instruction is ambiguous, choose the most likely writing/editing action and produce the resulting note content."
        );

        let title_context = draft_create_title
            .as_deref()
            .map(|title| format!("New note title: {}\n\n", title))
            .unwrap_or_default();
        let user_msg = format!(
            "{}Current note content:\n---\n{}\n---\n\nInstruction: {}",
            title_context, editor_content, instruction
        );
        let strix_instruction = format!("{}\n\n{}", system_prompt, user_msg);

        let conversation = vec![
            (String::from("system"), system_prompt),
            (String::from("user"), user_msg),
        ];

        let (sender, receiver) = mpsc::channel();
        self.ghost_stream_rx = Some(receiver);

        match provider {
            AiProvider::OpenRouter => {
                let Some(api_key) = openrouter_api_key else {
                    self.ghost_result = Some(String::from("OpenRouter is not configured as a model provider. Run /login openrouter first."));
                    self.ghost_streaming = false;
                    self.thinking = false;
                    self.thinking_status.clear();
                    self.thinking_ticks_remaining = 0;
                    self.ghost_stream_rx = None;
                    return;
                };
                thread::spawn(move || {
                    if let Err(error) = Self::send_openrouter_chat_streaming(
                        &api_key,
                        &conversation,
                        sender.clone(),
                    ) {
                        let _ = sender.send(ChatStreamUpdate::Error(error));
                    }
                });
            }
            AiProvider::Strix => {
                let Some(access_token) = strix_access_token else {
                    self.ghost_result = Some(String::from(
                        "Strix is not connected. Run /login strix first.",
                    ));
                    self.ghost_streaming = false;
                    self.thinking = false;
                    self.thinking_status.clear();
                    self.thinking_ticks_remaining = 0;
                    self.ghost_stream_rx = None;
                    return;
                };
                let base_url = Self::strix_api_base_url();
                let notes = vec![Note {
                    id: 1,
                    remote_id: None,
                    obsidian_path: None,
                    title: String::from("Current note"),
                    content: editor_content,
                    raw_content: String::new(),
                    updated_at: String::new(),
                    folder_id: None,
                }];
                thread::spawn(move || {
                    if let Err(error) = Self::send_strix_chat(
                        &base_url,
                        &access_token,
                        &strix_instruction,
                        &notes,
                        sender.clone(),
                    ) {
                        let _ = sender.send(ChatStreamUpdate::Error(error));
                    }
                });
            }
        }

        self.ai_input_buffer.clear();
        self.ai_input_cursor = 0;
    }

    pub fn process_ghost_stream(&mut self) {
        let mut finished = false;
        while !finished {
            let result = match self.ghost_stream_rx.as_ref() {
                Some(receiver) => receiver.try_recv(),
                None => break,
            };

            match result {
                Ok(ChatStreamUpdate::Delta(chunk)) => {
                    if self.ghost_result.is_none() {
                        self.ghost_result = Some(String::new());
                    }
                    if let Some(ref mut buf) = self.ghost_result {
                        buf.push_str(&chunk);
                    }
                    self.thinking = true;
                    self.thinking_status = String::from("Aleph is editing...");
                }
                Ok(ChatStreamUpdate::Done) => {
                    self.ghost_streaming = false;
                    self.thinking = false;
                    self.thinking_status.clear();
                    self.thinking_ticks_remaining = 0;
                    self.ghost_stream_rx = None;

                    if let Some(ref result) = self.ghost_result {
                        let proposed = result.trim().to_string();
                        if proposed.is_empty() {
                            self.ghost_result =
                                Some(String::from("AI returned an empty proposal."));
                            self.last_action = String::from("AI note edit returned no changes.");
                        } else if proposed == self.editor_buffer {
                            self.ghost_result = Some(String::from("No changes proposed."));
                            self.last_action = String::from("AI note edit found no changes.");
                        } else if let Some(title) = self.ai_draft_create_title.clone() {
                            let diff_lines = Self::build_line_diff("", &proposed);
                            self.pending_ai_edit = Some(AiEditProposal {
                                note_index: None,
                                title: Some(title),
                                instruction: String::from("AI note create"),
                                proposed,
                                diff_lines,
                            });
                            self.last_action = String::from(
                                "AI drafted a note. Press Enter to create it or Ctrl+R to reject.",
                            );
                        } else if let Some(note_index) = self.editor_note_index {
                            let diff_lines = Self::build_line_diff(&self.editor_buffer, &proposed);
                            self.pending_ai_edit = Some(AiEditProposal {
                                note_index: Some(note_index),
                                title: None,
                                instruction: String::from("AI note edit"),
                                proposed,
                                diff_lines,
                            });
                            self.last_action = String::from(
                                "AI proposed edits. Press Enter to apply or Ctrl+R to reject.",
                            );
                        }
                    }
                    finished = true;
                }
                Ok(ChatStreamUpdate::Error(error)) => {
                    self.ghost_result = Some(format!("Error: {}", error));
                    self.ghost_streaming = false;
                    self.thinking = false;
                    self.thinking_status.clear();
                    self.thinking_ticks_remaining = 0;
                    self.ghost_stream_rx = None;
                    self.last_action = String::from("Ghost request failed.");
                    finished = true;
                }
                Err(TryRecvError::Empty) => {
                    self.thinking = true;
                    self.thinking_status = String::from("Aleph is editing...");
                    break;
                }
                Err(TryRecvError::Disconnected) => {
                    self.ghost_result = Some(String::from("Ghost disconnected."));
                    self.ghost_streaming = false;
                    self.thinking = false;
                    self.thinking_status.clear();
                    self.thinking_ticks_remaining = 0;
                    self.ghost_stream_rx = None;
                    finished = true;
                }
            }
        }
    }

    pub(super) fn apply_pending_ai_edit(&mut self) {
        let Some(proposal) = self.pending_ai_edit.take() else {
            return;
        };

        let Some(note_index) = proposal.note_index else {
            let title = proposal
                .title
                .clone()
                .unwrap_or_else(|| String::from("AI draft"));
            match self.create_note_from_content(&title, &proposal.proposed) {
                Ok(index) => {
                    self.selected_note = index;
                    self.open_note_editor(index);
                    self.ghost_result = None;
                    self.last_action = format!("Created AI note: {}", title);
                    self.close_ai_overlay();
                }
                Err(error) => {
                    self.ghost_result = Some(format!("Create failed: {}", error));
                    self.last_action = String::from("AI note create failed.");
                }
            }
            return;
        };

        if self.editor_note_index != Some(note_index) {
            self.open_note_editor(note_index);
        }

        self.save_undo_state();
        self.editor_buffer = proposal.proposed;
        self.editor_cursor = self.editor_buffer.len();
        self.ghost_result = None;
        self.save_editor();
        self.last_action = String::from("Applied AI note edits.");
        self.close_ai_overlay();
    }

    pub(super) fn reject_pending_ai_edit(&mut self) {
        self.pending_ai_edit = None;
        self.ghost_result = None;
        self.ghost_streaming = false;
        self.ghost_stream_rx = None;
        self.thinking = false;
        self.thinking_ticks_remaining = 0;
        self.last_action = String::from("Rejected AI note edits.");
    }

    pub(super) fn build_line_diff(original: &str, proposed: &str) -> Vec<String> {
        let old_lines: Vec<&str> = original.lines().collect();
        let new_lines: Vec<&str> = proposed.lines().collect();
        let mut table = vec![vec![0usize; new_lines.len() + 1]; old_lines.len() + 1];

        for old_index in (0..old_lines.len()).rev() {
            for new_index in (0..new_lines.len()).rev() {
                table[old_index][new_index] = if old_lines[old_index] == new_lines[new_index] {
                    table[old_index + 1][new_index + 1] + 1
                } else {
                    table[old_index + 1][new_index].max(table[old_index][new_index + 1])
                };
            }
        }

        let mut diff = Vec::new();
        let mut old_index = 0;
        let mut new_index = 0;
        while old_index < old_lines.len() && new_index < new_lines.len() {
            if old_lines[old_index] == new_lines[new_index] {
                diff.push(format!("  {}", old_lines[old_index]));
                old_index += 1;
                new_index += 1;
            } else if table[old_index + 1][new_index] >= table[old_index][new_index + 1] {
                diff.push(format!("- {}", old_lines[old_index]));
                old_index += 1;
            } else {
                diff.push(format!("+ {}", new_lines[new_index]));
                new_index += 1;
            }
        }
        while old_index < old_lines.len() {
            diff.push(format!("- {}", old_lines[old_index]));
            old_index += 1;
        }
        while new_index < new_lines.len() {
            diff.push(format!("+ {}", new_lines[new_index]));
            new_index += 1;
        }

        if diff.is_empty() && !proposed.is_empty() {
            diff.push(format!("+ {}", proposed));
        }
        diff
    }
}
