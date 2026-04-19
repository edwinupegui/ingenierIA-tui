//! Widget overlay para Ctrl+R input history search (E30b).

use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Clear, Paragraph},
    Frame,
};

use crate::state::history_search::HistorySearch;
use crate::ui::design_system::Pane;
use crate::ui::theme::{blue, cyan, dim, white};

/// Ancho fijo del modal — alineado con el resto de overlays.
const MODAL_WIDTH: u16 = 70;

pub fn render(f: &mut Frame, area: Rect, search: Option<&HistorySearch>, history: &[String]) {
    let Some(search) = search else { return };
    let visible = search.matches.len().min(area.height.saturating_sub(6) as usize);
    let modal_h: u16 = (visible as u16 + 6).max(7);
    let modal_w = MODAL_WIDTH.min(area.width.saturating_sub(2));

    let modal = Rect {
        x: area.x + (area.width.saturating_sub(modal_w)) / 2,
        y: area.y + (area.height.saturating_sub(modal_h)) / 2,
        width: modal_w,
        height: modal_h,
    };
    f.render_widget(Clear, modal);

    let title = format!(" historial (Ctrl+R) — {} coincidencias ", search.matches.len());
    let inner =
        Pane::new().title(&title).border_style(Style::default().fg(blue())).render(f, modal);

    let mut lines: Vec<Line<'static>> = Vec::with_capacity(visible + 2);
    let prompt = format!(" › {}", search.query);
    lines.push(Line::from(vec![Span::styled(prompt, Style::default().fg(cyan()))]));
    lines.push(Line::from(""));

    for (row, (idx, _score)) in search.matches.iter().take(visible).enumerate() {
        let entry = history.get(*idx).map(String::as_str).unwrap_or("(missing)");
        let is_selected = row == search.cursor;
        let style = if is_selected {
            Style::default().fg(white()).bg(blue()).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(white())
        };
        let arrow = if is_selected { " ▸ " } else { "   " };
        lines.push(Line::from(vec![
            Span::styled(arrow, Style::default().fg(blue())),
            Span::styled(truncate(entry, modal_w as usize - 5), style),
        ]));
    }

    if search.matches.is_empty() {
        lines.push(Line::from(Span::styled("  (sin coincidencias)", Style::default().fg(dim()))));
    }

    f.render_widget(Paragraph::new(lines), inner);
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let head: String = s.chars().take(max.saturating_sub(1)).collect();
        format!("{head}…")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{backend::TestBackend, Terminal};

    fn render_to_buffer(search: &HistorySearch, history: &[String]) -> String {
        let backend = TestBackend::new(80, 20);
        let mut term = Terminal::new(backend).unwrap();
        term.draw(|f| {
            let area = f.area();
            render(f, area, Some(search), history);
        })
        .unwrap();
        let buf = term.backend().buffer().clone();
        let mut out = String::new();
        for y in 0..buf.area.height {
            for x in 0..buf.area.width {
                out.push_str(buf[(x, y)].symbol());
            }
            out.push('\n');
        }
        out
    }

    #[test]
    fn render_shows_matches() {
        let mut s = HistorySearch::new();
        let history = vec!["/help".to_string(), "/clear".to_string(), "/cron-list".to_string()];
        s.recompute(&history);
        let frame = render_to_buffer(&s, &history);
        assert!(frame.contains("historial"));
        assert!(frame.contains("/cron-list"));
    }

    #[test]
    fn render_shows_empty_state() {
        let mut s = HistorySearch::new();
        s.query = "no-match".into();
        let history = vec!["/help".to_string()];
        s.recompute(&history);
        let frame = render_to_buffer(&s, &history);
        assert!(frame.contains("sin coincidencias"));
    }

    #[test]
    fn truncate_respects_max() {
        assert_eq!(truncate("short", 10), "short");
        assert!(truncate("a very long string that wont fit", 10).chars().count() <= 10);
    }
}
