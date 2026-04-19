//! Tip card (E39).
//!
//! Banner de 1 linea mostrando un tip contextual. Se auto-selecciona desde
//! `TipState::pick` en el render — el estado no se muta aqui (la mutacion
//! de `last_shown_session` ocurre al entrar al screen, no a cada frame).

use ratatui::{
    layout::{Alignment, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::services::onboarding::Tip;
use crate::ui::theme::{bar_bg, dimmer, yellow};

/// Renderiza la tip card en una sola linea. Asume que `tip` ya fue resuelto
/// por el caller (puede venir de `TipState::pick`).
pub fn render_tip(f: &mut Frame, area: Rect, tip: &Tip) {
    let line = Line::from(vec![
        Span::styled(" 💡 Tip: ", Style::default().fg(yellow()).add_modifier(Modifier::BOLD)),
        Span::styled(tip.text.to_string(), Style::default().fg(dimmer())),
    ]);
    f.render_widget(
        Paragraph::new(line).alignment(Alignment::Left).style(Style::default().bg(bar_bg())),
        area,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::onboarding::TipScope;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    #[test]
    fn tip_renders_without_panic() {
        let backend = TestBackend::new(80, 1);
        let mut term = Terminal::new(backend).unwrap();
        let tip = Tip { id: "test", scope: TipScope::Any, text: "hola" };
        term.draw(|f| render_tip(f, Rect::new(0, 0, 80, 1), &tip)).unwrap();
    }
}
