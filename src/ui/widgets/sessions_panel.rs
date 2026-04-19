use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
    Frame,
};

use crate::state::AppState;
use crate::ui::theme::{cyan, dim, surface, white, GLYPH_PENDING};

pub fn render_sessions_panel(f: &mut Frame, area: Rect, state: &AppState) {
    let count = state.sessions.len().max(1);
    let overlay_h = (4 + count as u16).min(area.height.saturating_sub(4));
    let overlay_w = area.width.clamp(44, 60);

    let overlay = Rect {
        x: area.x + (area.width.saturating_sub(overlay_w)) / 2,
        y: area.y + (area.height.saturating_sub(overlay_h)) / 2,
        width: overlay_w,
        height: overlay_h,
    };

    f.render_widget(Clear, overlay);

    let sessions_count = state.sessions.len();
    let title = format!(" Sesiones Activas ({sessions_count}) ");
    let panel = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(cyan()))
        .style(Style::default().bg(surface()));

    let inner = panel.inner(overlay);
    f.render_widget(panel, overlay);

    if state.sessions.is_empty() {
        f.render_widget(
            Paragraph::new(vec![
                Line::from(""),
                Line::from(Span::styled(
                    "  Sin sesiones registradas aún",
                    Style::default().fg(dim()),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    "  Los eventos SSE de sesión aparecerán aquí",
                    Style::default().fg(dim()),
                )),
            ]),
            inner,
        );
        return;
    }

    let lines: Vec<Line<'static>> = state
        .sessions
        .iter()
        .map(|s| {
            Line::from(vec![
                Span::styled(format!("  {GLYPH_PENDING} "), Style::default().fg(cyan())),
                Span::styled(
                    format!("{:<20}", s.developer),
                    Style::default().fg(white()).add_modifier(Modifier::BOLD),
                ),
                Span::styled(format!("desde {}", s.time), Style::default().fg(dim())),
            ])
        })
        .collect();

    f.render_widget(Paragraph::new(lines), inner);
}
