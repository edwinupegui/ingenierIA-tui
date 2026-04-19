/// KeyboardHint — Shortcut + action display pair.
///
/// Reference: claude-code KeyboardShortcutHint.tsx.
/// Renders as: "ctrl+k" (dim()) + " command palette" (dimmer()).
use ratatui::{style::Style, text::Span};

use crate::theme::{dim, dimmer};

pub struct KeyboardHint;

impl KeyboardHint {
    /// Build a pair of spans: key (dim()) + action (dimmer()) with trailing space.
    pub fn spans(key: &str, action: &str) -> Vec<Span<'static>> {
        vec![
            Span::styled(format!("{key} "), Style::default().fg(dim())),
            Span::styled(action.to_string(), Style::default().fg(dimmer())),
            Span::styled("  ", Style::default()),
        ]
    }
}
