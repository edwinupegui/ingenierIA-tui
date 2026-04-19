//! Panel de todos (E12): seccion compacta dentro del agent panel con la
//! TodoList activa. Se oculta cuando la lista esta vacia.
//!
//! Limite de filas: `TODO_SECTION_ROWS` (10) — si hay mas todos, se trunca
//! con una linea `… +N mas` al final.

use ratatui::{
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::domain::todos::{TodoList, TodoStatus};
use crate::ui::theme::{blue, cyan, dim, green, yellow};

pub const TODO_SECTION_ROWS: u16 = 10;

/// Renderiza el panel. Llamar solo cuando `!list.is_empty()`.
pub fn render(f: &mut Frame, area: Rect, list: &TodoList) {
    let header = Line::from(vec![
        Span::styled("Todos ", Style::default().fg(blue())),
        Span::styled(format!("({})", list.short_summary()), Style::default().fg(dim())),
    ]);

    let mut lines: Vec<Line<'static>> = vec![header];
    let max_rows = area.height.saturating_sub(1) as usize;
    let shown = list.items.iter().take(max_rows);
    let remaining = list.items.len().saturating_sub(max_rows);

    for item in shown {
        let (glyph_color, title_style) = style_for(item.status);
        lines.push(Line::from(vec![
            Span::styled(format!(" {} ", item.status.glyph()), Style::default().fg(glyph_color)),
            Span::styled(format!("#{:<3}", item.id), Style::default().fg(cyan())),
            Span::styled(format!(" {}", truncate(&item.title, 60)), title_style),
        ]));
    }

    if remaining > 0 {
        lines.push(Line::from(Span::styled(
            format!("   … +{remaining} mas"),
            Style::default().fg(dim()),
        )));
    }

    f.render_widget(Paragraph::new(lines), area);
}

fn style_for(status: TodoStatus) -> (ratatui::style::Color, Style) {
    match status {
        TodoStatus::Pending => (yellow(), Style::default().fg(dim())),
        TodoStatus::InProgress => (blue(), Style::default()),
        TodoStatus::Completed => (
            green(),
            Style::default().fg(dim()).add_modifier(ratatui::style::Modifier::CROSSED_OUT),
        ),
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let take = max.saturating_sub(1);
    let mut out: String = s.chars().take(take).collect();
    out.push('…');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::todos::TodoList;

    #[test]
    fn truncate_respects_char_count() {
        assert_eq!(truncate("hola", 10), "hola");
        assert_eq!(truncate("hola mundo largo", 7), "hola m…");
    }

    #[test]
    fn style_for_completed_uses_strikethrough() {
        let (_color, style) = style_for(TodoStatus::Completed);
        assert!(style.add_modifier.contains(ratatui::style::Modifier::CROSSED_OUT));
    }

    #[test]
    fn todo_section_rows_is_positive() {
        assert_eq!(TODO_SECTION_ROWS, 10);
        let mut list = TodoList::new();
        for i in 0..20 {
            list.add(format!("item {i}"));
        }
        assert_eq!(list.len(), 20);
    }
}
