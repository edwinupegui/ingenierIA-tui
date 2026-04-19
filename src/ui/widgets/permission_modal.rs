use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
    Frame,
};

use crate::state::AppState;
use crate::ui::diff_render::{diff_stats, render_diff_lines, DiffRenderOpts};
use crate::ui::theme::{bg, blue, dim, green, red, surface, white, yellow};
use crate::ui::tool_display::{structured_preview, tool_icon, PreviewKind, PreviewLine};

/// Renders a bottom-anchored permission panel when tool approvals are pending.
pub fn render_permission_modal(f: &mut Frame, area: Rect, state: &AppState) {
    let approvals = &state.chat.pending_approvals;
    if approvals.is_empty() {
        return;
    }

    // Estimate height: 4 base lines + 2 per approval + 1 per validator reason
    let total_reasons: usize =
        approvals.iter().map(|a| a.validator_reasons.len() + a.reason.is_some() as usize).sum();
    let overlay_h =
        (6 + approvals.len() as u16 * 2 + total_reasons as u16).min(area.height.saturating_sub(4));
    // Ancho completo (menos 1px de margen por lado) para integrarse con el input.
    let overlay_w = area.width.saturating_sub(2).max(20);

    // Anclar al fondo: justo sobre el área de input (reservamos ~4 líneas para input+hints).
    let overlay = Rect {
        x: area.x + 1,
        y: area.y + area.height.saturating_sub(overlay_h + 4),
        width: overlay_w,
        height: overlay_h,
    };

    f.render_widget(Clear, overlay);

    // Border color based on highest risk level from enforcer
    let border_color =
        if approvals.iter().any(|a| a.permission == "critical" || a.permission == "high") {
            red()
        } else if approvals.iter().any(|a| a.permission == "medium" || a.permission == "ask") {
            yellow()
        } else {
            blue()
        };

    let panel = Block::default()
        .title(" Permiso Requerido ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border_color))
        .style(Style::default().bg(surface()));

    let inner = panel.inner(overlay);
    f.render_widget(panel, overlay);

    let mut lines: Vec<Line<'static>> = Vec::new();

    let cursor_idx = state.chat.approval_cursor.min(approvals.len().saturating_sub(1));
    for (i, approval) in approvals.iter().enumerate() {
        let risk_color = match approval.permission.as_str() {
            "critical" | "high" => red(),
            "medium" | "ask" => yellow(),
            _ => blue(),
        };

        let is_cursor = i == cursor_idx;
        let checkbox = if approval.selected { "[x]" } else { "[ ]" };
        let cursor_glyph = if is_cursor { "▶ " } else { "  " };
        let name_style = if is_cursor {
            Style::default().fg(white()).add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
        } else {
            Style::default().fg(white()).add_modifier(Modifier::BOLD)
        };

        // E32 + P1.3: cursor + checkbox + icono por tool + nivel de riesgo.
        let (icon, short) = tool_icon(&approval.tool_name);
        lines.push(Line::from(vec![
            Span::styled(cursor_glyph, Style::default().fg(risk_color)),
            Span::styled(
                format!("{checkbox} "),
                Style::default().fg(if approval.selected { green() } else { dim() }),
            ),
            Span::styled(format!("{icon} "), Style::default().fg(risk_color)),
            Span::styled(approval.tool_name.clone(), name_style),
            Span::styled(format!(" ({short})"), Style::default().fg(dim())),
            Span::styled(format!("  [{}]", approval.permission), Style::default().fg(risk_color)),
        ]));

        // E32: preview estructurado tool-specific cuando podemos parsear args.
        // Fallback al preview generico de 50 chars cuando no hay specializer.
        match structured_preview(&approval.tool_name, &approval.arguments) {
            Some(preview_lines) => push_structured_lines(&mut lines, &preview_lines, risk_color),
            None => {
                let preview = super::truncate(&approval.arguments, 50);
                if !preview.is_empty() {
                    lines.push(Line::from(vec![
                        Span::styled("  Args: ", Style::default().fg(dim())),
                        Span::styled(preview, Style::default().fg(dim())),
                    ]));
                }
            }
        }

        // Enforcer reason (subtitle)
        if let Some(reason) = &approval.reason {
            lines.push(Line::from(vec![
                Span::styled("  ⚑ ", Style::default().fg(risk_color)),
                Span::styled(reason.clone(), Style::default().fg(dim())),
            ]));
        }

        // Validator reasons (bash pipeline details)
        for r in &approval.validator_reasons {
            lines.push(Line::from(vec![
                Span::styled("    • ", Style::default().fg(dim())),
                Span::styled(r.clone(), Style::default().fg(dim())),
            ]));
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("  ", Style::default()),
        Span::styled("[Enter]", Style::default().fg(bg()).bg(green()).add_modifier(Modifier::BOLD)),
        Span::styled(" aprobar  ", Style::default().fg(green())),
        Span::styled("[Esc]", Style::default().fg(bg()).bg(red()).add_modifier(Modifier::BOLD)),
        Span::styled(" denegar  ", Style::default().fg(red())),
        Span::styled("[↑↓]", Style::default().fg(white()).add_modifier(Modifier::BOLD)),
        Span::styled(" mover  ", Style::default().fg(dim())),
        Span::styled("[Space]", Style::default().fg(white()).add_modifier(Modifier::BOLD)),
        Span::styled(" sel", Style::default().fg(dim())),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  ", Style::default()),
        Span::styled("[Y]", Style::default().fg(bg()).bg(green()).add_modifier(Modifier::BOLD)),
        Span::styled(" ok all  ", Style::default().fg(green())),
        Span::styled("[N]", Style::default().fg(bg()).bg(red()).add_modifier(Modifier::BOLD)),
        Span::styled(" deny all  ", Style::default().fg(red())),
        Span::styled("[a]", Style::default().fg(white()).add_modifier(Modifier::BOLD)),
        Span::styled(" siempre permitir  ", Style::default().fg(dim())),
        Span::styled("[d]", Style::default().fg(white()).add_modifier(Modifier::BOLD)),
        Span::styled(" siempre denegar", Style::default().fg(dim())),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  ⇧Tab: modo AUTO para aprobar siempre sin preguntar", Style::default().fg(dim())),
    ]));

    f.render_widget(Paragraph::new(lines), inner);
}

