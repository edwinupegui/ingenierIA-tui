/// Dialog — Centered modal overlay with title, body area, and action hints.
///
/// Reference: claude-code Dialog.tsx with cancel/confirm + exit state.
/// Use for permission modals, confirmations, and interactive overlays.
use ratatui::{layout::Rect, style::Style, Frame};

use super::pane::Pane;

pub struct Dialog<'a> {
    title: &'a str,
    border_style: Style,
    width_pct: u16,
    height_pct: u16,
}

impl<'a> Dialog<'a> {
    pub fn new(title: &'a str) -> Self {
        Self { title, border_style: Style::default(), width_pct: 60, height_pct: 50 }
    }

    pub fn border_style(mut self, style: Style) -> Self {
        self.border_style = style;
        self
    }

    pub fn width_pct(mut self, pct: u16) -> Self {
        self.width_pct = pct;
        self
    }

    pub fn height_pct(mut self, pct: u16) -> Self {
        self.height_pct = pct;
        self
    }

    /// Compute the centered rect for the dialog within the parent area.
    fn centered_rect(&self, parent: Rect) -> Rect {
        let w = (parent.width * self.width_pct / 100).clamp(40, parent.width.saturating_sub(4));
        let h = (parent.height * self.height_pct / 100).clamp(6, parent.height.saturating_sub(4));
        Rect {
            x: parent.x + (parent.width.saturating_sub(w)) / 2,
            y: parent.y + (parent.height.saturating_sub(h)) / 2,
            width: w,
            height: h,
        }
    }

    /// Render the dialog and return the inner content area.
    pub fn render(self, f: &mut Frame, parent: Rect) -> Rect {
        let dialog_rect = self.centered_rect(parent);
        Pane::new().title(self.title).border_style(self.border_style).render(f, dialog_rect)
    }
}
