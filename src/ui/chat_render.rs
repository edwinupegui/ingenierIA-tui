use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Paragraph, Wrap},
    Frame,
};

use super::chat_tools;
use super::msg_height;
use super::theme::{
    accent, bg, blue, cyan, dim, dimmer, green, purple, red, surface, white, yellow,
    GLYPH_ACCENT_BAR, GLYPH_CURSOR_BLOCK, GLYPH_TREE_RESULT, GLYPH_WARNING, SPINNERS,
};
use super::virtual_scroll::{MessageHeight, VirtualWindow};
use super::widgets;
use crate::state::{AppState, ChatDisplayMode, ChatRole, ChatStatus, BRIEF_MAX_LINES};

pub(super) fn render_messages(f: &mut Frame, area: Rect, state: &AppState) {
    let chat = &state.chat;
    let wrap_width = area.width as usize;

    // Build height map for non-System messages (System are always skipped).
    let heights = build_height_map(chat, wrap_width);

    // Cachear offsets acumulados de cada user message (en líneas visuales)
    // para que el handler de keys pueda realinear scroll sin recomputar
    // heights. Se recalcula en cada render — no es costoso porque heights
    // ya se construyó arriba.
    refresh_user_offsets(chat, &heights);

    let window = VirtualWindow::compute(&heights, chat.scroll_offset, area.height);
    chat.last_max_scroll.set(window.total_lines.saturating_sub(area.height));

    // Only render messages within the visible window.
    let mut lines: Vec<Line<'_>> = Vec::new();

    let prev_role = prev_role_before_window(&heights, &chat.messages, &window);
    let mut prev_role = prev_role.as_ref();

    for mh in &heights[window.range.clone()] {
        let msg = &chat.messages[mh.index];
        maybe_push_turn_separator(&mut lines, prev_role, &msg.role);

        match msg.role {
            ChatRole::System => {}
            ChatRole::Tool => {
                let tool_call = resolve_tool_call_for_result(&chat.messages, msg);
                let tool_name = tool_call.map(|t| t.name.as_str()).unwrap_or("tool");
                let tool_args = tool_call.map(|t| t.arguments.as_str()).unwrap_or("");
                render_tool_message(&mut lines, msg, state, tool_name, tool_args);
            }
            ChatRole::User => render_user_message(&mut lines, msg),
            ChatRole::Assistant => render_assistant_message(&mut lines, msg, chat, state),
        }
        prev_role = Some(&msg.role);
    }

    // Status lines siempre al final (streaming heartbeat, tool approval, etc.)
    if window.range.end >= heights.len() {
        render_status_lines(&mut lines, chat, state);
    }

    let paragraph = Paragraph::new(lines)
        .style(Style::default().bg(bg()))
        .wrap(Wrap { trim: false })
        .scroll((window.line_offset, 0));

    f.render_widget(paragraph, area);
}

/// Actualiza la cache `last_user_offsets` con el offset (en líneas visuales)
/// del primer line de cada user message en el timeline completo. Consumido
/// por el handler del navigator para calcular scroll target.
fn refresh_user_offsets(chat: &crate::state::ChatState, heights: &[MessageHeight]) {
    use crate::state::ChatRole;
    let mut offsets: Vec<u16> = Vec::new();
    let mut accum: u32 = 0;
    for mh in heights {
        if matches!(chat.messages[mh.index].role, ChatRole::User) {
            offsets.push(accum.min(u16::MAX as u32) as u16);
        }
        accum = accum.saturating_add(mh.lines as u32);
    }
    chat.last_user_offsets.replace(offsets);
}

/// Construye el mapa de alturas estimadas para cada mensaje no-System.
fn build_height_map(chat: &crate::state::ChatState, wrap_width: usize) -> Vec<MessageHeight> {
    let mode = chat.display_mode;
    let expanded = chat.tools_expanded;
    let mut prev_role: Option<&ChatRole> = None;
    let mut heights = Vec::with_capacity(chat.messages.len());

    for (i, msg) in chat.messages.iter().enumerate() {
        if msg.role == ChatRole::System {
            prev_role = Some(&msg.role);
            continue;
        }
        let has_sep = is_turn_boundary(prev_role, &msg.role);
        let lines = msg_height::estimate(msg, wrap_width, mode, expanded, has_sep);
        if lines > 0 {
            heights.push(MessageHeight { index: i, lines });
        }
        prev_role = Some(&msg.role);
    }
    heights
}

