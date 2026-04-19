//! Modal overlay para MCP Elicitation (E18).
//!
//! Se dibuja centrado, con estilo similar a `permission_modal`. El render es
//! puro: solo lee `state.chat.pending_elicitation`. El handler de teclado
//! vive en `app/elicitation_handler.rs`.

use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
    Frame,
};

use crate::services::mcp::elicitation::ElicitationField;
use crate::state::chat_types::PendingElicitation;
use crate::state::AppState;
use crate::ui::theme::{
    bg, blue, cyan, dim, green, red, surface, white, yellow, GLYPH_CURSOR_BLOCK,
};

/// Renderiza el modal si hay una elicitation pendiente.
pub fn render_elicitation_modal(f: &mut Frame, area: Rect, state: &AppState) {
    let Some(pending) = state.chat.pending_elicitation.as_ref() else {
        return;
    };

    let overlay = overlay_rect(area, pending);
    f.render_widget(Clear, overlay);

    let panel = Block::default()
        .title(title_for(pending))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(cyan()))
        .style(Style::default().bg(surface()));

    let inner = panel.inner(overlay);
    f.render_widget(panel, overlay);

    let mut lines: Vec<Line<'static>> = Vec::new();
    lines.push(header_line(pending));
    if let Some(src) = pending.request.source.as_ref() {
        lines.push(Line::from(vec![
            Span::styled("  desde: ", Style::default().fg(dim())),
            Span::styled(src.clone(), Style::default().fg(blue())),
        ]));
    }
    lines.push(Line::from(""));

    append_field_lines(&mut lines, pending);
    lines.push(Line::from(""));
    lines.push(hint_line(&pending.request.field));

    f.render_widget(Paragraph::new(lines), inner);
}

fn title_for(pending: &PendingElicitation) -> String {
    format!(" Solicitud ({}) ", pending.request.field.kind_label())
}

fn overlay_rect(area: Rect, pending: &PendingElicitation) -> Rect {
    let n_opts = pending.request.field.option_count() as u16;
    let base_h: u16 = 8;
    let extra = n_opts.min(8); // cap de 8 opciones visibles
    let overlay_h = (base_h + extra).min(area.height.saturating_sub(4));
    let overlay_w = 70u16.min(area.width.saturating_sub(4));
    Rect {
        x: area.x + (area.width.saturating_sub(overlay_w)) / 2,
        y: area.y + (area.height.saturating_sub(overlay_h)) / 2,
        width: overlay_w,
        height: overlay_h,
    }
}

fn header_line(pending: &PendingElicitation) -> Line<'static> {
    Line::from(vec![
        Span::styled("  ", Style::default()),
        Span::styled(
            pending.request.message.clone(),
            Style::default().fg(white()).add_modifier(Modifier::BOLD),
        ),
    ])
}

fn append_field_lines(lines: &mut Vec<Line<'static>>, pending: &PendingElicitation) {
    match &pending.request.field {
        ElicitationField::Text { label, placeholder } => {
            append_text_field(lines, label, placeholder, &pending.text_buffer);
        }
        ElicitationField::Select { label, options } => {
            append_select_field(lines, label, options, pending.cursor);
        }
        ElicitationField::Confirm { prompt } => {
            append_confirm_field(lines, prompt);
        }
        ElicitationField::MultiSelect { label, options } => {
            append_multi_field(lines, label, options, pending);
        }
    }
}

fn append_text_field(lines: &mut Vec<Line<'static>>, label: &str, placeholder: &str, buffer: &str) {
    lines.push(Line::from(vec![
        Span::styled("  ", Style::default()),
        Span::styled(label.to_string(), Style::default().fg(dim())),
    ]));
    let shown: Vec<Span<'static>> = if buffer.is_empty() {
        vec![Span::styled(
            format!("  {placeholder}"),
            Style::default().fg(dim()).add_modifier(Modifier::ITALIC),
        )]
    } else {
        vec![
            Span::styled("  ", Style::default()),
            Span::styled(buffer.to_string(), Style::default().fg(white())),
            Span::styled(GLYPH_CURSOR_BLOCK, Style::default().fg(blue())),
        ]
    };
    lines.push(Line::from(shown));
}

