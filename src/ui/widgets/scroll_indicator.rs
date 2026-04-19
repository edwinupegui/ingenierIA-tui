/// Scroll indicators — ▲/▼ markers for lists that overflow viewport.
///
/// Renders a single-line indicator at top or bottom of a list area.
use ratatui::{
    layout::{Alignment, Rect},
    style::Style,
    text::Span,
    widgets::Paragraph,
    Frame,
};

use crate::ui::theme::dim;

/// Render ▲ at the top of `area` if `can_scroll_up` is true.
pub fn render_scroll_up(f: &mut Frame, area: Rect, can_scroll_up: bool) {
    if !can_scroll_up || area.height == 0 {
        return;
    }
    let indicator_area = Rect { height: 1, ..area };
    let widget =
        Paragraph::new(Span::styled("▲", Style::default().fg(dim()))).alignment(Alignment::Center);
    f.render_widget(widget, indicator_area);
}

/// Render ▼ at the bottom of `area` if `can_scroll_down` is true.
pub fn render_scroll_down(f: &mut Frame, area: Rect, can_scroll_down: bool) {
    if !can_scroll_down || area.height == 0 {
        return;
    }
    let indicator_area = Rect { y: area.y + area.height.saturating_sub(1), height: 1, ..area };
    let widget =
        Paragraph::new(Span::styled("▼", Style::default().fg(dim()))).alignment(Alignment::Center);
    f.render_widget(widget, indicator_area);
}