/// Determina el rol del ultimo mensaje visible justo antes de la ventana,
/// para que el primer mensaje del rango pueda decidir si necesita separador.
fn prev_role_before_window(
    heights: &[MessageHeight],
    messages: &[crate::state::ChatMessage],
    window: &VirtualWindow,
) -> Option<ChatRole> {
    if window.range.start == 0 {
        return None;
    }
    heights
        .get(window.range.start.wrapping_sub(1))
        .and_then(|h| messages.get(h.index))
        .map(|m| m.role.clone())
}

/// Retorna `true` cuando entre dos roles consecutivos deberia ir un separador.
fn is_turn_boundary(prev: Option<&ChatRole>, current: &ChatRole) -> bool {
    matches!(
        (prev, current),
        (Some(ChatRole::Assistant), ChatRole::User) | (Some(ChatRole::User), ChatRole::Assistant)
    )
}

fn render_assistant_message<'a>(
    lines: &mut Vec<Line<'a>>,
    msg: &'a crate::state::ChatMessage,
    chat: &'a crate::state::ChatState,
    state: &AppState,
) {
    let is_streaming_msg = chat.status == ChatStatus::Streaming
        && chat.messages.last().is_some_and(|last| std::ptr::eq(msg, last));

    append_thinking_block(lines, msg, is_streaming_msg);
    append_assistant_body(lines, msg, chat.display_mode, is_streaming_msg, state);
    append_tool_calls_summary(lines, msg, state, chat.display_mode);

    if is_streaming_msg {
        if let Some(last) = lines.last_mut() {
            last.spans.push(Span::styled(GLYPH_CURSOR_BLOCK, Style::default().fg(green())));
        }
    }
}

/// Renderiza el bloque de thinking extendido antes del body del asistente.
/// Estilo opencode: `Thinking:` label en dim + contenido en italic/muted.
/// Máx 4 líneas visibles; el resto se indica con `… (+N líneas)`.
fn append_thinking_block<'a>(
    lines: &mut Vec<Line<'a>>,
    msg: &'a crate::state::ChatMessage,
    is_streaming: bool,
) {
    let Some(thinking) = &msg.thinking else { return };
    if thinking.is_empty() {
        return;
    }
    const MAX_VISIBLE: usize = 4;
    let thinking_lines: Vec<&str> = thinking.lines().collect();
    let total = thinking_lines.len();

    lines.push(Line::from(vec![
        Span::styled("Thinking: ", Style::default().fg(dim()).add_modifier(Modifier::ITALIC)),
        Span::styled(
            thinking_lines.first().copied().unwrap_or(""),
            Style::default().fg(dimmer()).add_modifier(Modifier::ITALIC),
        ),
    ]));
    for line in thinking_lines.iter().skip(1).take(MAX_VISIBLE.saturating_sub(1)) {
        lines.push(Line::from(vec![Span::styled(
            format!("  {line}"),
            Style::default().fg(dimmer()).add_modifier(Modifier::ITALIC),
        )]));
    }
    if total > MAX_VISIBLE {
        let hidden = total - MAX_VISIBLE;
        lines.push(Line::from(vec![Span::styled(
            format!("  … (+{hidden} líneas)"),
            Style::default().fg(dimmer()).add_modifier(Modifier::ITALIC),
        )]));
    } else if is_streaming {
        if let Some(last) = lines.last_mut() {
            last.spans.push(Span::styled(GLYPH_CURSOR_BLOCK, Style::default().fg(dimmer())));
        }
    }
    lines.push(Line::from(""));
}

