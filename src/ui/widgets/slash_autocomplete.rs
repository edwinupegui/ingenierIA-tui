use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use crate::state::SlashAutocomplete;
use crate::ui::theme::{bg, blue, border, dim, green, surface};

const MAX_VISIBLE: usize = 12;
const CMD_COL_WIDTH: u16 = 16;

/// Render the slash command autocomplete popup above the input area.
/// `full_width`: area to use for popup width (pass f.area() for chat, input_rect for splash).
pub fn render(f: &mut Frame, autocomplete: &SlashAutocomplete, input_area: Rect, full_width: Rect) {
    if !autocomplete.visible || autocomplete.filtered.is_empty() {
        return;
    }

    let item_count = autocomplete.filtered.len().min(MAX_VISIBLE);
    let popup_height = item_count as u16 + 2; // +2 for borders
    let popup_width = full_width.width.saturating_sub(2).max(44);
    let popup_x = full_width.x + (full_width.width.saturating_sub(popup_width)) / 2;
    let popup_y = input_area.y.saturating_sub(popup_height);

    let area = Rect { x: popup_x, y: popup_y, width: popup_width, height: popup_height };

    // Clear the area behind the popup
    f.render_widget(Clear, area);

    // Hint contextual: si hay un unico match, Tab/Enter completan; si hay varios,
    // Tab primero expande al prefijo comun y luego cicla.
    let hint_text = if autocomplete.filtered.len() == 1 {
        " Tab/Enter: completar "
    } else {
        " Tab: prefijo/ciclar · Enter: usar "
    };
    let hint_line = Line::styled(hint_text, Style::default().fg(dim()))
        .alignment(ratatui::layout::Alignment::Right);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border()))
        .style(Style::default().bg(surface()))
        .title_bottom(hint_line);
    let inner = block.inner(area);
    f.render_widget(block, area);

    // Scroll window around cursor
    let scroll_offset =
        if autocomplete.cursor >= MAX_VISIBLE { autocomplete.cursor - MAX_VISIBLE + 1 } else { 0 };

    let desc_width = inner.width.saturating_sub(CMD_COL_WIDTH + 3) as usize;

    let lines: Vec<Line<'_>> = autocomplete
        .filtered
        .iter()
        .skip(scroll_offset)
        .take(MAX_VISIBLE)
        .enumerate()
        .map(|(vi, (_, cmd, desc))| {
            let is_selected = vi + scroll_offset == autocomplete.cursor;
            let (cmd_style, desc_style, bg) = if is_selected {
                (
                    Style::default().fg(bg()).bg(green()).add_modifier(Modifier::BOLD),
                    Style::default().fg(bg()).bg(green()),
                    Style::default().bg(green()),
                )
            } else {
                (Style::default().fg(blue()), Style::default().fg(dim()), Style::default())
            };

            // Pad command name to fixed width
            let cmd_padded = format!("{:<width$}", cmd, width = CMD_COL_WIDTH as usize);
            // Truncate description
            let desc_str: String = if desc.len() > desc_width {
                let truncate_at = desc_width.saturating_sub(3);
                let end = desc
                    .char_indices()
                    .map(|(i, _)| i)
                    .take_while(|&i| i <= truncate_at)
                    .last()
                    .unwrap_or(0);
                format!("{}...", &desc[..end])
            } else {
                desc.to_string()
            };

            Line::from(vec![
                Span::styled(" ", bg),
                Span::styled(cmd_padded, cmd_style),
                Span::styled("  ", bg),
                Span::styled(desc_str, desc_style),
            ])
        })
        .collect();

    f.render_widget(Paragraph::new(lines).style(Style::default().bg(surface())), inner);

    // Scroll indicators
    if scroll_offset > 0 {
        let indicator = Span::styled(" ▲", Style::default().fg(dim()));
        f.render_widget(
            Paragraph::new(Line::from(indicator)),
            Rect { x: inner.x + inner.width - 3, y: area.y, width: 3, height: 1 },
        );
    }
    if scroll_offset + MAX_VISIBLE < autocomplete.filtered.len() {
        let indicator = Span::styled(" ▼", Style::default().fg(dim()));
        f.render_widget(
            Paragraph::new(Line::from(indicator)),
            Rect { x: inner.x + inner.width - 3, y: area.y + area.height - 1, width: 3, height: 1 },
        );
    }
}
