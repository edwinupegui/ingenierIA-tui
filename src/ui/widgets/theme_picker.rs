//! Modal selector de themes al estilo opencode: lista vertical con search
//! inline y live preview. Se abre con `/theme` sin args y se cierra con Esc
//! (revierte) o Enter (persiste).

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
    Frame,
};

use crate::state::AppState;
use crate::ui::theme::{bg, cyan, dim, green, surface, white, GLYPH_CURSOR_BLOCK};

pub fn render_theme_picker(f: &mut Frame, area: Rect, state: &AppState) {
    let Some(picker) = state.theme_picker.as_ref() else {
        return;
    };
    let items = picker.filtered();
    let item_count = items.len().max(1);
    let overlay_h = (4 + item_count as u16).min(area.height.saturating_sub(4));
    let overlay_w = area.width.clamp(40, 56);

    let overlay = Rect {
        x: area.x + (area.width.saturating_sub(overlay_w)) / 2,
        y: area.y + 3,
        width: overlay_w,
        height: overlay_h,
    };

    f.render_widget(Clear, overlay);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(2), Constraint::Fill(1)])
        .split(overlay);

    let frame_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(cyan()))
        .style(Style::default().bg(surface()));
    f.render_widget(frame_block.clone(), overlay);
    let inner = frame_block.inner(overlay);

    let header = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Fill(1), Constraint::Length(4)])
        .split(rows[0]);

    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::raw(" "),
            Span::styled("Temas", Style::default().fg(white()).add_modifier(Modifier::BOLD)),
        ])),
        header[0],
    );
    f.render_widget(
        Paragraph::new(Line::from(vec![Span::styled("esc ", Style::default().fg(dim()))])),
        header[1],
    );

    let cursor_glyph =
        if (state.tick_count / 4).is_multiple_of(2) { GLYPH_CURSOR_BLOCK } else { " " };
    let search_display =
        if picker.query.is_empty() { "Buscar".to_string() } else { picker.query.clone() };
    let search_style = if picker.query.is_empty() {
        Style::default().fg(dim())
    } else {
        Style::default().fg(white())
    };
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::raw(" "),
            Span::styled(search_display, search_style),
            Span::styled(cursor_glyph, Style::default().fg(cyan())),
        ])),
        rows[1],
    );

    if rows[2].height == 0 {
        return;
    }
    f.render_widget(Block::default().style(Style::default().bg(bg())), rows[2]);

    let visible = rows[2].height as usize;
    let start = picker.cursor.saturating_sub(visible.saturating_sub(1));
    let lines: Vec<Line<'static>> = items
        .iter()
        .enumerate()
        .skip(start)
        .take(visible)
        .map(|(i, variant)| {
            let is_cursor = i == picker.cursor;
            let is_current = *variant == state.active_theme;
            let marker = if is_current { "• " } else { "  " };
            let label = variant.slug();
            if is_cursor {
                Line::from(vec![Span::styled(
                    format!(" {marker}{label:<width$} ", width = overlay_w as usize - 5),
                    Style::default().fg(bg()).bg(cyan()).add_modifier(Modifier::BOLD),
                )])
            } else {
                let color = if is_current { green() } else { white() };
                Line::from(vec![
                    Span::styled(" ", Style::default()),
                    Span::styled(
                        marker,
                        Style::default().fg(if is_current { green() } else { dim() }),
                    ),
                    Span::styled(label, Style::default().fg(color)),
                ])
            }
        })
        .collect();

    f.render_widget(Paragraph::new(lines), rows[2]);
    let _ = inner;
}