/// Renderiza el cuerpo del asistente (markdown). En modo Brief trunca a
/// `BRIEF_MAX_LINES` y agrega un indicador de lineas ocultas.
fn append_assistant_body<'a>(
    lines: &mut Vec<Line<'a>>,
    msg: &'a crate::state::ChatMessage,
    mode: ChatDisplayMode,
    is_streaming_msg: bool,
    state: &AppState,
) {
    let body: Vec<Line<'a>> = if let Some(cached) = &msg.cached_lines {
        cached.iter().cloned().collect()
    } else if is_streaming_msg {
        widgets::markdown::render_markdown_streaming(&msg.content, &state.active_theme.colors())
    } else {
        widgets::markdown::render_markdown(&msg.content, &state.active_theme.colors())
    };

    if mode == ChatDisplayMode::Brief && body.len() > BRIEF_MAX_LINES {
        let hidden = body.len() - BRIEF_MAX_LINES;
        lines.extend(body.into_iter().take(BRIEF_MAX_LINES));
        lines.push(Line::from(vec![
            Span::styled("    ", Style::default()),
            Span::styled(
                format!("… [+{hidden} líneas ocultas — modo brief activo]"),
                Style::default().fg(dimmer()).add_modifier(Modifier::ITALIC),
            ),
        ]));
    } else {
        lines.extend(body);
    }
}

/// Renderiza tool calls. En modo Brief siempre colapsa a un unico resumen
/// compacto "↳ N tools" sin importar cuantos sean ni el flag tools_expanded.
fn append_tool_calls_summary<'a>(
    lines: &mut Vec<Line<'a>>,
    msg: &'a crate::state::ChatMessage,
    state: &AppState,
    mode: ChatDisplayMode,
) {
    let tcs = &msg.tool_calls;
    if tcs.is_empty() {
        return;
    }
    if mode == ChatDisplayMode::Brief {
        let ok = tcs
            .iter()
            .filter(|tc| matches!(tc.status, crate::state::ToolCallStatus::Success))
            .count();
        let err = tcs
            .iter()
            .filter(|tc| matches!(tc.status, crate::state::ToolCallStatus::Error))
            .count();
        lines.push(Line::from(vec![
            Span::styled("    ↳ ", Style::default().fg(dim())),
            Span::styled(format!("{} herramientas", tcs.len()), Style::default().fg(dim())),
            Span::styled(format!(" ({ok} ok, {err} err)"), Style::default().fg(dimmer())),
        ]));
        return;
    }
    let expanded = state.chat.tools_expanded;
    if expanded {
        for tc in tcs {
            chat_tools::render_tool_call_expanded(lines, tc);
        }
    } else if tcs.len() >= 3 {
        chat_tools::render_tool_calls_collapsed(lines, tcs);
    } else {
        // E32: agrupar tool calls consecutivos del mismo nombre para reducir
        // ruido visual. 2 `read_file` seguidos → "read_file × 2" en una sola
        // linea. Mantiene visibles los individuales cuando son distintos.
        render_tool_calls_grouped(lines, tcs);
    }
}

/// Agrupa tool_calls consecutivos del mismo nombre en un indicador compacto.
fn render_tool_calls_grouped<'a>(lines: &mut Vec<Line<'a>>, tcs: &'a [crate::state::ToolCall]) {
    use crate::state::ToolCallStatus;
    let mut i = 0;
    while i < tcs.len() {
        let name = &tcs[i].name;
        let mut j = i + 1;
        while j < tcs.len() && &tcs[j].name == name {
            j += 1;
        }
        let run_len = j - i;
        if run_len >= 2 {
            // Resumen del grupo.
            let ok = tcs[i..j].iter().filter(|t| t.status == ToolCallStatus::Success).count();
            let err = tcs[i..j].iter().filter(|t| t.status == ToolCallStatus::Error).count();
            let total_ms: u64 = tcs[i..j].iter().filter_map(|t| t.duration_ms).sum();
            let (icon, _) = super::tool_display::tool_icon(name);
            lines.push(Line::from(vec![
                Span::styled("    ", Style::default()),
                Span::styled(format!("{icon} "), Style::default().fg(cyan())),
                Span::styled(
                    format!("{name} × {run_len}"),
                    Style::default().fg(cyan()).add_modifier(Modifier::BOLD),
                ),
                Span::styled(format!("  ({ok} ok"), Style::default().fg(dim())),
                if err > 0 {
                    Span::styled(format!(", {err} err"), Style::default().fg(red()))
                } else {
                    Span::raw("")
                },
                Span::styled(format!(", {total_ms}ms)"), Style::default().fg(dim())),
            ]));
        } else {
            chat_tools::render_tool_call_indicator(lines, &tcs[i]);
        }
        i = j;
    }
}