fn append_select_field(
    lines: &mut Vec<Line<'static>>,
    label: &str,
    options: &[String],
    cursor: usize,
) {
    lines.push(Line::from(vec![
        Span::styled("  ", Style::default()),
        Span::styled(label.to_string(), Style::default().fg(dim())),
    ]));
    for (idx, opt) in options.iter().take(8).enumerate() {
        let is_active = idx == cursor;
        let prefix = if is_active { "  ▸ " } else { "    " };
        let style = if is_active {
            Style::default().fg(bg()).bg(blue()).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(white())
        };
        lines.push(Line::from(vec![
            Span::styled(prefix, Style::default().fg(if is_active { blue() } else { dim() })),
            Span::styled(opt.clone(), style),
        ]));
    }
}

fn append_confirm_field(lines: &mut Vec<Line<'static>>, prompt: &str) {
    lines.push(Line::from(vec![
        Span::styled("  ", Style::default()),
        Span::styled(prompt.to_string(), Style::default().fg(yellow())),
    ]));
}

fn append_multi_field(
    lines: &mut Vec<Line<'static>>,
    label: &str,
    options: &[String],
    pending: &PendingElicitation,
) {
    lines.push(Line::from(vec![
        Span::styled("  ", Style::default()),
        Span::styled(label.to_string(), Style::default().fg(dim())),
    ]));
    for (idx, opt) in options.iter().take(8).enumerate() {
        let is_active = idx == pending.cursor;
        let checked = pending.multi_selected.contains(&idx);
        let glyph = if checked { "[x]" } else { "[ ]" };
        let prefix = if is_active { "  ▸ " } else { "    " };
        let glyph_color = if checked { green() } else { dim() };
        let text_style = if is_active {
            Style::default().fg(bg()).bg(blue()).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(white())
        };
        lines.push(Line::from(vec![
            Span::styled(prefix, Style::default().fg(if is_active { blue() } else { dim() })),
            Span::styled(glyph.to_string(), Style::default().fg(glyph_color)),
            Span::styled(" ", Style::default()),
            Span::styled(opt.clone(), text_style),
        ]));
    }
}

fn hint_line(field: &ElicitationField) -> Line<'static> {
    let hints: Vec<Span<'static>> = match field {
        ElicitationField::Text { .. } => {
            vec![hotkey("Enter", "Aceptar", green()), sep(), hotkey("Esc", "Cancelar", red())]
        }
        ElicitationField::Select { .. } => vec![
            hotkey("↑↓", "Navegar", blue()),
            sep(),
            hotkey("Enter", "Aceptar", green()),
            sep(),
            hotkey("Esc", "Cancelar", red()),
        ],
        ElicitationField::Confirm { .. } => vec![
            hotkey("Y", "Si", green()),
            sep(),
            hotkey("N", "No", red()),
            sep(),
            hotkey("Esc", "Cancelar", dim()),
        ],
        ElicitationField::MultiSelect { .. } => vec![
            hotkey("↑↓", "Navegar", blue()),
            sep(),
            hotkey("Space", "Alternar", cyan()),
            sep(),
            hotkey("Enter", "Aceptar", green()),
            sep(),
            hotkey("Esc", "Cancelar", red()),
        ],
    };
    let mut all = vec![Span::styled("  ", Style::default())];
    all.extend(hints);
    Line::from(all)
}

fn hotkey(key: &str, label: &str, color: ratatui::style::Color) -> Span<'static> {
    Span::styled(
        format!("[{key}] {label}"),
        Style::default().fg(color).add_modifier(Modifier::BOLD),
    )
}

fn sep() -> Span<'static> {
    Span::styled("  ", Style::default().fg(dim()))
}
