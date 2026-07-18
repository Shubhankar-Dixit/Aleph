use super::*;
use ratatui::prelude::Rect;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

impl App {
    pub(crate) fn cached_transcript_message_lines(
        &self,
        message: &ChatMessage,
        render: impl FnOnce() -> Vec<ratatui::prelude::Line<'static>>,
    ) -> Vec<ratatui::prelude::Line<'static>> {
        let mut hasher = DefaultHasher::new();
        message.id.hash(&mut hasher);
        message.role.hash(&mut hasher);
        message.content.hash(&mut hasher);
        message.timestamp.hash(&mut hasher);
        message.thought_seconds.map(f32::to_bits).hash(&mut hasher);
        message.turn_seconds.map(f32::to_bits).hash(&mut hasher);
        if message.content.trim().is_empty() {
            self.thinking.hash(&mut hasher);
            self.streaming_active.hash(&mut hasher);
            self.thinking_status.hash(&mut hasher);
        }
        let fingerprint = hasher.finish();

        if let Some(cached) = self.transcript_message_cache.borrow().get(&message.id) {
            if cached.fingerprint == fingerprint {
                return cached.lines.clone();
            }
        }

        let lines = render();
        let mut cache = self.transcript_message_cache.borrow_mut();
        if cache.len() > MAX_CHAT_MESSAGES.saturating_mul(2) {
            cache.clear();
        }
        cache.insert(
            message.id,
            CachedTranscriptMessage {
                fingerprint,
                lines: lines.clone(),
            },
        );
        lines
    }

    pub(super) fn note_transcript_activity(&mut self) {
        if matches!(
            self.transcript_viewport.mode,
            TranscriptViewportMode::Anchored(_)
        ) {
            self.transcript_viewport.new_activity = true;
        }
    }

    pub(super) fn follow_chat_tail(&mut self) {
        self.transcript_viewport.mode = TranscriptViewportMode::FollowTail;
        self.transcript_viewport.new_activity = false;
    }

    pub(super) fn scroll_chat_by(&mut self, rows: isize) {
        let area = crossterm::terminal::size()
            .map(|(width, height)| Rect::new(0, 0, width, height))
            .unwrap_or(Rect::new(0, 0, 80, 24));
        self.scroll_chat_by_in_area(rows, area);
    }

    pub(super) fn scroll_chat_by_in_area(&mut self, rows: isize, area: Rect) {
        let layout = crate::ui::chat_transcript_layout(self, area);
        if layout.max_top() == 0 {
            self.follow_chat_tail();
            return;
        }
        let current_top = match self.transcript_viewport.mode {
            TranscriptViewportMode::FollowTail => layout.max_top(),
            TranscriptViewportMode::Anchored(anchor) => layout.resolve_anchor(anchor),
        };
        let next_top = if rows < 0 {
            current_top.saturating_sub(rows.unsigned_abs())
        } else {
            current_top
                .saturating_add(rows as usize)
                .min(layout.max_top())
        };

        if next_top >= layout.max_top() && rows >= 0 {
            self.follow_chat_tail();
        } else {
            self.transcript_viewport.mode =
                TranscriptViewportMode::Anchored(layout.anchor_at(next_top));
        }
    }

    pub(crate) fn transcript_top_row(&self, layout: &TranscriptLayoutSnapshot) -> usize {
        match self.transcript_viewport.mode {
            TranscriptViewportMode::FollowTail => layout.max_top(),
            TranscriptViewportMode::Anchored(anchor) => {
                layout.resolve_anchor(anchor).min(layout.max_top())
            }
        }
    }
}
