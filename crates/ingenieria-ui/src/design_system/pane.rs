/// Pane — Bordered container with optional title and semantic styling.
///
/// Reference: claude-code design-system/Pane, claw-code "╭─ label" / "╰─".
/// Use for all top-level overlay containers instead of raw Block::default().
use ratatui::{
    layout::Rect,
    style::Style,
    widgets::{Block, BorderType, Borders, Clear, Padding},
    Frame,
};

use super::tokens::border;
use crate::theme::surface;

pub struct Pane<'a> {
    title: Option<&'a str>,
    border_style: Style,
    border_type: BorderType,
    padding: Padding,
    clear_bg: bool,
}

impl Default for Pane<'_> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> Pane<'a> {
    pub fn new() -> Self {
        Self {
            title: None,
            border_style: Style::default(),
            border_type: border::DEFAULT,
            padding: Padding::new(1, 1, 0, 0),
            clear_bg: true,
        }
    }

    pub fn title(mut self, title: &'a str) -> Self {
        self.title = Some(title);
        self
    }

    pub fn border_style(mut self, style: Style) -> Self {
        self.border_style = style;
        self
    }

    pub fn border_type(mut self, bt: BorderType) -> Self {
        self.border_type = bt;
        self
    }

    pub fn padding(mut self, padding: Padding) -> Self {
        self.padding = padding;
        self
    }

    pub fn clear_bg(mut self, clear: bool) -> Self {
        self.clear_bg = clear;
        self
    }

    /// Render the pane and return the inner area for content.
    pub fn render(self, f: &mut Frame, area: Rect) -> Rect {
        if self.clear_bg {
            f.render_widget(Clear, area);
        }

        let mut block = Block::default()
            .borders(Borders::ALL)
            .border_type(self.border_type)
            .border_style(self.border_style)
            .padding(self.padding)
            .style(Style::default().bg(surface()));

        if let Some(t) = self.title {
            block = block.title(format!(" {t} "));
        }

        let inner = block.inner(area);
        f.render_widget(block, area);
        inner
    }
}
