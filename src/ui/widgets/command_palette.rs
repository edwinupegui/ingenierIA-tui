use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
    Frame,
};

use crate::state::AppState;
use crate::ui::theme::{dim, gray, purple, surface, white, GLYPH_CURSOR, GLYPH_CURSOR_BLOCK};

pub fn render_command_palette(f: &mut Frame, area: Rect, state: &AppState) {
    let cmds = state.command.filtered();
    let is_filtering = !state.command.query.is_empty();

    // Count category headers (only when not filtering)
    let header_count = if is_filtering {
        0u16
    } else {
        let mut count = 0u16;
        let mut prev_cat = "";
        for cmd in &cmds {
            if cmd.category != prev_cat {
                count += 1;
                prev_cat = &cmd.category;
            }
        }
        count
    };

    let total_lines = cmds.len() as u16 + header_count;
    let res_count = total_lines.min(16);
    // +1 for bottom padding
    let overlay_h = (3 + res_count + 1).min(area.height.saturating_sub(4));
    let overlay_w = area.width.clamp(44, 78);

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

    let input_block = Block::default()
        .title(" Paleta de Comandos ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(purple()))
        .style(Style::default().bg(surface()));

    let inner_input = input_block.inner(rows[0]);
    f.render_widget(input_block, rows[0]);

    let cursor = if (state.tick_count / 4).is_multiple_of(2) { GLYPH_CURSOR_BLOCK } else { " " };
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(" : ", Style::default().fg(purple())),
            Span::styled(state.command.query.clone(), Style::default().fg(white())),
            Span::styled(cursor, Style::default().fg(purple())),
        ])),
        inner_input,
    );

    if rows[1].height == 0 {
        return;
    }

    let bg = surface();
    f.render_widget(Block::default().style(Style::default().bg(bg)), rows[1]);

    // Build display lines: interleave category headers with commands
    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut cursor_line: usize = 0;
    let mut prev_cat = "";
    let inner_w = rows[1].width as usize;
    let id_width: usize = 18;
    let desc_offset: usize = 3 + id_width + 2; // "   " + id_pad + "  "
    let desc_budget = inner_w.saturating_sub(desc_offset);

    for (i, cmd) in cmds.iter().enumerate() {
        if !is_filtering && cmd.category.as_str() != prev_cat {
            let label = cmd.category.to_lowercase();
            let fill_len = inner_w.saturating_sub(label.len() + 4);
            let fill = "─".repeat(fill_len);
            lines.push(Line::from(vec![
                Span::styled("── ", Style::default().fg(dim()).bg(bg)),
                Span::styled(label, Style::default().fg(dim()).bg(bg)),
                Span::styled(format!(" {fill}"), Style::default().fg(dim()).bg(bg)),
            ]));
            prev_cat = &cmd.category;
        }

        let is_cursor = i == state.command.cursor;
        if is_cursor {
            cursor_line = lines.len();
        }
        let id_pad = format!("{:<width$}", cmd.id, width = id_width);
        let desc = truncate_desc(&cmd.description, desc_budget);
        if is_cursor {
            lines.push(Line::from(vec![
                Span::styled(format!(" {GLYPH_CURSOR} "), Style::default().fg(purple()).bg(bg)),
                Span::styled(
                    id_pad,
                    Style::default().fg(bg).bg(purple()).add_modifier(Modifier::BOLD),
                ),
                Span::styled("  ", Style::default().bg(bg)),
                Span::styled(desc, Style::default().fg(white()).bg(bg)),
            ]));
        } else {
            lines.push(Line::from(vec![
                Span::styled("   ", Style::default().bg(bg)),
                Span::styled(id_pad, Style::default().fg(white()).bg(bg)),
                Span::styled("  ", Style::default().bg(bg)),
                Span::styled(desc, Style::default().fg(gray()).bg(bg)),
            ]));
        }
    }

    let visible = rows[1].height as usize;
    let scroll_offset = if cursor_line >= visible { cursor_line - visible + 1 } else { 0 };

    let visible_lines: Vec<Line<'static>> =
        lines.into_iter().skip(scroll_offset).take(visible).collect();

    f.render_widget(Paragraph::new(visible_lines), rows[1]);
}

fn truncate_desc(s: &str, budget: usize) -> String {
    if budget == 0 {
        return String::new();
    }
    if s.chars().count() <= budget {
        return s.to_string();
    }
    let take = budget.saturating_sub(1);
    let mut out: String = s.chars().take(take).collect();
    out.push('…');
    out
}
