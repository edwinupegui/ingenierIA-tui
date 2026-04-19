use ratatui::{
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
    Frame,
};

use crate::state::AppState;
use crate::ui::theme::{
    dim, green, purple, red, surface, yellow, GLYPH_ERROR, GLYPH_PENDING, GLYPH_SUCCESS,
};

pub fn render_enforcement(f: &mut Frame, area: Rect, state: &AppState) {
    let events = &state.hook_events;
    let count = events.len().min(20);
    let overlay_h = (4 + count as u16).min(area.height.saturating_sub(4));
    let overlay_w = area.width.clamp(50, 76);

    let overlay = Rect {
        x: area.x + (area.width.saturating_sub(overlay_w)) / 2,
        y: area.y + 2,
        width: overlay_w,
        height: overlay_h,
    };

    f.render_widget(Clear, overlay);

    let total = events.len();
    let blocks = events.iter().filter(|e| e.is_block()).count();
    let passes = events.iter().filter(|e| e.is_pass()).count();
    let block_rate = if total > 0 { (blocks as f64 / total as f64) * 100.0 } else { 0.0 };

    let title = format!(
        " Enforcement ({total} checks, {blocks} blocked, {passes} passed, {block_rate:.0}% rate) "
    );

    let panel = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(purple()))
        .style(Style::default().bg(surface()));

    let inner = panel.inner(overlay);
    f.render_widget(panel, overlay);

    if events.is_empty() {
        crate::ui::primitives::render_empty_state(
            f,
            inner,
            "○",
            "Sin eventos de enforcement registrados",
            Some("Los hooks emiten eventos al validar compliance"),
        );
        return;
    }

    let lines: Vec<Line<'static>> = events
        .iter()
        .take(inner.height as usize)
        .map(|e| {
            let (icon, color) = if e.is_block() {
                (GLYPH_ERROR, red())
            } else if e.is_pass() {
                (GLYPH_SUCCESS, green())
            } else {
                (GLYPH_PENDING, yellow())
            };

            let rule = e.rule.as_deref().unwrap_or("");
            let factory = e.factory.as_deref().unwrap_or("");
            let time = super::extract_time(&e.timestamp);

            crate::ui::primitives::list_row(
                icon,
                color,
                &e.hook,
                18,
                vec![
                    Span::styled(format!("{factory:<5}"), Style::default().fg(dim())),
                    Span::styled(
                        format!(" {rule:<16}"),
                        Style::default().fg(if e.is_block() { red() } else { dim() }),
                    ),
                    Span::styled(format!(" {time}"), Style::default().fg(dim())),
                ],
            )
        })
        .collect();

    f.render_widget(Paragraph::new(lines), inner);
}
