use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use super::theme::{dim, dimmer, green, red, surface, white, GLYPH_CURSOR, GLYPH_ERROR};
use crate::state::{AppState, WIZARD_PROVIDERS};

pub fn render_select_model_phase(f: &mut Frame, area: Rect, state: &AppState) {
    let copilot = &state.wizard.copilot;
    let model_count = copilot.models.len();

    // Calculate visible area: 2 lines for header, rest for models
    let max_visible = (area.height as usize).saturating_sub(3);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // label
            Constraint::Fill(1),   // models list
            Constraint::Length(1), // error or spacer
        ])
        .split(area);

    // Header
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("Modelo", Style::default().fg(white()).add_modifier(Modifier::BOLD)),
            Span::styled(
                format!("  GitHub Copilot — {} modelos", model_count),
                Style::default().fg(dim()),
            ),
        ])),
        rows[0],
    );

    if model_count == 0 {
        let msg = if let Some(err) = &copilot.error {
            format!("{GLYPH_ERROR} {err}")
        } else {
            "No hay modelos disponibles".to_string()
        };
        f.render_widget(Paragraph::new(Span::styled(msg, Style::default().fg(red()))), rows[1]);
        return;
    }

    // Scrolling: ensure cursor is visible
    let scroll_offset = if copilot.model_cursor >= max_visible {
        copilot.model_cursor - max_visible + 1
    } else {
        0
    };

    let visible_models: Vec<(usize, &crate::services::copilot::CopilotModel)> =
        copilot.models.iter().enumerate().skip(scroll_offset).take(max_visible).collect();

    // Build model lines
    let model_lines: Vec<Line<'_>> = visible_models
        .iter()
        .map(|(i, m)| {
            let is_selected = *i == copilot.model_cursor;
            if is_selected {
                Line::from(vec![
                    Span::styled(
                        format!("{GLYPH_CURSOR} "),
                        Style::default().fg(green()).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        &m.display_name,
                        Style::default().fg(white()).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(format!("  {}", m.id), Style::default().fg(dimmer())),
                ])
            } else {
                Line::from(vec![
                    Span::styled("  ", Style::default()),
                    Span::styled(&m.display_name, Style::default().fg(dim())),
                ])
            }
        })
        .collect();

    f.render_widget(Paragraph::new(model_lines), rows[1]);
}

pub fn render_provider_phase(f: &mut Frame, area: Rect, state: &AppState) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            std::iter::once(Constraint::Length(2))
                .chain(WIZARD_PROVIDERS.iter().map(|_| Constraint::Length(1)))
                .chain(std::iter::once(Constraint::Fill(1)))
                .collect::<Vec<_>>(),
        )
        .split(area);

    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                "Proveedor de AI",
                Style::default().fg(white()).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("  Hola, {}", state.wizard.name_input.trim()),
                Style::default().fg(dim()),
            ),
        ])),
        rows[0],
    );

    for (i, (_, label, enabled)) in WIZARD_PROVIDERS.iter().enumerate() {
        let is_selected = i == state.wizard.provider_cursor;
        let row_idx = i + 1;
        if row_idx >= rows.len() {
            break;
        }

        let line = if !enabled {
            Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled(*label, Style::default().fg(super::theme::muted())),
            ])
        } else if is_selected {
            Line::from(vec![
                Span::styled(
                    format!("{GLYPH_CURSOR} "),
                    Style::default().fg(green()).add_modifier(Modifier::BOLD),
                ),
                Span::styled(*label, Style::default().fg(white()).add_modifier(Modifier::BOLD)),
            ])
        } else {
            Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled(*label, Style::default().fg(dim())),
            ])
        };

        let bg = if is_selected && *enabled {
            Style::default().bg(super::theme::surface_green())
        } else {
            Style::default().bg(surface())
        };
        f.render_widget(Paragraph::new(line).style(bg), rows[row_idx]);
    }
}
