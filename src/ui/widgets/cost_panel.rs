use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
    Frame,
};

use crate::state::cost::format_money;
use crate::state::AppState;
use crate::ui::theme::{cyan, dim, green, red, surface, white, yellow, GLYPH_IDLE};

/// Anchura del overlay de costos.
const PANEL_WIDTH: u16 = 56;
/// Altura del overlay (caben 14 lineas + bordes).
const PANEL_HEIGHT: u16 = 18;

pub fn render_cost_panel(f: &mut Frame, area: Rect, state: &AppState) {
    let overlay = compute_overlay(area);
    f.render_widget(Clear, overlay);

    let cost = &state.chat.cost;
    let title = format!(" Costo de Sesion  {} ", cost.cost_display());
    let panel = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(green()))
        .style(Style::default().bg(surface()));

    let inner = panel.inner(overlay);
    f.render_widget(panel, overlay);

    if cost.turn_count == 0 && cost.total_tokens() == 0 {
        crate::ui::primitives::render_empty_state(
            f,
            inner,
            GLYPH_IDLE,
            "Sin datos de costo",
            Some("Inicia una conversacion"),
        );
        return;
    }

    f.render_widget(Paragraph::new(build_lines(cost)), inner);
}

fn compute_overlay(area: Rect) -> Rect {
    let h = PANEL_HEIGHT.min(area.height.saturating_sub(2));
    let w = PANEL_WIDTH.min(area.width.saturating_sub(4));
    Rect {
        x: area.x + (area.width.saturating_sub(w)) / 2,
        y: area.y + (area.height.saturating_sub(h)) / 2,
        width: w,
        height: h,
    }
}

fn build_lines(cost: &crate::state::CostState) -> Vec<Line<'static>> {
    let mut lines = Vec::with_capacity(14);
    lines.push(model_header(cost));
    lines.push(separator());
    lines.extend(token_rows(cost));
    lines.push(separator());
    lines.push(turn_summary(cost));
    lines.push(tools_summary(cost));
    if cost.cache_read_input + cost.cache_creation_input > 0 {
        lines.push(separator());
        lines.extend(cache_rows(cost));
    }
    lines.push(separator());
    lines.push(budget_row(cost));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  ESC para cerrar",
        Style::default().fg(yellow()).add_modifier(Modifier::BOLD),
    )));
    lines
}

fn model_header(cost: &crate::state::CostState) -> Line<'static> {
    Line::from(vec![
        Span::styled("  Modelo: ", Style::default().fg(dim())),
        Span::styled(cost.model_display_name().to_string(), Style::default().fg(white())),
    ])
}

fn separator() -> Line<'static> {
    Line::from(Span::styled("  ──────────────────────────────────────", Style::default().fg(dim())))
}

fn token_rows(cost: &crate::state::CostState) -> Vec<Line<'static>> {
    vec![
        token_row("Input fresh ", cost.total_input, cost.input_cost(), white()),
        token_row("Output      ", cost.total_output, cost.output_cost(), white()),
    ]
}

fn cache_rows(cost: &crate::state::CostState) -> Vec<Line<'static>> {
    vec![
        token_row("Cache write ", cost.cache_creation_input, cost.cache_write_cost(), cyan()),
        token_row("Cache read  ", cost.cache_read_input, cost.cache_read_cost(), green()),
        cache_savings_row(cost),
    ]
}

fn token_row(
    label: &'static str,
    tokens: u32,
    cost: f64,
    color: ratatui::style::Color,
) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("  {label}"), Style::default().fg(dim())),
        Span::styled(format!("{tokens:>9} tok"), Style::default().fg(color)),
        Span::styled(format!("  {}", format_money(cost)), Style::default().fg(green())),
    ])
}

fn cache_savings_row(cost: &crate::state::CostState) -> Line<'static> {
    let hit = cost.cache_hit_ratio();
    let savings = cost.cache_savings();
    let hit_color = if hit >= 50.0 {
        green()
    } else if hit >= 20.0 {
        yellow()
    } else {
        dim()
    };
    Line::from(vec![
        Span::styled("  Hit ratio   ", Style::default().fg(dim())),
        Span::styled(format!("{hit:>6.1}%   "), Style::default().fg(hit_color)),
        Span::styled(format!("ahorro {}", format_money(savings)), Style::default().fg(green())),
    ])
}

fn turn_summary(cost: &crate::state::CostState) -> Line<'static> {
    let avg = if cost.turn_count > 0 { cost.total_cost() / cost.turn_count as f64 } else { 0.0 };
    Line::from(vec![
        Span::styled(format!("  Turns: {}", cost.turn_count), Style::default().fg(white())),
        Span::styled(format!("  │  Avg: {}/turn", format_money(avg)), Style::default().fg(dim())),
    ])
}

fn tools_summary(cost: &crate::state::CostState) -> Line<'static> {
    Line::from(Span::styled(
        format!("  Tools: {} llamadas", cost.tool_calls),
        Style::default().fg(white()),
    ))
}

fn budget_row(cost: &crate::state::CostState) -> Line<'static> {
    let pct = cost.budget_percent();
    let color = if pct >= 100.0 {
        red()
    } else if pct >= 80.0 {
        yellow()
    } else {
        green()
    };
    let bar = bar_string(pct, 20);
    Line::from(vec![
        Span::styled("  Budget ", Style::default().fg(dim())),
        Span::styled(bar, Style::default().fg(color)),
        Span::styled(
            format!(" {}/{}", cost.cost_display(), format_money(cost.session_budget)),
            Style::default().fg(color),
        ),
    ])
}

fn bar_string(percent: f64, width: usize) -> String {
    let filled = ((percent / 100.0) * width as f64).round().clamp(0.0, width as f64) as usize;
    let mut s = String::with_capacity(width + 2);
    s.push('[');
    for i in 0..width {
        s.push(if i < filled { '█' } else { '░' });
    }
    s.push(']');
    s
}
