//! Estimacion de altura (en lineas visuales) por mensaje del chat.
//!
//! Se usa junto con `VirtualWindow::compute()` para determinar qué mensajes
//! caen dentro del viewport y evitar construir `Line`s de los que quedan fuera.
//!
//! La precision no necesita ser perfecta: `VIRTUAL_OVERSCAN` agrega un colchon
//! de 2 mensajes arriba/abajo para absorber desviaciones.

use crate::state::{ChatDisplayMode, ChatMessage, ChatRole, BRIEF_MAX_LINES};

/// Overhead fijo por mensaje de usuario: 0 (sin label, sin blank — el separador lo agrega).
const USER_OVERHEAD: u16 = 0;

/// Overhead fijo por mensaje de asistente: 0 (sin label, sin blank — el separador lo agrega).
const ASSISTANT_OVERHEAD: u16 = 0;

/// Estima cuantas lineas visuales ocupa un mensaje dado un ancho de wrap.
///
/// `has_separator` indica si antes de este mensaje se dibuja un separador de
/// turno (1 linea extra). Calcula sin construir `Line`s ni `Span`s.
pub fn estimate(
    msg: &ChatMessage,
    wrap_width: usize,
    display_mode: ChatDisplayMode,
    tools_expanded: bool,
    has_separator: bool,
) -> u16 {
    let sep: u16 = if has_separator { 1 } else { 0 };

    match msg.role {
        ChatRole::System => 0,
        ChatRole::User => sep + USER_OVERHEAD + wrapped_lines(&msg.content, wrap_width),
        ChatRole::Tool => sep + estimate_tool(msg, display_mode, tools_expanded, wrap_width),
        ChatRole::Assistant => {
            sep + estimate_assistant(msg, display_mode, tools_expanded, wrap_width)
        }
    }
}

/// Lineas del cuerpo de texto wrapeadas a `width` columnas.
fn wrapped_lines(text: &str, width: usize) -> u16 {
    if width == 0 {
        return text.lines().count().max(1) as u16;
    }
    text.lines()
        .map(|line| {
            let len = line.len();
            if len == 0 {
                1u16
            } else {
                ((len as f64 / width as f64).ceil() as u16).max(1)
            }
        })
        .sum::<u16>()
        .max(1)
}

fn estimate_assistant(
    msg: &ChatMessage,
    mode: ChatDisplayMode,
    tools_expanded: bool,
    wrap_width: usize,
) -> u16 {
    // Thinking block: max 4 visible lines + 1 blank separator
    let thinking_lines: u16 = msg
        .thinking
        .as_deref()
        .filter(|t| !t.is_empty())
        .map(|t| {
            let total = t.lines().count();
            let visible = total.min(4) as u16;
            let overflow: u16 = if total > 4 { 1 } else { 0 }; // "… (+N)"
            visible + overflow + 1 // +1 blank after block
        })
        .unwrap_or(0);

    // Body lines
    let body_lines: u16 = if let Some(cached) = &msg.cached_lines {
        cached.len() as u16
    } else {
        wrapped_lines(&msg.content, wrap_width)
    };

    let body = if mode == ChatDisplayMode::Brief && body_lines > BRIEF_MAX_LINES as u16 {
        BRIEF_MAX_LINES as u16 + 1 // +1 for the "… [+N lineas]" indicator
    } else {
        body_lines
    };

    // Tool call lines
    let tool_lines = estimate_tool_calls(&msg.tool_calls, mode, tools_expanded);

    ASSISTANT_OVERHEAD + thinking_lines + body + tool_lines
}

/// Estima lineas ocupadas por el bloque de tool calls de un mensaje assistant.
fn estimate_tool_calls(
    tcs: &[crate::state::ToolCall],
    mode: ChatDisplayMode,
    expanded: bool,
) -> u16 {
    if tcs.is_empty() {
        return 0;
    }
    if mode == ChatDisplayMode::Brief {
        return 1; // "↳ N tools (ok, err)"
    }
    if expanded {
        // Cada tool call expandido: ~4 lineas (header + args preview + spacing)
        return tcs.len() as u16 * 4;
    }
    if tcs.len() >= 3 {
        // Collapsed: 1 linea resumen + 1 por cada tool (icon + name)
        return tcs.len() as u16 + 1;
    }
    // Grouped: peor caso 1 linea por tool call
    tcs.len() as u16
}

fn estimate_tool(
    msg: &ChatMessage,
    mode: ChatDisplayMode,
    tools_expanded: bool,
    wrap_width: usize,
) -> u16 {
    if mode == ChatDisplayMode::Brief {
        return 0; // Tool results omitidos en brief
    }
    if tools_expanded {
        // Expanded: result_limits truncan a ~30-80 lineas segun tool
        let content_lines = wrapped_lines(&msg.content, wrap_width);
        content_lines.min(80) // worst-case read_file limit
    } else {
        1 // Collapsed: 1 linea preview
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{ChatMessage, ChatRole};

    #[test]
    fn system_message_has_zero_height() {
        let msg = ChatMessage::new(ChatRole::System, "system prompt".into());
        assert_eq!(estimate(&msg, 80, ChatDisplayMode::Normal, false, false), 0);
    }

    #[test]
    fn user_message_includes_overhead() {
        let msg = ChatMessage::new(ChatRole::User, "hola mundo".into());
        let h = estimate(&msg, 80, ChatDisplayMode::Normal, false, false);
        // 0 overhead + 1 content line
        assert_eq!(h, 1);
    }

    #[test]
    fn user_message_with_separator_adds_one() {
        let msg = ChatMessage::new(ChatRole::User, "hola".into());
        let without = estimate(&msg, 80, ChatDisplayMode::Normal, false, false);
        let with = estimate(&msg, 80, ChatDisplayMode::Normal, false, true);
        assert_eq!(with, without + 1);
    }

    #[test]
    fn tool_message_brief_is_zero() {
        let msg = ChatMessage::new(ChatRole::Tool, "result data".into());
        assert_eq!(estimate(&msg, 80, ChatDisplayMode::Brief, false, false), 0);
    }

    #[test]
    fn assistant_brief_truncates_long_body() {
        let long_content = "linea\n".repeat(50);
        let msg = ChatMessage::new(ChatRole::Assistant, long_content);
        let h = estimate(&msg, 80, ChatDisplayMode::Brief, false, false);
        // ASSISTANT_OVERHEAD(0) + BRIEF_MAX_LINES + 1 indicator
        assert_eq!(h, BRIEF_MAX_LINES as u16 + 1);
    }

    #[test]
    fn wrapping_increases_line_count() {
        // 160 chars at width 80 = 2 visual lines
        let content = "a".repeat(160);
        let msg = ChatMessage::new(ChatRole::User, content);
        let h = estimate(&msg, 80, ChatDisplayMode::Normal, false, false);
        assert_eq!(h, 2); // 0 overhead + 2 wrapped lines
    }
}
