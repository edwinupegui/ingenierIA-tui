//! Message navigator — minimap vertical de turnos del chat.
//!
//! Renderiza una barra a la derecha del timeline con un tick por cada
//! mensaje del usuario. Permite saltar entre turnos con `[` / `]` y expandir
//! a una columna más ancha con previews con `Ctrl+G` (ver `keys_chat.rs`).
//!
//! Inspirado en `message-nav.tsx:34-45` de opencode-dev (modos compact /
//! normal con preview + diff bars).
//!
//! Sin efectos secundarios: toma slice de mensajes + cursor + theme y pinta.
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::state::{ChatMessage, ChatRole};
use ingenieria_ui::theme::ColorTheme;

/// Ancho fijo del navigator compacto (incluye padding izquierdo de 1).
pub const COMPACT_WIDTH: u16 = 2;

/// Ancho del navigator expandido (preview de turnos).
pub const EXPANDED_WIDTH: u16 = 22;

/// Ancho que debe reservar el caller para el navigator según el estado.
pub fn width(expanded: bool) -> u16 {
    if expanded {
        EXPANDED_WIDTH
    } else {
        COMPACT_WIDTH
    }
}

/// Render del message navigator.
///
/// - `area`: rect reservado por el caller (debe coincidir con `width(...)`).
/// - `messages`: todos los mensajes del chat.
/// - `cursor`: índice (sobre user messages) del turno seleccionado. `None`
///   fuerza highlight del último.
/// - `expanded`: si renderizar preview extendido.
/// - `colors`: tema activo.
pub fn render(
    f: &mut Frame,
    area: Rect,
    messages: &[ChatMessage],
    cursor: Option<usize>,
    expanded: bool,
    colors: ColorTheme,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let user_indices: Vec<usize> = messages
        .iter()
        .enumerate()
        .filter(|(_, m)| m.role == ChatRole::User)
        .map(|(i, _)| i)
        .collect();

    if user_indices.is_empty() {
        return;
    }

    let selected = cursor.unwrap_or(user_indices.len() - 1).min(user_indices.len() - 1);
    let rows = row_assignments(user_indices.len(), area.height as usize);

    let lines: Vec<Line<'_>> = (0..area.height as usize)
        .map(|row| render_row(row, &rows, &user_indices, messages, selected, expanded, colors))
        .collect();

    let para = Paragraph::new(lines).style(Style::default().bg(colors.bg));
    f.render_widget(para, area);
}

/// Asigna cada fila visible a un user message (o None para huecos).
///
/// Cuando hay más filas que turnos, los turnos se espacian dejando huecos.
/// Cuando hay más turnos que filas, se comprimen varios turnos por fila y
/// solo se representan los más recientes.
fn row_assignments(n_turns: usize, n_rows: usize) -> Vec<Option<usize>> {
    if n_rows == 0 || n_turns == 0 {
        return Vec::new();
    }
    let mut out = vec![None; n_rows];
    if n_turns >= n_rows {
        let start = n_turns - n_rows;
        for (row, turn) in (start..n_turns).enumerate() {
            out[row] = Some(turn);
        }
    } else {
        let denom = (n_turns - 1).max(1) as f32;
        for turn in 0..n_turns {
            let position = (turn as f32 * (n_rows - 1) as f32 / denom).round() as usize;
            out[position.min(n_rows - 1)] = Some(turn);
        }
    }
    out
}

fn render_row<'a>(
    row: usize,
    assignments: &[Option<usize>],
    user_indices: &[usize],
    messages: &'a [ChatMessage],
    selected: usize,
    expanded: bool,
    colors: ColorTheme,
) -> Line<'a> {
    let turn = assignments.get(row).copied().flatten();
    let is_selected = turn == Some(selected);

    let glyph = match turn {
        Some(_) if is_selected => "▐",
        Some(_) => "│",
        None => " ",
    };
    let tick_style = if is_selected {
        Style::default().fg(colors.accent).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(colors.text_dim)
    };

    let mut spans: Vec<Span<'a>> =
        vec![Span::styled(" ", Style::default()), Span::styled(glyph, tick_style)];

    if expanded {
        if let Some(t) = turn {
            let msg_idx = user_indices[t];
            let preview = preview_text(&messages[msg_idx].content, EXPANDED_WIDTH as usize - 3);
            let prev_style = if is_selected {
                Style::default().fg(colors.text).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(colors.text_dim)
            };
            spans.push(Span::styled(format!(" {preview}"), prev_style));
        }
    }

    Line::from(spans)
}

fn preview_text(content: &str, max: usize) -> String {
    let first = content.lines().next().unwrap_or("").trim();
    if first.chars().count() <= max {
        first.to_string()
    } else {
        let cut: String = first.chars().take(max.saturating_sub(1)).collect();
        format!("{cut}…")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn width_respects_expanded() {
        assert_eq!(width(false), COMPACT_WIDTH);
        assert_eq!(width(true), EXPANDED_WIDTH);
    }

    #[test]
    fn row_assignments_empty_when_no_rows() {
        assert!(row_assignments(5, 0).is_empty());
        assert!(row_assignments(0, 5).iter().all(|s| s.is_none()));
    }

    #[test]
    fn row_assignments_one_per_row_when_more_turns_than_rows() {
        let out = row_assignments(10, 5);
        assert_eq!(out.len(), 5);
        let filled: Vec<usize> = out.iter().filter_map(|&s| s).collect();
        assert_eq!(filled, vec![5, 6, 7, 8, 9]);
    }

    #[test]
    fn row_assignments_spaces_turns_when_more_rows() {
        let out = row_assignments(3, 10);
        let filled = out.iter().filter(|s| s.is_some()).count();
        assert_eq!(filled, 3);
        assert_eq!(out[0], Some(0));
        assert_eq!(out[9], Some(2));
    }

    #[test]
    fn preview_truncates_long_content() {
        let p = preview_text("lorem ipsum dolor sit amet consectetur", 10);
        assert!(p.chars().count() <= 10);
        assert!(p.ends_with('…'));
    }

    #[test]
    fn preview_keeps_short_content_intact() {
        let p = preview_text("hola", 10);
        assert_eq!(p, "hola");
    }
}
