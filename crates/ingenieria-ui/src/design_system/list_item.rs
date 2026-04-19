/// ListItem — Selectable row with focus styling.
///
/// Reference: claude-code ListItem.tsx.
/// Renders a row with icon, label, and optional trailing text.
/// When selected, inverts background for visual focus.
use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};

use crate::theme::{dim, surface, white};

pub struct ListItem;

impl ListItem {
    /// Build a selectable list row.
    /// `selected` inverts the background color for focus indication.
    pub fn row(
        icon: &str,
        icon_color: ratatui::style::Color,
        label: &str,
        label_w: usize,
        trailing: Vec<Span<'static>>,
        selected: bool,
    ) -> Line<'static> {
        let bg = if selected { dim() } else { surface() };
        let fg = if selected { surface() } else { white() };

        let mut spans = vec![
            Span::styled(format!(" {icon} "), Style::default().fg(icon_color).bg(bg)),
            Span::styled(
                format!("{:<w$}", label, w = label_w),
                Style::default().fg(fg).bg(bg).add_modifier(Modifier::BOLD),
            ),
        ];
        for s in trailing {
            spans.push(Span::styled(s.content.clone(), s.style.bg(bg)));
        }
        Line::from(spans)
    }
}