/// Render del mensaje del usuario — estilo opencode con borde izquierdo `▌`.
fn render_user_message<'a>(lines: &mut Vec<Line<'a>>, msg: &'a crate::state::ChatMessage) {
    let border_color = accent();
    for text_line in msg.content.lines() {
        lines.push(Line::from(vec![
            Span::styled(format!("{GLYPH_ACCENT_BAR} "), Style::default().fg(border_color)),
            Span::styled(text_line, Style::default().fg(white())),
        ]));
    }
}

/// Render de tool results. En modo Brief se omiten por completo (los resume
/// el asistente via `append_tool_calls_summary`).
fn render_tool_message<'a>(
    lines: &mut Vec<Line<'a>>,
    msg: &'a crate::state::ChatMessage,
    state: &AppState,
    tool_name: &str,
    tool_args: &str,
) {
    if state.chat.display_mode == ChatDisplayMode::Brief {
        return;
    }
    if state.chat.tools_expanded {
        chat_tools::render_tool_result_expanded(lines, &msg.content, tool_name, tool_args);
    } else {
        let preview = chat_tools::tool_result_preview(&msg.content, 80);
        lines.push(Line::from(vec![
            Span::styled("    ", Style::default()),
            Span::styled(format!("{GLYPH_TREE_RESULT} "), Style::default().fg(dim())),
            Span::styled(preview, Style::default().fg(dim())),
        ]));
    }
}

/// Devuelve el `ToolCall` con matching `tool_call_id` para un mensaje de
/// `ChatRole::Tool`. Recorre hacia atras hasta encontrarlo (O(n), < 10 msgs
/// tipicamente). E41 usa los args para renderizar diff visual en Edit/Write.
fn resolve_tool_call_for_result<'a>(
    messages: &'a [crate::state::ChatMessage],
    result: &'a crate::state::ChatMessage,
) -> Option<&'a crate::state::ToolCall> {
    let target_id = result.tool_call_id.as_deref()?;
    for msg in messages.iter().rev() {
        for tc in &msg.tool_calls {
            if tc.id == target_id {
                return Some(tc);
            }
        }
    }
    None
}

/// Inserta líneas en blanco entre turnos User↔Assistant para separación visual
/// sin separadores horizontales (estilo opencode).
fn maybe_push_turn_separator<'a>(
    lines: &mut Vec<Line<'a>>,
    prev: Option<&ChatRole>,
    current: &ChatRole,
) {
    let Some(prev) = prev else { return };
    let boundary = matches!(
        (prev, current),
        (ChatRole::Assistant, ChatRole::User) | (ChatRole::User, ChatRole::Assistant)
    );
    if !boundary {
        return;
    }
    lines.push(Line::from(""));
}

fn render_status_lines<'a>(
    lines: &mut Vec<Line<'a>>,
    chat: &'a crate::state::ChatState,
    state: &AppState,
) {
    if chat.status == ChatStatus::LoadingContext {
        let dots = ".".repeat((state.tick_count as usize % 3) + 1);
        lines.push(Line::from(""));
        lines.push(Line::from(vec![Span::styled(
            format!("  Cargando contexto de ingenierIA{dots}"),
            Style::default().fg(yellow()),
        )]));
    }

    if chat.status == ChatStatus::Streaming {
        render_streaming_heartbeat(lines, chat, state);
    }

    if chat.status == ChatStatus::ExecutingTools {
        render_executing_tools_status(lines, chat, state);
    }

}

fn render_streaming_heartbeat<'a>(
    lines: &mut Vec<Line<'a>>,
    chat: &'a crate::state::ChatState,
    state: &AppState,
) {
    let secs = chat.stream_elapsed_secs;
    if secs == 0 {
        return;
    }
    let spinner = SPINNERS[(state.tick_count as usize / 2) % SPINNERS.len()];
    let color = if chat.stream_stalled { red() } else { dimmer() };
    let label = if chat.stream_stalled {
        format!("    {spinner} Respuesta lenta ({secs}s)")
    } else {
        format!("    {spinner} Pensando ({secs}s)")
    };
    lines.push(Line::from(vec![Span::styled(label, Style::default().fg(color))]));
}

