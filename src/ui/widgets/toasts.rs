use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Clear, Paragraph},
    Frame,
};

use crate::state::{AppState, ToastLevel};
use crate::ui::theme::{blue, green, red, surface, white, yellow};

/// Renderiza toasts en la esquina inferior derecha del area.
pub fn render_toasts(f: &mut Frame, area: Rect, state: &AppState) {
    if state.toasts.is_empty() {
        return;
    }

    let toasts: Vec<_> = state.toasts.visible().collect();
    let count = toasts.len() as u16;
    if count == 0 {
        return;
    }

    let toast_w = 44u16.min(area.width.saturating_sub(4));
    let toast_h = count;
    let x = area.x + area.width.saturating_sub(toast_w + 2);
    let y = area.y + area.height.saturating_sub(toast_h + 2);

    let toast_area = Rect { x, y, width: toast_w, height: toast_h };
    f.render_widget(Clear, toast_area);
    f.render_widget(Block::default().style(Style::default().bg(surface())), toast_area);

    let lines: Vec<Line<'static>> = toasts
        .iter()
        .map(|t| {
            let (icon, color) = match t.level {
                ToastLevel::Info => ("ℹ", blue()),
                ToastLevel::Success => ("✓", green()),
                ToastLevel::Warning => ("⚠", yellow()),
                ToastLevel::Error => ("✗", red()),
            };
            let msg: String = t.message.chars().take(toast_w as usize - 4).collect();
            Line::from(vec![
                Span::styled(
                    format!(" {icon} "),
                    Style::default().fg(color).add_modifier(Modifier::BOLD),
                ),
                Span::styled(msg, Style::default().fg(white())),
            ])
        })
        .collect();

    f.render_widget(Paragraph::new(lines), toast_area);
}
