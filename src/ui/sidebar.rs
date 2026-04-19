//! Sidebar derecho del chat — contexto, tokens, LSP y metadata de sesión.
//!
//! Renderizado solo cuando el terminal tiene ≥ 100 columnas.
//! Ancho fijo: 28 columnas.

use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph, Wrap},
    Frame,
};

use super::theme::{
    accent, bg, dim, dimmer, green, muted, red, surface, white, yellow,
};
use crate::state::AppState;

/// Ancho mínimo del terminal para mostrar el sidebar.
pub const SIDEBAR_MIN_TERMINAL_WIDTH: u16 = 100;

/// Ancho del sidebar en columnas.
pub const SIDEBAR_WIDTH: u16 = 28;

/// Renderiza el sidebar derecho del chat.
pub fn render(f: &mut Frame, area: Rect, state: &AppState) {
    let bg_block = Block::default().style(Style::default().bg(bg()));
    f.render_widget(bg_block, area);

    // Borde superior sutil para separar del área principal
    let border_col = Rect { x: area.x, y: area.y, width: 1, height: area.height };
    let border_block = Block::default().style(Style::default().bg(surface()));
    f.render_widget(border_block, border_col);

    let content = Rect {
        x: area.x + 2,
        y: area.y,
        width: area.width.saturating_sub(2),
        height: area.height,
    };

    let mut lines: Vec<Line<'static>> = Vec::new();

    push_conversation_title(&mut lines, state);
    push_blank(&mut lines);
    push_context_section(&mut lines, state);
    push_blank(&mut lines);
    push_lsp_section(&mut lines, state);
    push_blank(&mut lines);
    push_model_info(&mut lines, state);
    push_blank(&mut lines);
    push_cwd_info(&mut lines, state);

    let paragraph = Paragraph::new(lines)
        .style(Style::default().bg(bg()))
        .wrap(Wrap { trim: true });

    f.render_widget(paragraph, content);
}

fn push_blank(lines: &mut Vec<Line<'static>>) {
    lines.push(Line::from(""));
}

fn push_conversation_title(lines: &mut Vec<Line<'static>>, state: &AppState) {
    let title = state
        .chat
        .messages
        .iter()
        .find(|m| m.role == crate::state::ChatRole::User)
        .map(|m| {
            let first_line = m.content.lines().next().unwrap_or("").trim().to_string();
            truncate_to(first_line, 24)
        })
        .unwrap_or_else(|| "Nueva conversación".to_string());

    lines.push(Line::from(vec![Span::styled(
        title,
        Style::default().fg(white()).add_modifier(Modifier::BOLD),
    )]));
}

fn push_context_section(lines: &mut Vec<Line<'static>>, state: &AppState) {
    lines.push(Line::from(vec![Span::styled(
        "Contexto",
        Style::default().fg(dim()).add_modifier(Modifier::BOLD),
    )]));

    let cost = &state.chat.cost;
    let total_tokens = cost.total_tokens();

    if total_tokens == 0 {
        lines.push(Line::from(vec![Span::styled("sin tokens aún", Style::default().fg(dimmer()))]));
        return;
    }

    let tokens_str = format_tokens_k(total_tokens);
    lines.push(Line::from(vec![Span::styled(
        format!("{tokens_str} tokens"),
        Style::default().fg(white()),
    )]));

    let pct = state.chat.context_percent();
    let pct_color = if pct < 60.0 { green() } else if pct < 80.0 { yellow() } else { red() };
    lines.push(Line::from(vec![Span::styled(
        format!("{pct:.0}% usado"),
        Style::default().fg(pct_color).add_modifier(Modifier::BOLD),
    )]));

    let cost_str = cost.cost_display();
    lines.push(Line::from(vec![Span::styled(
        format!("{cost_str} gastado"),
        Style::default().fg(dim()),
    )]));
}

fn push_lsp_section(lines: &mut Vec<Line<'static>>, state: &AppState) {
    lines.push(Line::from(vec![Span::styled(
        "LSP",
        Style::default().fg(dim()).add_modifier(Modifier::BOLD),
    )]));

    let lsp = &state.lsp;
    if let Some(ref err) = lsp.error {
        lines.push(Line::from(vec![Span::styled(
            truncate_to(err.clone(), 24),
            Style::default().fg(red()),
        )]));
    } else if lsp.connected {
        let name = lsp.server_name.as_deref().unwrap_or("desconocido");
        let diag_count = lsp.diagnostics.len();
        lines.push(Line::from(vec![Span::styled(
            name.to_string(),
            Style::default().fg(green()),
        )]));
        if diag_count > 0 {
            lines.push(Line::from(vec![Span::styled(
                format!("{diag_count} diagnóstico(s)"),
                Style::default().fg(yellow()),
            )]));
        } else {
            lines.push(Line::from(vec![Span::styled("sin errores", Style::default().fg(dimmer()))]));
        }
    } else {
        lines.push(Line::from(vec![Span::styled(
            "activa al leer archivos",
            Style::default().fg(muted()),
        )]));
    }
}

fn push_model_info(lines: &mut Vec<Line<'static>>, state: &AppState) {
    lines.push(Line::from(vec![Span::styled(
        "Modelo",
        Style::default().fg(dim()).add_modifier(Modifier::BOLD),
    )]));
    lines.push(Line::from(vec![Span::styled(
        truncate_to(state.model.clone(), 24),
        Style::default().fg(accent()),
    )]));
}

fn push_cwd_info(lines: &mut Vec<Line<'static>>, state: &AppState) {
    let cwd = &state.working_dir;
    if cwd.is_empty() {
        return;
    }
    let home = std::env::var("HOME").unwrap_or_default();
    let display = if !home.is_empty() && cwd.starts_with(&home) {
        format!("~{}", &cwd[home.len()..])
    } else {
        cwd.clone()
    };
    lines.push(Line::from(vec![Span::styled(
        truncate_to(display, 24),
        Style::default().fg(dimmer()),
    )]));
    if let Some(branch) = &state.git_branch {
        lines.push(Line::from(vec![
            Span::styled("⎇ ", Style::default().fg(muted())),
            Span::styled(truncate_to(branch.clone(), 22), Style::default().fg(muted())),
        ]));
    }
}


fn format_tokens_k(tokens: u32) -> String {
    if tokens >= 1_000 {
        format!("{:.1}K", tokens as f64 / 1_000.0)
    } else {
        tokens.to_string()
    }
}

fn truncate_to(s: String, max: usize) -> String {
    if s.chars().count() <= max {
        s
    } else {
        let end = s
            .char_indices()
            .map(|(i, _)| i)
            .take_while(|&i| i < max.saturating_sub(1))
            .last()
            .unwrap_or(0);
        format!("{}…", &s[..end])
    }
}
