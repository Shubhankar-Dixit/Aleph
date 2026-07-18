use super::*;

#[allow(dead_code)]
impl App {
    pub(super) fn begin_run(&mut self, request: &str, phase: RunPhase) -> u64 {
        if let Some(run) = self.active_agent_run() {
            if !run.phase.is_terminal() {
                return run.id;
            }
        }

        let id = self.next_run_id;
        self.next_run_id = self.next_run_id.saturating_add(1);
        let context = self.capture_run_context(request);
        self.agent_runs.push(AgentRun {
            id,
            request: request.trim().to_string(),
            phase,
            context,
            steps: Vec::new(),
            approval: None,
            changes: Vec::new(),
            outcome: None,
        });
        if self.agent_runs.len() > MAX_CHAT_MESSAGES {
            let overflow = self.agent_runs.len() - MAX_CHAT_MESSAGES;
            self.agent_runs.drain(0..overflow);
        }
        self.active_run_id = Some(id);
        id
    }

    pub(super) fn transition_run(&mut self, next: RunPhase) -> Result<(), String> {
        let run = self.active_run_mut()?;
        if run.phase == next {
            return Ok(());
        }
        if !run.phase.can_transition_to(next) {
            return Err(format!(
                "invalid agent run transition: {:?} -> {:?}",
                run.phase, next
            ));
        }
        run.phase = next;
        self.note_transcript_activity();
        Ok(())
    }

    pub(super) fn queue_run_steps(
        &mut self,
        steps: impl IntoIterator<Item = (String, Option<String>)>,
    ) -> Result<(), String> {
        let run = self.active_run_mut()?;
        run.steps
            .extend(steps.into_iter().map(|(label, target)| RunStep {
                label,
                target,
                status: StepStatus::Pending,
                summary: None,
                error: None,
            }));
        self.note_transcript_activity();
        Ok(())
    }

    pub(super) fn start_queued_step(&mut self, index: usize) -> Result<(), String> {
        if self
            .active_agent_run()
            .is_some_and(|run| run.phase == RunPhase::Planning)
        {
            self.transition_run(RunPhase::Acting)?;
        }
        let step = self
            .active_run_mut()?
            .steps
            .get_mut(index)
            .ok_or_else(|| format!("unknown run step {}", index))?;
        if step.status != StepStatus::Pending {
            return Err(format!("run step {} is not pending", index));
        }
        step.status = StepStatus::Running;
        self.note_transcript_activity();
        Ok(())
    }

    pub(super) fn start_step(
        &mut self,
        label: impl Into<String>,
        target: Option<String>,
    ) -> Result<usize, String> {
        if self
            .active_agent_run()
            .is_some_and(|run| run.phase == RunPhase::Planning)
        {
            self.transition_run(RunPhase::Acting)?;
        }
        let run = self.active_run_mut()?;
        let index = run.steps.len();
        run.steps.push(RunStep {
            label: label.into(),
            target,
            status: StepStatus::Running,
            summary: None,
            error: None,
        });
        self.note_transcript_activity();
        Ok(index)
    }

    pub(super) fn complete_step(
        &mut self,
        index: usize,
        summary: impl Into<String>,
    ) -> Result<(), String> {
        let step = self
            .active_run_mut()?
            .steps
            .get_mut(index)
            .ok_or_else(|| format!("unknown run step {}", index))?;
        step.status = StepStatus::Completed;
        step.summary = Some(summary.into());
        step.error = None;
        self.note_transcript_activity();
        Ok(())
    }

    pub(super) fn fail_step(
        &mut self,
        index: usize,
        error: impl Into<String>,
    ) -> Result<(), String> {
        let error = error.into();
        let step = self
            .active_run_mut()?
            .steps
            .get_mut(index)
            .ok_or_else(|| format!("unknown run step {}", index))?;
        step.status = StepStatus::Failed;
        step.error = Some(error.clone());
        self.fail_run(error)
    }

    pub(super) fn request_approval(
        &mut self,
        request: ApprovalRequest,
        change: RunChange,
    ) -> Result<(), String> {
        self.transition_run(RunPhase::WaitingApproval)?;
        let run = self.active_run_mut()?;
        run.approval = Some(request);
        run.changes.push(change);
        self.chat_composer
            .set_interaction(ComposerInteraction::Approval);
        self.note_transcript_activity();
        Ok(())
    }

    pub(super) fn approve_request(&mut self) -> Result<(), String> {
        let run = self.active_run_mut()?;
        if run.phase != RunPhase::WaitingApproval || run.approval.is_none() {
            return Err(String::from("no agent approval is pending"));
        }
        run.approval = None;
        run.phase = RunPhase::Acting;
        self.chat_composer
            .set_interaction(ComposerInteraction::Editing);
        self.note_transcript_activity();
        Ok(())
    }

    pub(super) fn reject_request(&mut self, reason: impl Into<String>) -> Result<(), String> {
        let reason = reason.into();
        let run = self.active_run_mut()?;
        for change in &mut run.changes {
            if change.status == ChangeStatus::Proposed {
                change.status = ChangeStatus::Rejected;
            }
        }
        run.approval = None;
        run.phase = RunPhase::Cancelled;
        run.outcome = Some(RunOutcome::Cancelled { reason });
        self.chat_composer
            .set_interaction(ComposerInteraction::Editing);
        self.terminate_pending_execution();
        self.note_transcript_activity();
        Ok(())
    }

