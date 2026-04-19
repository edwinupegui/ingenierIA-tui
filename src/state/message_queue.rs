// ── MessageQueue — Cola de mensajes durante streaming ────────────────────────

use std::collections::VecDeque;

const MAX_QUEUE_SIZE: usize = 10;

pub struct QueuedMessage {
    pub content: String,
}

/// Multi-message queue for chat input during AI streaming.
///
/// `draft` holds what the user is currently typing.
/// `items` holds committed messages (Enter during streaming).
/// On stream done, items drain as new turns; leftover draft becomes input.
pub struct MessageQueue {
    items: VecDeque<QueuedMessage>,
    draft: String,
    max_size: usize,
}

impl MessageQueue {
    pub fn new() -> Self {
        Self { items: VecDeque::new(), draft: String::new(), max_size: MAX_QUEUE_SIZE }
    }

    // ── Draft access ────────────────────────────────────────────────────────

    pub fn draft(&self) -> &str {
        &self.draft
    }

    pub fn draft_mut(&mut self) -> &mut String {
        &mut self.draft
    }

    pub fn push_draft_char(&mut self, c: char) {
        self.draft.push(c);
    }

    pub fn pop_draft_char(&mut self) {
        self.draft.pop();
    }

    pub fn append_to_draft(&mut self, text: &str) {
        self.draft.push_str(text);
    }

    pub fn is_draft_empty(&self) -> bool {
        self.draft.is_empty()
    }

    // ── Queue operations ────────────────────────────────────────────────────

    /// Move non-empty draft into the queue as a committed message.
    pub fn commit_draft(&mut self) {
        if self.draft.is_empty() {
            return;
        }
        if self.items.len() >= self.max_size {
            self.items.pop_front();
        }
        let content = std::mem::take(&mut self.draft);
        self.items.push_back(QueuedMessage { content });
    }

    /// Remove and return the first committed message (for auto-chain).
    pub fn pop_front(&mut self) -> Option<QueuedMessage> {
        self.items.pop_front()
    }

    /// Remove and return the last committed message (Escape undo).
    pub fn pop_back(&mut self) -> Option<QueuedMessage> {
        self.items.pop_back()
    }

    /// If no committed items remain, take the draft as input text.
    pub fn drain_to_input(&mut self) -> Option<String> {
        if self.items.is_empty() && !self.draft.is_empty() {
            Some(std::mem::take(&mut self.draft))
        } else {
            None
        }
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn has_items(&self) -> bool {
        !self.items.is_empty()
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty() && self.draft.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commit_and_drain() {
        let mut q = MessageQueue::new();
        q.push_draft_char('h');
        q.push_draft_char('i');
        assert_eq!(q.draft(), "hi");
        q.commit_draft();
        assert!(q.is_draft_empty());
        assert_eq!(q.len(), 1);
        let msg = q.pop_front().expect("should have 1 item");
        assert_eq!(msg.content, "hi");
        assert!(q.is_empty());
    }

    #[test]
    fn pop_back_undo() {
        let mut q = MessageQueue::new();
        q.append_to_draft("first");
        q.commit_draft();
        q.append_to_draft("second");
        q.commit_draft();
        assert_eq!(q.len(), 2);
        let undone = q.pop_back().expect("should pop");
        assert_eq!(undone.content, "second");
        assert_eq!(q.len(), 1);
    }

    #[test]
    fn max_size_evicts_oldest() {
        let mut q = MessageQueue::new();
        for i in 0..12 {
            q.append_to_draft(&format!("msg{i}"));
            q.commit_draft();
        }
        assert_eq!(q.len(), MAX_QUEUE_SIZE);
        let first = q.pop_front().expect("should have items");
        assert_eq!(first.content, "msg2"); // 0 and 1 were evicted
    }

    #[test]
    fn drain_to_input_only_draft() {
        let mut q = MessageQueue::new();
        q.append_to_draft("pending");
        assert_eq!(q.drain_to_input(), Some("pending".to_string()));
        assert!(q.is_empty());
    }

    #[test]
    fn drain_to_input_with_items_returns_none() {
        let mut q = MessageQueue::new();
        q.append_to_draft("queued");
        q.commit_draft();
        q.append_to_draft("draft");
        assert_eq!(q.drain_to_input(), None);
    }

    #[test]
    fn commit_empty_draft_is_noop() {
        let mut q = MessageQueue::new();
        q.commit_draft();
        assert_eq!(q.len(), 0);
    }
}
