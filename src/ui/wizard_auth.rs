use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use super::theme::{
    blue, dim, dimmer, green, red, white, yellow, GLYPH_CURSOR_BLOCK, GLYPH_ERROR, GLYPH_PENDING,
};
use crate::state::AppState;

pub fn render_auth_phase(f: &mut Frame, area: ratatui::layout::Rect, state: &AppState) {
    if state.wizard.selected_provider_id() == "claude-api" {
        render_claude_key_phase(f, area, state);
        return;
    }

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // title
            Constraint::Length(1), // spacing
            Constraint::Length(1), // URL
            Constraint::Length(1), // spacing
            Constraint::Length(1), // code
            Constraint::Length(1), // status
            Constraint::Length(1), // copy hint
        ])
        .split(area);

    f.render_widget(
        Paragraph::new(Span::styled(
            "Login con GitHub Copilot",
            Style::default().fg(white()).add_modifier(Modifier::BOLD),
        )),
        rows[0],
    );

    let copilot = &state.wizard.copilot;

    if copilot.user_code.is_empty() {
        f.render_widget(
            Paragraph::new(Span::styled("Conectando con GitHub...", Style::default().fg(yellow()))),
            rows[2],
        );
        return;
    }

    f.render_widget(
        Paragraph::new(Span::styled(
            &copilot.verification_uri,
            Style::default().fg(blue()).add_modifier(Modifier::UNDERLINED),
        )),
        rows[2],
    );

    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("Codigo: ", Style::default().fg(dim())),
            Span::styled(
                &copilot.user_code,
                Style::default().fg(white()).add_modifier(Modifier::BOLD),
            ),
        ])),
        rows[4],
    );

    if let Some(err) = &copilot.error {
        f.render_widget(
            Paragraph::new(Span::styled(
                format!("{GLYPH_ERROR} {err}"),
                Style::default().fg(red()),
            )),
            rows[5],
        );
    } else if copilot.auth_done {
        f.render_widget(
            Paragraph::new(Span::styled(
                format!("{GLYPH_PENDING} Autenticado — cargando modelos..."),
                Style::default().fg(green()),
            )),
            rows[5],
        );
    } else {
        let dots = ".".repeat((state.tick_count as usize % 3) + 1);
        f.render_widget(
            Paragraph::new(Span::styled(
                format!("Esperando autorizacion{dots}"),
                Style::default().fg(yellow()),
            )),
            rows[5],
        );
    }

    let recently_copied =
        copilot.code_copied_at.map(|t| state.tick_count.saturating_sub(t) < 8).unwrap_or(false);

    let copy_line = if recently_copied {
        Line::from(Span::styled("Codigo copiado", Style::default().fg(green())))
    } else {
        Line::from(vec![
            Span::styled("c ", Style::default().fg(dim())),
            Span::styled("copiar codigo", Style::default().fg(dimmer())),
        ])
    };
    f.render_widget(Paragraph::new(copy_line), rows[6]);
}

fn render_claude_key_phase(f: &mut Frame, area: ratatui::layout::Rect, state: &AppState) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // title
            Constraint::Length(1), // spacing
            Constraint::Length(1), // input
            Constraint::Length(1), // hint
            Constraint::Fill(1),
        ])
        .split(area);

    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                "API Key de Anthropic",
                Style::default().fg(white()).add_modifier(Modifier::BOLD),
            ),
            Span::styled("  console.anthropic.com", Style::default().fg(dim())),
        ])),
        rows[0],
    );

    let key_text = if state.wizard.claude_api_key.is_empty() {
        Line::from(vec![
            Span::styled(GLYPH_CURSOR_BLOCK, Style::default().fg(blue())),
            Span::styled("sk-ant-...", Style::default().fg(dim())),
        ])
    } else {
        // Mask the key for security, show only last 4 chars
        let key = &state.wizard.claude_api_key;
        let masked: String = if key.len() > 8 {
            format!("{}...{}", &key[..7], &key[key.len() - 4..])
        } else {
            "*".repeat(key.len())
        };
        let cursor = state.wizard.claude_key_cursor.min(masked.len());
        let before = &masked[..cursor.min(masked.len())];
        let after = &masked[cursor.min(masked.len())..];
        let mut spans = Vec::new();
        if !before.is_empty() {
            spans.push(Span::styled(before.to_string(), Style::default().fg(white())));
        }
        spans.push(Span::styled(GLYPH_CURSOR_BLOCK, Style::default().fg(blue())));
        if !after.is_empty() {
            spans.push(Span::styled(after.to_string(), Style::default().fg(white())));
        }
        Line::from(spans)
    };
    f.render_widget(Paragraph::new(key_text), rows[2]);

    f.render_widget(
        Paragraph::new(Span::styled(
            "Pega tu API key y presiona Enter",
            Style::default().fg(dimmer()),
        )),
        rows[3],
    );
}