fn render_executing_tools_status<'a>(
    lines: &mut Vec<Line<'a>>,
    chat: &'a crate::state::ChatState,
    state: &AppState,
) {
    if !chat.pending_approvals.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(vec![Span::styled(
            format!("  {GLYPH_WARNING} Tools requieren aprobación:"),
            Style::default().fg(yellow()).add_modifier(Modifier::BOLD),
        )]));
        for approval in &chat.pending_approvals {
            let args_preview = chat_tools::tool_args_preview(&approval.arguments, 40);
            lines.push(Line::from(vec![
                Span::styled("    ", Style::default()),
                Span::styled(approval.permission_label.as_str(), Style::default().fg(red())),
                Span::styled(
                    approval.tool_name.as_str(),
                    Style::default().fg(yellow()).add_modifier(Modifier::BOLD),
                ),
                Span::styled(format!(" {args_preview}"), Style::default().fg(dim())),
            ]));
        }
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled("y", Style::default().fg(green()).add_modifier(Modifier::BOLD)),
            Span::styled(" aprobar  ", Style::default().fg(dim())),
            Span::styled("n", Style::default().fg(red()).add_modifier(Modifier::BOLD)),
            Span::styled(" denegar  ", Style::default().fg(dim())),
            Span::styled("a", Style::default().fg(blue()).add_modifier(Modifier::BOLD)),
            Span::styled(" siempre permitir  ", Style::default().fg(dim())),
            Span::styled("d", Style::default().fg(purple()).add_modifier(Modifier::BOLD)),
            Span::styled(" siempre denegar", Style::default().fg(dim())),
        ]));
    } else {
        let spinner = SPINNERS[(state.tick_count as usize / 2) % SPINNERS.len()];
        lines.push(Line::from(vec![Span::styled(
            format!("    {spinner} Ejecutando tools..."),
            Style::default().fg(yellow()),
        )]));
    }
}

pub(super) fn render_input(f: &mut Frame, area: Rect, state: &AppState) {
    let chat = &state.chat;

    let input_bg = ratatui::widgets::Block::default().style(Style::default().bg(surface()));
    f.render_widget(input_bg, area);

    let inner = super::primitives::input_inner(area);

    let accent_color = input_accent_color(state);
    super::primitives::render_accent_bar(f, area, accent_color, true, state.tick_count);

    let can_type = chat.status == ChatStatus::Ready;
    let input_text = build_input_text(chat, can_type, accent_color);

    f.render_widget(Paragraph::new(input_text).wrap(Wrap { trim: false }), inner);

    if can_type && !chat.input.is_empty() {
        render_char_indicator(f, area, chat);
    }
}

fn input_accent_color(state: &AppState) -> ratatui::style::Color {
    match state.chat.status {
        ChatStatus::Ready => green(),
        ChatStatus::Streaming | ChatStatus::LoadingContext | ChatStatus::ExecutingTools => yellow(),
        ChatStatus::Error(_) => red(),
    }
}

fn build_input_text<'a>(
    chat: &'a crate::state::ChatState,
    can_type: bool,
    accent_color: ratatui::style::Color,
) -> Text<'a> {
    let draft = chat.message_queue.draft();
    if !draft.is_empty() {
        let mut spans = vec![
            Span::styled("▸ ", Style::default().fg(yellow())),
            Span::styled(draft, Style::default().fg(white())),
            Span::styled(GLYPH_CURSOR_BLOCK, Style::default().fg(yellow())),
        ];
        let queue_count = chat.message_queue.len();
        if queue_count > 0 {
            spans.push(Span::styled(
                format!("  [{queue_count} en cola]"),
                Style::default().fg(dim()),
            ));
        }
        return Text::from(Line::from(spans));
    }

    if chat.input.is_empty() && can_type {
        let spans = if let Some(ref skill) = chat.selected_skill {
            let hint = format!("/{} — escribe tu peticion...", skill.name);
            vec![
                Span::styled(hint, Style::default().fg(dim())),
                Span::styled(GLYPH_CURSOR_BLOCK, Style::default().fg(accent_color)),
            ]
        } else {
            vec![
                Span::styled("Escribe tu mensaje...", Style::default().fg(dim())),
                Span::styled(GLYPH_CURSOR_BLOCK, Style::default().fg(accent_color)),
            ]
        };
        return Text::from(Line::from(spans));
    }

    if !can_type {
        return input_as_multiline_text(&chat.input, Style::default().fg(dim()), None);
    }

    build_active_input_text(chat, accent_color)
}

