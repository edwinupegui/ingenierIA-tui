use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
    Frame,
};

use super::truncate;
use crate::state::AppState;
use crate::ui::theme::{dim, surface, white, yellow, GLYPH_CURSOR, GLYPH_CURSOR_BLOCK};

pub fn render_search_overlay(f: &mut Frame, area: Rect, state: &AppState) {
    let search = &state.search;
    let res_count = search.results.len().min(10);
    let extra = if search.loading || (!search.query.is_empty() && search.results.is_empty()) {
        1
    } else {
        0
    };
    let overlay_h = (3 + res_count as u16 + extra + 1).min(area.height.saturating_sub(4));
    let overlay_w = area.width.clamp(44, 68);

    let overlay = Rect {
        x: area.x + (area.width.saturating_sub(overlay_w)) / 2,
        y: area.y + 2,
        width: overlay_w,
        height: overlay_h,
    };

    f.render_widget(Clear, overlay);

    let input_rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Fill(1)])
        .split(overlay);

    let input_block = Block::default()
        .title(" Buscar ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(yellow()))
        .style(Style::default().bg(surface()));

    let inner_input = input_block.inner(input_rows[0]);
    f.render_widget(input_block, input_rows[0]);

    let cursor = if (state.tick_count / 4).is_multiple_of(2) { GLYPH_CURSOR_BLOCK } else { " " };
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(format!(" {GLYPH_CURSOR} "), Style::default().fg(yellow())),
            Span::styled(search.query.clone(), Style::default().fg(white())),
            Span::styled(cursor, Style::default().fg(yellow())),
        ])),
        inner_input,
    );

    let results_area = input_rows[1];
    if results_area.height == 0 {
        return;
    }

    let bg = surface();
    f.render_widget(Block::default().style(Style::default().bg(bg)), results_area);

    if search.loading {
        f.render_widget(
            Paragraph::new(Span::styled("  Buscando...", Style::default().fg(dim()).bg(bg))),
            results_area,
        );
        return;
    }

    if !search.query.is_empty() && search.results.is_empty() {
        f.render_widget(
            Paragraph::new(Span::styled("  Sin resultados", Style::default().fg(dim()).bg(bg))),
            results_area,
        );
        return;
    }

    let visible = results_area.height as usize;
    let scroll_offset = if search.cursor >= visible { search.cursor - visible + 1 } else { 0 };

    let lines: Vec<Line<'static>> = search
        .results
        .iter()
        .enumerate()
        .skip(scroll_offset)
        .take(visible)
        .map(|(i, r)| {
            let is_cursor = i == search.cursor;
            let name = truncate(&r.name, 22);
            let type_factory = format!("{}/{}", r.doc_type, r.factory);

            if is_cursor {
                Line::from(vec![
                    Span::styled(format!(" {GLYPH_CURSOR} "), Style::default().fg(yellow()).bg(bg)),
                    Span::styled(
                        name,
                        Style::default().fg(bg).bg(yellow()).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled("  ", Style::default().bg(bg)),
                    Span::styled(type_factory, Style::default().fg(dim()).bg(bg)),
                ])
            } else {
                Line::from(vec![
                    Span::styled("   ", Style::default().bg(bg)),
                    Span::styled(name, Style::default().fg(white()).bg(bg)),
                    Span::styled("  ", Style::default().bg(bg)),
                    Span::styled(type_factory, Style::default().fg(dim()).bg(bg)),
                ])
            }
        })
        .collect();

    f.render_widget(Paragraph::new(lines), results_area);
}
