/// StatusIcon — Semantic icon with automatic color by status.
///
/// Reference: claude-code StatusIcon.tsx, theme.rs glyphs.
use ratatui::{style::Style, text::Span};

use crate::theme::{cyan, dim, green, red, yellow};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusIcon {
    Success,
    Error,
    Warning,
    Pending,
    Idle,
    Thinking,
    Checking,
}

impl StatusIcon {
    pub fn glyph(self) -> &'static str {
        match self {
            Self::Success => "✓",
            Self::Error => "✗",
            Self::Warning => "⚠",
            Self::Pending => "●",
            Self::Idle => "○",
            Self::Thinking => "∴",
            Self::Checking => "◌",
        }
    }

    pub fn color(self) -> ratatui::style::Color {
        match self {
            Self::Success => green(),
            Self::Error => red(),
            Self::Warning => yellow(),
            Self::Pending | Self::Checking => cyan(),
            Self::Idle => dim(),
            Self::Thinking => cyan(),
        }
    }

    pub fn to_span(self) -> Span<'static> {
        Span::styled(self.glyph(), Style::default().fg(self.color()))
    }
}