    pub(super) fn complete_run(&mut self, summary: impl Into<String>) -> Result<(), String> {
        let run = self.active_run_mut()?;
        if run.phase.is_terminal() {
            return Err(String::from("agent run is already finished"));
        }
        run.approval = None;
        run.phase = RunPhase::Completed;
        run.outcome = Some(RunOutcome::Completed {
            summary: summary.into(),
        });
        self.chat_composer
            .set_interaction(ComposerInteraction::Editing);
        self.terminate_pending_execution();
        self.note_transcript_activity();
        Ok(())
    }

    pub(super) fn fail_run(&mut self, error: impl Into<String>) -> Result<(), String> {
        let error = error.into();
        let run = self.active_run_mut()?;
        if run.phase.is_terminal() {
            return Err(String::from("agent run is already finished"));
        }
        run.approval = None;
        run.phase = RunPhase::Failed;
        run.outcome = Some(RunOutcome::Failed { error });
        self.chat_composer
            .set_interaction(ComposerInteraction::Editing);
        self.terminate_pending_execution();
        self.note_transcript_activity();
        Ok(())
    }

    pub(super) fn cancel_run(&mut self, reason: impl Into<String>) -> Result<(), String> {
        let reason = reason.into();
        let run = self.active_run_mut()?;
        if run.phase.is_terminal() {
            return Err(String::from("agent run is already finished"));
        }
        run.approval = None;
        run.phase = RunPhase::Cancelled;
        run.outcome = Some(RunOutcome::Cancelled { reason });
        self.chat_composer
            .set_interaction(ComposerInteraction::Editing);
        self.terminate_pending_execution();
        self.note_transcript_activity();
        Ok(())
    }

    pub(super) fn add_run_change(&mut self, change: RunChange) -> Result<(), String> {
        self.active_run_mut()?.changes.push(change);
        self.note_transcript_activity();
        Ok(())
    }

    pub(super) fn mark_proposed_changes(&mut self, status: ChangeStatus) -> Result<(), String> {
        for change in &mut self.active_run_mut()?.changes {
            if change.status == ChangeStatus::Proposed {
                change.status = status;
            }
        }
        self.note_transcript_activity();
        Ok(())
    }

    fn active_run_mut(&mut self) -> Result<&mut AgentRun, String> {
        let id = self
            .active_run_id
            .ok_or_else(|| String::from("no active agent run"))?;
        self.agent_runs
            .iter_mut()
            .find(|run| run.id == id)
            .ok_or_else(|| format!("active agent run {} is missing", id))
    }

    fn capture_run_context(&self, request: &str) -> RunContextSnapshot {
        let live_repo = match self.agent_context_scope {
            AgentContextScope::CurrentFolder => Self::capture_repo_context(),
            AgentContextScope::ActiveRoom => self
                .active_room_ref()
                .project_paths
                .iter()
                .map(Path::new)
                .find(|path| path.exists() && path.is_dir())
                .and_then(Self::capture_repo_context_for_path),
            AgentContextScope::Global => None,
        };
        let (repository_source, repository_summary) = if let Some(repo) = live_repo {
            (
                RepositoryContextSource::Live,
                Some(Self::run_repo_summary(&repo)),
            )
        } else if let Some(repo) = self.current_repo_context() {
            (
                RepositoryContextSource::Snapshot,
                Some(Self::run_repo_summary(repo)),
            )
        } else {
            (RepositoryContextSource::Unavailable, None)
        };
        let provider_online = match self.ai_provider {
            AiProvider::OpenRouter => self.is_openrouter_connected(),
            AiProvider::Strix => self.is_strix_connected(),
        };
        let relevant_notes = self
            .ranked_note_matches(request, 4)
            .into_iter()
            .filter_map(|result| self.notes.get(result.index))
            .map(|note| note.title.clone())
            .collect();
        let relevant_memories = self.ranked_memory_matches(request, 8).len();
        let relevant_trail_events = self.trail_lines(Some(request)).len().min(8);

        RunContextSnapshot {
            scope: self.agent_context_scope_label().to_string(),
            room: self.active_room_label().to_string(),
            room_summary: self.room_scope_summary(),
            selected_note: self.active_note().map(|note| note.title.clone()),
            relevant_notes,
            relevant_memories,
            relevant_trail_events,
            notes_available: self.room_note_count(),
            memories_available: self.memories.len(),
            local_tools_available: true,
            provider: self.ai_provider_label().to_string(),
            provider_online,
            repository_source,
            repository_summary,
        }
    }

    fn run_repo_summary(repo: &RepoContext) -> String {
        let branch = repo.branch.as_deref().unwrap_or("detached");
        if repo.dirty_files.is_empty() {
            format!("{} · clean", branch)
        } else {
            format!("{} · {} dirty", branch, repo.dirty_files.len())
        }
    }
}
