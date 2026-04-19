use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
    Frame,
};

use crate::state::AppState;
use crate::ui::theme::{bg, cyan, dim, green, red, surface, white, yellow};

pub fn render_model_picker(f: &mut Frame, area: Rect, state: &AppState) {
    let picker = &state.model_picker;
    let item_count = picker.models.len().max(1);
    let overlay_h = (3 + item_count as u16).min(area.height.saturating_sub(4));
    let overlay_w = area.width.clamp(44, 64);

    let overlay = Rect {
        x: area.x + (area.width.saturating_sub(overlay_w)) / 2,
        y: area.y + 3,
        width: overlay_w,
        height: overlay_h,
    };

    f.render_widget(Clear, overlay);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Fill(1)])
        .split(overlay);

    let title = if picker.loading { " Cargando modelos... " } else { " Seleccionar Modelo " };
    let title_block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(cyan()))
        .style(Style::default().bg(surface()));

    let title_inner = title_block.inner(rows[0]);
    f.render_widget(title_block, rows[0]);

    let current_line = Line::from(vec![
        Span::styled(" Actual: ", Style::default().fg(dim())),
        Span::styled(
            state.model.clone(),
            Style::default().fg(green()).add_modifier(Modifier::BOLD),
        ),
    ]);
    f.render_widget(Paragraph::new(current_line), title_inner);

    if rows[1].height == 0 {
        return;
    }

    f.render_widget(Block::default().style(Style::default().bg(bg())), rows[1]);

    if let Some(ref err) = picker.error {
        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled("  Error: ", Style::default().fg(red())),
                Span::styled(err.clone(), Style::default().fg(red())),
            ])),
            rows[1],
        );
        return;
    }

    if picker.loading {
        let dots = ".".repeat((state.tick_count as usize % 3) + 1);
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                format!("  Cargando{dots}"),
                Style::default().fg(yellow()),
            ))),
            rows[1],
        );
        return;
    }

    let lines: Vec<Line<'static>> = picker
        .models
        .iter()
        .take(rows[1].height as usize)
        .enumerate()
        .map(|(i, model)| {
            let is_cursor = i == picker.cursor;
            let is_current = model.id == state.model;
            let marker = if is_current { " * " } else { "   " };
            if is_cursor {
                Line::from(vec![
                    Span::styled(" > ", Style::default().fg(cyan())),
                    Span::styled(
                        format!("{:<30}", model.id),
                        Style::default().fg(bg()).bg(cyan()).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(marker, Style::default().fg(green())),
                ])
            } else {
                Line::from(vec![
                    Span::styled("   ", Style::default()),
                    Span::styled(format!("{:<30}", model.id), Style::default().fg(white())),
                    Span::styled(
                        marker,
                        Style::default().fg(if is_current { green() } else { dim() }),
                    ),
                ])
            }
        })
        .collect();

    f.render_widget(Paragraph::new(lines), rows[1]);
}
