use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use crate::state::DocPickerState;
use crate::ui::theme::{bg, blue, border, dim, dimmer, green, surface, white};

const MAX_VISIBLE: usize = 15;
const NAME_COL_WIDTH: usize = 28;

/// Render the MCP document picker overlay centered on screen.
pub fn render(f: &mut Frame, picker: &DocPickerState) {
    if !picker.visible {
        return;
    }

    let screen = f.area();
    let popup_w = (screen.width * 70 / 100).clamp(50, 90);
    let popup_h = (MAX_VISIBLE as u16 + 6).min(screen.height.saturating_sub(4));
    let popup_x = (screen.width.saturating_sub(popup_w)) / 2;
    let popup_y = (screen.height.saturating_sub(popup_h)) / 2;
    let area = Rect { x: popup_x, y: popup_y, width: popup_w, height: popup_h };

    f.render_widget(Clear, area);

    let title = format!(" {} ", picker.label);
    let block = Block::default()
        .title(title)
        .title_alignment(Alignment::Left)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border()))
        .style(Style::default().bg(surface()));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // search input
            Constraint::Length(1), // separator
            Constraint::Fill(1),   // items list
            Constraint::Length(1), // footer
        ])
        .split(inner);

    // Search input
    let search_line = if picker.query.is_empty() {
        Line::from(Span::styled("Buscar...", Style::default().fg(dim())))
    } else {
        Line::from(Span::styled(&picker.query, Style::default().fg(white())))
    };
    f.render_widget(Paragraph::new(search_line).style(Style::default().bg(surface())), rows[0]);

    // Separator
    let sep = "─".repeat(inner.width as usize);
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(sep, Style::default().fg(border())))),
        rows[1],
    );

    // Items list
    let item_count = picker.filtered.len();
    let scroll_offset =
        if picker.cursor >= MAX_VISIBLE { picker.cursor - MAX_VISIBLE + 1 } else { 0 };
    let desc_width = (rows[2].width as usize).saturating_sub(NAME_COL_WIDTH + 4);

    let lines: Vec<Line<'_>> = picker
        .filtered
        .iter()
        .skip(scroll_offset)
        .take(MAX_VISIBLE)
        .enumerate()
        .map(|(vi, &idx)| {
            let doc = &picker.items[idx];
            let is_selected = vi + scroll_offset == picker.cursor;
            let (name_style, desc_style, bg) = if is_selected {
                (
                    Style::default().fg(bg()).bg(green()).add_modifier(Modifier::BOLD),
                    Style::default().fg(bg()).bg(green()),
                    Style::default().bg(green()),
                )
            } else {
                (Style::default().fg(blue()), Style::default().fg(dim()), Style::default())
            };

            let name_padded = format!("{:<width$}", doc.name, width = NAME_COL_WIDTH);
            let desc_str: String = if doc.description.len() > desc_width {
                let truncate_at = desc_width.saturating_sub(3);
                let end = doc
                    .description
                    .char_indices()
                    .map(|(i, _)| i)
                    .take_while(|&i| i <= truncate_at)
                    .last()
                    .unwrap_or(0);
                format!("{}...", &doc.description[..end])
            } else {
                doc.description.clone()
            };

            Line::from(vec![
                Span::styled(" ", bg),
                Span::styled(name_padded, name_style),
                Span::styled("  ", bg),
                Span::styled(desc_str, desc_style),
            ])
        })
        .collect();

    f.render_widget(Paragraph::new(lines).style(Style::default().bg(surface())), rows[2]);

    // Scroll indicators on border
    if scroll_offset > 0 {
        let indicator = Span::styled(" ▲", Style::default().fg(dim()));
        f.render_widget(
            Paragraph::new(Line::from(indicator)),
            Rect { x: area.x + area.width - 4, y: area.y, width: 3, height: 1 },
        );
    }
    if scroll_offset + MAX_VISIBLE < item_count {
        let indicator = Span::styled(" ▼", Style::default().fg(dim()));
        f.render_widget(
            Paragraph::new(Line::from(indicator)),
            Rect { x: area.x + area.width - 4, y: area.y + area.height - 1, width: 3, height: 1 },
        );
    }

    // Footer
    let count = format!("{} de {}", item_count, picker.items.len());
    let footer = Line::from(vec![
        Span::styled(" enter ", Style::default().fg(dim())),
        Span::styled("seleccionar", Style::default().fg(dimmer())),
        Span::styled("  esc ", Style::default().fg(dim())),
        Span::styled("cerrar", Style::default().fg(dimmer())),
        Span::styled(format!("  {count}"), Style::default().fg(dim())),
    ]);
    f.render_widget(Paragraph::new(footer).style(Style::default().bg(surface())), rows[3]);
}