/// Renderiza lineas de preview estructurado. Diferencia el estilo segun
/// `PreviewKind`: comandos en bordered box, paths con filename resaltado,
/// patterns con quotes, text plano.
fn push_structured_lines(
    lines: &mut Vec<Line<'static>>,
    preview: &[PreviewLine],
    accent: ratatui::style::Color,
) {
    for item in preview {
        match item.kind {
            PreviewKind::Command => {
                lines.push(Line::from(vec![
                    Span::styled("  ┌─ ", Style::default().fg(accent)),
                    Span::styled(item.label.clone(), Style::default().fg(dim())),
                ]));
                lines.push(Line::from(vec![
                    Span::styled("  │ $ ", Style::default().fg(accent)),
                    Span::styled(
                        item.value.clone(),
                        Style::default().fg(white()).add_modifier(Modifier::BOLD),
                    ),
                ]));
                lines.push(Line::from(vec![Span::styled("  └─", Style::default().fg(accent))]));
            }
            PreviewKind::Path => {
                let (dir, file) = split_path_for_display(&item.value);
                let mut spans =
                    vec![Span::styled(format!("  {}: ", item.label), Style::default().fg(dim()))];
                if !dir.is_empty() {
                    spans.push(Span::styled(dir, Style::default().fg(dim())));
                }
                spans.push(Span::styled(
                    file,
                    Style::default().fg(white()).add_modifier(Modifier::BOLD),
                ));
                lines.push(Line::from(spans));
            }
            PreviewKind::Pattern => {
                lines.push(Line::from(vec![
                    Span::styled(format!("  {}: ", item.label), Style::default().fg(dim())),
                    Span::styled(item.value.clone(), Style::default().fg(yellow())),
                ]));
            }
            PreviewKind::Text => {
                lines.push(Line::from(vec![
                    Span::styled(format!("  {}: ", item.label), Style::default().fg(dim())),
                    Span::styled(item.value.clone(), Style::default().fg(dim())),
                ]));
            }
            PreviewKind::Diff => {
                let new = item.value_alt.as_deref().unwrap_or("");
                let (added, removed) = diff_stats(&item.value, new);
                lines.push(Line::from(vec![
                    Span::styled(format!("  {}: ", item.label), Style::default().fg(dim())),
                    Span::styled(
                        format!("+{added} -{removed}"),
                        Style::default().fg(accent).add_modifier(Modifier::BOLD),
                    ),
                ]));
                let diff_lines = render_diff_lines(
                    &item.value,
                    new,
                    DiffRenderOpts { max_lines: 12, indent: "    ", context: 1, file_path: None },
                );
                lines.extend(diff_lines);
            }
        }
    }
}

/// Divide un path en (directorio, filename). Si no hay separador retorna
/// `("", path)` para que el caller pueda resaltar el nombre completo.
fn split_path_for_display(path: &str) -> (String, String) {
    match path.rsplit_once('/') {
        Some((dir, file)) => (format!("{dir}/"), file.to_string()),
        None => (String::new(), path.to_string()),
    }
}
