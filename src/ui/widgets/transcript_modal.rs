//! Overlay fullscreen del transcript (E33).
//!
//! Snapshot read-only de toda la conversacion — incluye mensajes `System` y
//! tool results expandidos que suelen estar ocultos en el chat normal. Soporta
//! busqueda literal case-insensitive: el primer match selecciona la linea y se
//! hace scroll hasta visible.
//!
//! El render es puro: solo lee `state.chat.transcript`. La busqueda se recalcula
//! cada frame sobre el snapshot de lineas — no se cachea porque el costo es
//! O(n) sobre el texto plano (typically < 1ms para <500 msgs).

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap},
    Frame,
};

use crate::state::{AppState, ChatRole};
use crate::ui::theme::{bg, blue, cyan, dim, dimmer, green, purple, surface, white, yellow};

/// Dibuja el transcript si esta activo. No-op en caso contrario.
pub fn render_transcript_modal(f: &mut Frame, area: Rect, state: &AppState) {
    if !state.chat.transcript.active {
        return;
    }
    f.render_widget(Clear, area);

    let panel = Block::default()
        .title(" Transcript — Ctrl+O cerrar · Ctrl+F buscar · n/N navegar ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(cyan()))
        .style(Style::default().bg(surface()));
    let inner = panel.inner(area);
    f.render_widget(panel, area);

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(1)])
        .split(inner);

    let lines = build_transcript_lines(state);
    let matches = find_match_indices(&lines, &state.chat.transcript.query);
    let highlighted = highlight_matches(
        lines,
        &state.chat.transcript.query,
        state.chat.transcript.match_cursor,
        &matches,
    );

    render_body(f, layout[0], &highlighted, state);
    render_footer(f, layout[1], state, matches.len());
}

/// Construye las `Line` que conforman el transcript completo (sin filtros).
fn build_transcript_lines(state: &AppState) -> Vec<Line<'static>> {
    let mut out: Vec<Line<'static>> = Vec::new();
    for (idx, msg) in state.chat.messages.iter().enumerate() {
        let role_label = match msg.role {
            ChatRole::System => "sistema",
            ChatRole::User => "usuario",
            ChatRole::Assistant => "asistente",
            ChatRole::Tool => "tool",
        };
        let role_color = role_color(&msg.role);
        out.push(Line::from(vec![
            Span::styled(format!("[{idx:03}] "), Style::default().fg(dimmer())),
            Span::styled(
                role_label.to_string(),
                Style::default().fg(role_color).add_modifier(Modifier::BOLD),
            ),
        ]));
        for text_line in msg.content.lines() {
            out.push(Line::from(vec![
                Span::styled("    ", Style::default()),
                Span::styled(text_line.to_string(), Style::default().fg(white())),
            ]));
        }
        if !msg.tool_calls.is_empty() {
            for tc in &msg.tool_calls {
                out.push(Line::from(vec![
                    Span::styled("    ↳ ", Style::default().fg(dim())),
                    Span::styled(tc.name.clone(), Style::default().fg(yellow())),
                    Span::styled(format!("({})", tc.arguments), Style::default().fg(dimmer())),
                ]));
            }
        }
        out.push(Line::from(""));
    }
    if out.is_empty() {
        out.push(Line::from(vec![Span::styled(
            "  (conversacion vacia)",
            Style::default().fg(dimmer()),
        )]));
    }
    out
}

fn role_color(role: &ChatRole) -> ratatui::style::Color {
    match role {
        ChatRole::System => dim(),
        ChatRole::User => blue(),
        ChatRole::Assistant => purple(),
        ChatRole::Tool => yellow(),
    }
}

/// Indices de `lines` que contienen la query (case-insensitive). Vacio si la
/// query esta vacia.
fn find_match_indices(lines: &[Line<'_>], query: &str) -> Vec<usize> {
    if query.is_empty() {
        return Vec::new();
    }
    let needle = query.to_lowercase();
    let mut out = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let haystack: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        if haystack.to_lowercase().contains(&needle) {
            out.push(i);
        }
    }
    out
}