/// Splits `text` on `\n` into multiple `Line`s. Appends `cursor` span to the last line.
fn input_as_multiline_text<'a>(
    text: &'a str,
    style: Style,
    cursor: Option<Span<'a>>,
) -> Text<'a> {
    let parts: Vec<&str> = text.split('\n').collect();
    let n = parts.len();
    let lines: Vec<Line<'a>> = parts
        .into_iter()
        .enumerate()
        .map(|(i, part)| {
            if i == n - 1 {
                let mut spans = vec![Span::styled(part, style)];
                if let Some(ref c) = cursor {
                    spans.push(c.clone());
                }
                Line::from(spans)
            } else {
                Line::from(Span::styled(part, style))
            }
        })
        .collect();
    Text::from(lines)
}

fn build_active_input_text<'a>(
    chat: &'a crate::state::ChatState,
    accent_color: ratatui::style::Color,
) -> Text<'a> {
    let cursor = Span::styled(GLYPH_CURSOR_BLOCK, Style::default().fg(accent_color));
    let has_skill = chat.selected_skill.is_some();

    if has_skill {
        // Skill prefix (e.g. "/skillname ") stays on first visual line with cyan style.
        let (prefix, body) = if let Some(idx) = chat.input.find(' ') {
            (&chat.input[..idx], &chat.input[idx..])
        } else {
            (chat.input.as_str(), "")
        };
        let parts: Vec<&str> = body.split('\n').collect();
        let n = parts.len();
        let mut lines: Vec<Line<'a>> = Vec::with_capacity(n);
        for (i, part) in parts.into_iter().enumerate() {
            if i == 0 {
                let mut spans = vec![
                    Span::styled(
                        prefix.to_string(),
                        Style::default().fg(cyan()).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(part.to_string(), Style::default().fg(white())),
                ];
                if n == 1 {
                    spans.push(cursor.clone());
                }
                lines.push(Line::from(spans));
            } else if i == n - 1 {
                lines.push(Line::from(vec![
                    Span::styled(part.to_string(), Style::default().fg(white())),
                    cursor.clone(),
                ]));
            } else {
                lines.push(Line::from(Span::styled(
                    part.to_string(),
                    Style::default().fg(white()),
                )));
            }
        }
        Text::from(lines)
    } else {
        input_as_multiline_text(
            &chat.input,
            Style::default().fg(white()),
            Some(cursor),
        )
    }
}

fn render_char_indicator(f: &mut Frame, area: Rect, chat: &crate::state::ChatState) {
    let line_count = chat.input.lines().count();
    let char_count = chat.input.len();
    let indicator = if line_count > 1 {
        format!("[{line_count} líneas] {char_count}")
    } else {
        format!("{char_count}")
    };
    let ind_w = indicator.len() as u16 + 1;
    let ind_area = Rect {
        x: area.x + area.width.saturating_sub(ind_w + 1),
        y: area.y,
        width: ind_w,
        height: 1,
    };
    f.render_widget(
        Paragraph::new(Span::styled(indicator, Style::default().fg(dimmer()))),
        ind_area,
    );
}

// ── Queue footer ────────────────────────────────────────────────────────────

pub(super) fn render_queue_footer(f: &mut Frame, area: Rect, chat: &crate::state::ChatState) {
    let count = chat.message_queue.len();
    if count == 0 || area.height == 0 {
        return;
    }
    let label = format!("  {count} mensaje(s) en cola (esc limpiar)");
    let p = Paragraph::new(Span::styled(label, Style::default().fg(yellow()).bg(surface())));
    f.render_widget(p, area);
}
