//! Modal del picker Autoskill: lista de skills recomendadas del stack con
//! checkboxes para marcar cuales instalar. Se abre desde `:` + `autoskill`.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
    Frame,
};

use crate::state::AppState;
use crate::ui::theme::{bg, cyan, dim, green, surface, white, yellow};

pub fn render_autoskill_picker(f: &mut Frame, area: Rect, state: &AppState) {
    let Some(picker) = state.autoskill_picker.as_ref() else {
        return;
    };
    let items_len = picker.items.len().max(1);
    let overlay_h = (6 + items_len as u16).min(area.height.saturating_sub(4));
    let overlay_w = area.width.clamp(56, 88);
    let overlay = Rect {
        x: area.x + (area.width.saturating_sub(overlay_w)) / 2,
        y: area.y + 2,
        width: overlay_w,
        height: overlay_h,
    };

    f.render_widget(Clear, overlay);
    let frame_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(cyan()))
        .style(Style::default().bg(surface()));
    f.render_widget(frame_block.clone(), overlay);
    let inner = frame_block.inner(overlay);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // title
            Constraint::Length(1), // subtitle (project summary)
            Constraint::Length(1), // separator / counts
            Constraint::Fill(1),   // list
            Constraint::Length(1), // hint
        ])
        .split(inner);

    // Title row
    let header = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Fill(1), Constraint::Length(4)])
        .split(rows[0]);
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::raw(" "),
            Span::styled(
                "Autoskill — skills recomendadas",
                Style::default().fg(white()).add_modifier(Modifier::BOLD),
            ),
        ])),
        header[0],
    );
    f.render_widget(
        Paragraph::new(Line::from(vec![Span::styled("esc ", Style::default().fg(dim()))])),
        header[1],
    );

    // Project summary
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::raw(" "),
            Span::styled(picker.project_summary.clone(), Style::default().fg(dim())),
        ])),
        rows[1],
    );

    // Counts line
    let installed = picker.installed_count();
    let pending = picker.pending_count();
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::raw(" "),
            Span::styled(format!("{installed} instaladas"), Style::default().fg(green())),
            Span::styled(" · ", Style::default().fg(dim())),
            Span::styled(format!("{pending} nuevas"), Style::default().fg(yellow())),
        ])),
        rows[2],
    );

    // List / spinner / error
    if rows[3].height == 0 {
        return;
    }
    f.render_widget(Block::default().style(Style::default().bg(bg())), rows[3]);

    if picker.loading {
        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::raw(" "),
                Span::styled(
                    "Escaneando stack del proyecto...",
                    Style::default().fg(dim()).add_modifier(Modifier::ITALIC),
                ),
            ])),
            rows[3],
        );
    } else if let Some(err) = picker.error.as_ref() {
        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::raw(" "),
                Span::styled(format!("Error: {err}"), Style::default().fg(crate::ui::theme::red())),
            ])),
            rows[3],
        );
    } else if picker.items.is_empty() {
        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::raw(" "),
                Span::styled("Sin skills recomendadas para este stack", Style::default().fg(dim())),
            ])),
            rows[3],
        );
    } else {
        let visible = rows[3].height as usize;
        let start = picker.cursor.saturating_sub(visible.saturating_sub(1));
        let lines: Vec<Line<'static>> = picker
            .items
            .iter()
            .enumerate()
            .skip(start)
            .take(visible)
            .map(|(i, item)| render_item_line(i, item, picker.cursor, overlay_w))
            .collect();
        f.render_widget(Paragraph::new(lines), rows[3]);
    }

    // Hint
    let hint_style = Style::default().fg(dim());
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::raw(" "),
            Span::styled("space ", hint_style),
            Span::styled("toggle  ", hint_style),
            Span::styled("enter ", hint_style),
            Span::styled("instalar  ", hint_style),
            Span::styled("esc ", hint_style),
            Span::styled("cerrar", hint_style),
        ])),
        rows[4],
    );
}

fn render_item_line(
    i: usize,
    item: &crate::state::autoskill_picker::AutoskillItem,
    cursor: usize,
    overlay_w: u16,
) -> Line<'static> {
    let is_cursor = i == cursor;
    let box_char = if item.installed {
        "[✓]"
    } else if item.selected {
        "[x]"
    } else {
        "[ ]"
    };
    let tag = if item.installed { " instalada" } else { " nueva" };
    let name = item.name.clone();
    let source = item.sources.first().cloned().unwrap_or_default();

    let box_color = if item.installed {
        green()
    } else if item.selected {
        cyan()
    } else {
        dim()
    };
    let tag_color = if item.installed { green() } else { yellow() };
    let name_color = if is_cursor { bg() } else { white() };
    let source_color = if is_cursor { bg() } else { dim() };

    if is_cursor {
        let pad_len = overlay_w.saturating_sub(2) as usize;
        let base = format!(" {box_char} {name}  {tag}  {source}");
        Line::from(vec![Span::styled(
            format!("{base:<pad_len$}"),
            Style::default().fg(bg()).bg(cyan()).add_modifier(Modifier::BOLD),
        )])
    } else {
        Line::from(vec![
            Span::raw(" "),
            Span::styled(box_char, Style::default().fg(box_color)),
            Span::raw(" "),
            Span::styled(name, Style::default().fg(name_color)),
            Span::styled(tag, Style::default().fg(tag_color)),
            Span::raw("  "),
            Span::styled(source, Style::default().fg(source_color)),
        ])
    }
}
