use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph},
    Frame,
};

use crate::state::{AppState, ChatStatus};
use crate::ui::theme::{
    bar_bg, brand_blue, brand_green, dim, dimmer, factory_color, green, highlight, yellow,
};

/// Estimate how many rows the hint pairs need given the available width.
/// Each pair occupies: key.len() + action.len() + 2 (spacing).
pub fn hint_rows_needed(hint_pairs: &[(&str, &str)], available_width: u16) -> u16 {
    if hint_pairs.is_empty() || available_width == 0 {
        return 1;
    }
    let half_width = (available_width / 2).max(1) as usize;
    let mut used = 1_usize; // leading space
    let mut rows = 1_u16;
    for (key, action) in hint_pairs {
        let pair_len = key.len() + action.len() + 2;
        if used + pair_len > half_width && used > 1 {
            rows += 1;
            used = 1;
        }
        used += pair_len;
    }
    rows
}

/// Renders a hints bar — compacto una línea con branding a la derecha.
pub fn render_hints_bar(
    f: &mut Frame,
    area: Rect,
    state: &AppState,
    hint_pairs: &[(&'static str, &'static str)],
) {
    let bar_block = Block::default().style(Style::default().bg(bar_bg()));
    f.render_widget(bar_block, area);

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Fill(1), Constraint::Fill(1)])
        .split(area);

    if crate::ui::quit_armed(state) {
        f.render_widget(Paragraph::new(Line::from(armed_spans())), cols[0]);
    } else {
        // Compact single-line hints
        let lines = build_hint_lines(hint_pairs, cols[0].width as usize);
        f.render_widget(Paragraph::new(lines), cols[0]);
    }

    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("ctrl+p", Style::default().fg(dimmer())),
            Span::styled(" comandos  ", Style::default().fg(dimmer())),
            Span::styled("ingenier", Style::default().fg(brand_blue()).add_modifier(Modifier::BOLD)),
            Span::styled(
                "IA",
                Style::default().fg(brand_green()).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("  v{} ", env!("CARGO_PKG_VERSION")),
                Style::default().fg(dimmer()),
            ),
        ]))
        .alignment(Alignment::Right),
        cols[1],
    );
}

/// Título de sesión: primer mensaje del usuario truncado a ≤20 chars.
fn session_title(state: &AppState) -> String {
    use crate::state::ChatRole;
    let first = state
        .chat
        .messages
        .iter()
        .find(|m| m.role == ChatRole::User)
        .map(|m| m.content.as_str())
        .unwrap_or("");
    if first.is_empty() {
        return "Nueva conversación".to_string();
    }
    let line = first.lines().next().unwrap_or(first);
    let chars: Vec<char> = line.chars().collect();
    if chars.len() <= 20 {
        line.to_string()
    } else {
        chars[..20].iter().collect::<String>() + "…"
    }
}

/// Trunca modelo en el 3er guion: "claude-sonnet-4-20250514" → "claude-sonnet-4".
fn truncate_model(model: &str) -> &str {
    let mut count = 0;
    for (i, c) in model.char_indices() {
        if c == '-' {
            count += 1;
            if count == 3 {
                return &model[..i];
            }
        }
    }
    model
}

/// Renders a chat status bar — estilo opencode: modo/sesión/modelo/tema a la izquierda,
/// tokens/pct/atajo a la derecha, siempre una línea.
pub fn render_cost_bar(f: &mut Frame, area: Rect, state: &AppState, _hints: &[(&str, &str)]) {
    let bar = Block::default().style(Style::default().bg(bar_bg()));
    f.render_widget(bar, area);

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Fill(1), Constraint::Fill(1)])
        .split(area);

    if crate::ui::quit_armed(state) {
        f.render_widget(Paragraph::new(Line::from(armed_spans())), cols[0]);
    } else {
        let fc = factory_color(&state.factory);
        let mode_label = state.chat.agent_mode.label();
        let title = session_title(state);
        let model = truncate_model(&state.model);
        let theme_name = state.active_theme.label();
        let mut left: Vec<Span<'static>> = vec![
            Span::styled(format!(" {mode_label}"), Style::default().fg(dim())),
            Span::styled("  ", Style::default()),
            Span::styled(title, Style::default().fg(fc).add_modifier(Modifier::BOLD)),
            Span::styled("  ", Style::default()),
            Span::styled(model.to_string(), Style::default().fg(dimmer())),
            Span::styled("  ", Style::default()),
            Span::styled(theme_name.to_string(), Style::default().fg(dimmer())),
        ];
        // E34: OTPS/TTFT durante streaming
        if matches!(state.chat.status, ChatStatus::Streaming) {
            if let Some(text) =
                crate::services::chat::metrics::format_streaming_status(&state.chat.metrics)
            {
                left.push(Span::styled("  ", Style::default()));
                left.push(Span::styled(
                    text,
                    Style::default().fg(highlight()).add_modifier(Modifier::BOLD),
                ));
            }
        }
        f.render_widget(Paragraph::new(Line::from(left)), cols[0]);
    }

    let cost = &state.chat.cost;
    let pct = state.chat.context_percent();
    let pct_color = if pct < 60.0 { green() } else if pct < 80.0 { yellow() } else { dim() };
    let mut right: Vec<Span<'static>> = vec![
        Span::styled(cost.tokens_display(), Style::default().fg(dim())),
        Span::styled(
            format!(" ({:.0}%)", pct),
            Style::default().fg(pct_color).add_modifier(Modifier::BOLD),
        ),
        Span::styled("  ", Style::default()),
        Span::styled("ctrl+p", Style::default().fg(dimmer())),
        Span::styled(" comandos  ", Style::default().fg(dimmer())),
    ];
    right.push(Span::styled(
        "ingenier",
        Style::default().fg(brand_blue()).add_modifier(Modifier::BOLD),
    ));
    right.push(Span::styled(
        "IA ",
        Style::default().fg(brand_green()).add_modifier(Modifier::BOLD),
    ));
    f.render_widget(Paragraph::new(Line::from(right)).alignment(Alignment::Right), cols[1]);
}

fn armed_spans() -> Vec<Span<'static>> {
    vec![
        Span::raw(" "),
        Span::styled("ctrl+c ", Style::default().fg(yellow())),
        Span::styled(
            "otra vez para salir",
            Style::default().fg(yellow()).add_modifier(Modifier::BOLD),
        ),
    ]
}

/// Build hint text lines, wrapping into multiple rows when needed.
fn build_hint_lines<'a>(hint_pairs: &[(&'a str, &'a str)], max_width: usize) -> Vec<Line<'a>> {
    let max_w = if max_width == 0 { usize::MAX } else { max_width };
    let mut lines: Vec<Line<'a>> = Vec::new();
    let mut spans: Vec<Span<'a>> = vec![Span::raw(" ")];
    let mut used = 1_usize;

    for (key, action) in hint_pairs {
        let pair_len = key.len() + action.len() + 2;
        if used + pair_len > max_w && used > 1 {
            lines.push(Line::from(spans));
            spans = vec![Span::raw(" ")];
            used = 1;
        }
        spans.extend(crate::ui::primitives::hint_spans(key, action));
        spans.push(Span::styled("  ", Style::default()));
        used += pair_len;
    }
    if spans.len() > 1 {
        lines.push(Line::from(spans));
    }
    if lines.is_empty() {
        lines.push(Line::from(" "));
    }
    lines
}