/// Aplica estilo al match activo (reverse video verde) y al resto (amarillo).
fn highlight_matches(
    lines: Vec<Line<'static>>,
    query: &str,
    cursor: usize,
    matches: &[usize],
) -> Vec<Line<'static>> {
    if query.is_empty() || matches.is_empty() {
        return lines;
    }
    let active_line = matches.get(cursor.min(matches.len().saturating_sub(1))).copied();
    lines
        .into_iter()
        .enumerate()
        .map(|(i, line)| {
            if !matches.contains(&i) {
                return line;
            }
            let style = if Some(i) == active_line {
                Style::default().fg(bg()).bg(green()).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(yellow()).add_modifier(Modifier::BOLD)
            };
            let joined: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
            Line::from(Span::styled(joined, style))
        })
        .collect()
}

fn render_body(f: &mut Frame, area: Rect, lines: &[Line<'static>], state: &AppState) {
    let total = lines.len() as u16;
    let max_scroll = total.saturating_sub(area.height);
    let mut scroll = state.chat.transcript.scroll_offset.min(max_scroll);
    // Auto-scroll al match activo si hay query.
    if !state.chat.transcript.query.is_empty() {
        let matches = find_match_indices(lines, &state.chat.transcript.query);
        if let Some(&target) =
            matches.get(state.chat.transcript.match_cursor.min(matches.len().saturating_sub(1)))
        {
            scroll = center_on(target as u16, area.height, max_scroll);
        }
    }
    let para = Paragraph::new(lines.to_vec())
        .wrap(Wrap { trim: false })
        .scroll((scroll, 0))
        .style(Style::default().bg(surface()));
    f.render_widget(para, area);
}

/// Calcula scroll_offset para centrar `target` en `height`. Respeta `max_scroll`.
fn center_on(target: u16, height: u16, max_scroll: u16) -> u16 {
    let half = height / 2;
    target.saturating_sub(half).min(max_scroll)
}

fn render_footer(f: &mut Frame, area: Rect, state: &AppState, match_count: usize) {
    let t = &state.chat.transcript;
    let cursor_display = if match_count == 0 { 0 } else { t.match_cursor.min(match_count - 1) + 1 };
    let spans = if t.search_active {
        vec![
            Span::styled(" /", Style::default().fg(yellow()).add_modifier(Modifier::BOLD)),
            Span::styled(t.query.clone(), Style::default().fg(white())),
            Span::styled("▏", Style::default().fg(yellow())),
            Span::styled(
                format!("  {cursor_display}/{match_count} matches · Enter/Esc salir"),
                Style::default().fg(dimmer()),
            ),
        ]
    } else if !t.query.is_empty() {
        vec![
            Span::styled(" query: ", Style::default().fg(dim())),
            Span::styled(t.query.clone(), Style::default().fg(white())),
            Span::styled(
                format!("  {cursor_display}/{match_count}  (n/N, Ctrl+F editar)"),
                Style::default().fg(dimmer()),
            ),
        ]
    } else {
        vec![Span::styled(
            " Ctrl+F buscar · ↑↓ scroll · q/Esc cerrar",
            Style::default().fg(dimmer()),
        )]
    };
    f.render_widget(Paragraph::new(Line::from(spans)).style(Style::default().bg(surface())), area);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line_of(s: &str) -> Line<'static> {
        Line::from(Span::styled(s.to_string(), Style::default()))
    }

    #[test]
    fn find_matches_is_case_insensitive() {
        let lines = vec![line_of("Hello World"), line_of("foo bar"), line_of("HELLO there")];
        let matches = find_match_indices(&lines, "hello");
        assert_eq!(matches, vec![0, 2]);
    }

    #[test]
    fn find_matches_empty_query_returns_empty() {
        let lines = vec![line_of("anything")];
        assert!(find_match_indices(&lines, "").is_empty());
    }

    #[test]
    fn center_on_clamps_to_max_scroll() {
        assert_eq!(center_on(100, 10, 50), 50);
    }

    #[test]
    fn center_on_subtracts_half_height() {
        assert_eq!(center_on(20, 10, 100), 15);
    }

    #[test]
    fn center_on_saturates_at_zero() {
        assert_eq!(center_on(2, 10, 100), 0);
    }

    #[test]
    fn highlight_preserves_non_matching_lines() {
        let lines = vec![line_of("foo"), line_of("bar"), line_of("foo again")];
        let matches = vec![0, 2];
        let hl = highlight_matches(lines, "foo", 0, &matches);
        assert_eq!(hl.len(), 3);
        // linea 1 ("bar") queda intacta (no esta en matches)
        let joined_line1: String = hl[1].spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(joined_line1, "bar");
    }
}
