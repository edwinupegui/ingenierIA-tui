//! Checklist de onboarding (E39).
//!
//! Se renderiza como una tarjeta compacta en el splash mostrando los 5 pasos
//! canonicos. Cada linea tiene un glyph de estado (`✓` / `○`), titulo y hint.
//! Es pasivo — no captura inputs. Una vez que todos los pasos estan hechos,
//! se auto-oculta despues de `CHECKLIST_MAX_VIEWS` visualizaciones.

use ratatui::{
    layout::{Alignment, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
    Frame,
};

use crate::services::onboarding::{ChecklistState, ChecklistStep};
use crate::ui::theme::{dim, dimmer, green, surface, white, yellow, GLYPH_IDLE, GLYPH_SUCCESS};

/// Altura total (bordes + 1 titulo + 5 pasos + 1 progress + 1 padding).
pub const CHECKLIST_HEIGHT: u16 = 10;

/// Renderiza el checklist si debe mostrarse. Retorna `true` si renderizo algo
/// (el caller puede usar esto para decidir si consume el Rect reservado).
pub fn render_checklist(f: &mut Frame, area: Rect, state: &ChecklistState) -> bool {
    if !state.should_display() || area.height < 4 {
        return false;
    }
    let progress = state.progress();
    let total = ChecklistStep::ALL.len();

    let title = format!(" Onboarding ({progress}/{total}) ");
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(dimmer()))
        .style(Style::default().bg(surface()));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let mut lines: Vec<Line<'static>> = ChecklistStep::ALL
        .iter()
        .map(|step| build_step_line(*step, state.is_done(*step)))
        .collect();
    lines.push(progress_footer(progress, total));

    f.render_widget(
        Paragraph::new(lines).alignment(Alignment::Left).style(Style::default().bg(surface())),
        inner,
    );
    true
}

fn build_step_line(step: ChecklistStep, done: bool) -> Line<'static> {
    let (glyph, glyph_color) = if done { (GLYPH_SUCCESS, green()) } else { (GLYPH_IDLE, yellow()) };
    let label_style = if done {
        Style::default().fg(dim()).add_modifier(Modifier::CROSSED_OUT)
    } else {
        Style::default().fg(white())
    };
    Line::from(vec![
        Span::styled(" ", Style::default()),
        Span::styled(glyph, Style::default().fg(glyph_color).add_modifier(Modifier::BOLD)),
        Span::styled("  ", Style::default()),
        Span::styled(step.label().to_string(), label_style),
        Span::styled("  — ", Style::default().fg(dimmer())),
        Span::styled(step.hint().to_string(), Style::default().fg(dimmer())),
    ])
}

fn progress_footer(done: usize, total: usize) -> Line<'static> {
    let pct = if total == 0 { 0 } else { (done * 100) / total };
    let text = if done == total {
        format!(" ¡Onboarding completo! ({pct}%) — se ocultara en breve")
    } else {
        format!(" Progreso: {done}/{total} ({pct}%) — avanza usando la TUI")
    };
    Line::from(Span::styled(text, Style::default().fg(dimmer())))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_footer_100_when_all_done() {
        let l = progress_footer(5, 5);
        let text: String = l.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(text.contains("completo"));
        assert!(text.contains("100%"));
    }

    #[test]
    fn progress_footer_partial_shows_fraction() {
        let l = progress_footer(2, 5);
        let text: String = l.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(text.contains("2/5"));
        assert!(text.contains("40%"));
    }

    #[test]
    fn progress_footer_handles_zero_total() {
        // Edge case: 0/0 cae en la rama "completo" (done == total). Solo
        // verificamos que no panickee y produzca algun texto renderizable.
        let l = progress_footer(0, 0);
        let text: String = l.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(!text.is_empty());
    }

    #[test]
    fn step_line_marks_done_with_checkmark() {
        let line = build_step_line(ChecklistStep::SelectFactory, true);
        let joined: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(joined.contains(GLYPH_SUCCESS));
        assert!(joined.contains("Seleccionar factory"));
    }

    #[test]
    fn step_line_shows_idle_for_undone() {
        let line = build_step_line(ChecklistStep::FirstChat, false);
        let joined: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(joined.contains(GLYPH_IDLE));
    }

    #[test]
    fn render_returns_false_when_dismissed() {
        use ratatui::backend::TestBackend;
        use ratatui::Terminal;
        let backend = TestBackend::new(80, 12);
        let mut term = Terminal::new(backend).unwrap();
        let mut state = ChecklistState::default();
        state.dismiss();
        let rect = Rect::new(0, 0, 80, CHECKLIST_HEIGHT);
        let mut rendered = true;
        term.draw(|f| {
            rendered = render_checklist(f, rect, &state);
        })
        .unwrap();
        assert!(!rendered);
    }
}
