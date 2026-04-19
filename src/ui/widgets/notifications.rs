use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
    Frame,
};

use crate::state::{AppState, ToastLevel};
use crate::ui::theme::{blue, dim, green, red, surface, white, yellow};

pub fn render_notifications(f: &mut Frame, area: Rect, state: &AppState) {
    let toasts = &state.toasts.toasts;
    let count = toasts.len().min(20);
    let overlay_h = (4 + count as u16).min(area.height.saturating_sub(4));
    let overlay_w = area.width.clamp(48, 68);

    let overlay = Rect {
        x: area.x + (area.width.saturating_sub(overlay_w)) / 2,
        y: area.y + 2,
        width: overlay_w,
        height: overlay_h,
    };

    f.render_widget(Clear, overlay);

    let title = format!(" Notificaciones ({}) ", toasts.len());
    let panel = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(yellow()))
        .style(Style::default().bg(surface()));

    let inner = panel.inner(overlay);
    f.render_widget(panel, overlay);

    if toasts.is_empty() {
        f.render_widget(
            Paragraph::new(vec![
                Line::from(""),
                Line::from(Span::styled("  Sin notificaciones", Style::default().fg(dim()))),
            ]),
            inner,
        );
        return;
    }

    let lines: Vec<Line<'static>> = toasts
        .iter()
        .rev()
        .take(inner.height as usize)
        .map(|t| {
            let (icon, color) = match t.level {
                ToastLevel::Info => ("ℹ", blue()),
                ToastLevel::Success => ("✓", green()),
                ToastLevel::Warning => ("⚠", yellow()),
                ToastLevel::Error => ("✗", red()),
            };
            let msg: String = t.message.chars().take(overlay_w as usize - 6).collect();
            Line::from(vec![
                Span::styled(
                    format!(" {icon} "),
                    Style::default().fg(color).add_modifier(Modifier::BOLD),
                ),
                Span::styled(msg, Style::default().fg(white())),
            ])
        })
        .collect();

    f.render_widget(Paragraph::new(lines), inner);
}
