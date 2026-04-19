//! MarkdownStreamState — safe-boundary buffer for incremental rendering.
//!
//! As streaming deltas arrive, content is split into:
//! - `stable_prefix`: fully formed blocks that won't change (not re-parsed)
//! - `volatile_suffix`: the tail that may still be modified by incoming deltas

/// State machine for streaming markdown content.
pub struct MarkdownStreamState {
    content: String,
    stable_end: usize,
}

impl MarkdownStreamState {
    pub fn new() -> Self {
        Self { content: String::new(), stable_end: 0 }
    }

    /// Append a streaming delta and recalculate the safe boundary.
    pub fn push(&mut self, delta: &str) {
        self.content.push_str(delta);
        self.stable_end = find_safe_boundary(&self.content);
    }

    /// The stable prefix that won't change — skip re-parsing this portion.
    pub fn stable_prefix(&self) -> &str {
        &self.content[..self.stable_end]
    }

    /// The volatile suffix that may still change — re-parse only this part.
    pub fn volatile_suffix(&self) -> &str {
        &self.content[self.stable_end..]
    }

    /// Full content (stable + volatile).
    pub fn content(&self) -> &str {
        &self.content
    }

    /// Finalize: mark all content as stable (streaming done).
    pub fn finalize(&mut self) {
        self.stable_end = self.content.len();
    }

    /// Reset state for a new message.
    pub fn reset(&mut self) {
        self.content.clear();
        self.stable_end = 0;
    }

    /// Whether the content is inside an unclosed code fence.
    pub fn is_in_code_block(&self) -> bool {
        let fence_count = self.content.matches("```").count();
        !fence_count.is_multiple_of(2)
    }
}

impl Default for MarkdownStreamState {
    fn default() -> Self {
        Self::new()
    }
}

/// Find the byte offset of the last safe boundary in content.
///
/// Safe boundaries (points where it's safe to stop parsing):
/// - End of paragraph (double newline)
/// - Before unclosed code fence (triple backtick)
/// - End of heading (single newline after # line)
fn find_safe_boundary(content: &str) -> usize {
    let fence_count = content.matches("```").count();
    let in_unclosed_fence = !fence_count.is_multiple_of(2);

    if in_unclosed_fence {
        // Split before the unclosed fence
        if let Some(pos) = content.rfind("```") {
            return pos;
        }
    }

    // Find the last empty line (paragraph boundary)
    let bytes = content.as_bytes();
    let mut last_boundary = 0;
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] == b'\n' && i + 1 < bytes.len() && bytes[i + 1] == b'\n' {
            last_boundary = i + 2;
            i += 2;
            continue;
        }
        i += 1;
    }

    last_boundary
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_state() {
        let state = MarkdownStreamState::new();
        assert_eq!(state.content(), "");
        assert_eq!(state.stable_prefix(), "");
        assert_eq!(state.volatile_suffix(), "");
    }

    #[test]
    fn push_without_boundary_stays_volatile() {
        let mut state = MarkdownStreamState::new();
        state.push("Hello world");
        assert_eq!(state.stable_prefix(), "");
        assert_eq!(state.volatile_suffix(), "Hello world");
    }

    #[test]
    fn paragraph_break_creates_stable_prefix() {
        let mut state = MarkdownStreamState::new();
        state.push("First paragraph.\n\nSecond start");
        assert_eq!(state.stable_prefix(), "First paragraph.\n\n");
        assert_eq!(state.volatile_suffix(), "Second start");
    }

    #[test]
    fn code_fence_splits_before_unclosed() {
        let mut state = MarkdownStreamState::new();
        state.push("Some text\n\n```rust\nfn main() {");
        assert!(state.is_in_code_block());
        // Stable prefix is content before the unclosed fence
        assert!(state.stable_prefix().ends_with('\n'));
    }

    #[test]
    fn finalize_marks_all_stable() {
        let mut state = MarkdownStreamState::new();
        state.push("Partial content");
        state.finalize();
        assert_eq!(state.stable_prefix(), "Partial content");
        assert_eq!(state.volatile_suffix(), "");
    }

    #[test]
    fn reset_clears_all() {
        let mut state = MarkdownStreamState::new();
        state.push("Some content\n\nMore");
        state.reset();
        assert_eq!(state.content(), "");
    }
}
