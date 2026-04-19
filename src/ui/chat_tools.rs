use ingenieria_ui::design_system::diff_bars;
use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};

use super::theme::{dim, dimmer, green, red, yellow, GLYPH_TOOL_BLOCK, GLYPH_TOOL_PENDING};
use crate::state::{ToolCall, ToolCallStatus};

pub(super) fn render_tool_call_indicator(lines: &mut Vec<Line<'_>>, tc: &ToolCall) {
    let (block_icon, color) = match tc.status {
        ToolCallStatus::Pending => (GLYPH_TOOL_PENDING, yellow()),
        ToolCallStatus::Success => (GLYPH_TOOL_BLOCK, green()),
        ToolCallStatus::Error => (GLYPH_TOOL_BLOCK, red()),
    };

    let duration = tc
        .duration_ms
        .map(|ms| {
            if ms >= 1000 {
                format!(" · {:.1}s", ms as f64 / 1000.0)
            } else {
                format!(" · {ms}ms")
            }
        })
        .unwrap_or_default();

    let mut spans: Vec<Span<'_>> = vec![
        Span::styled("  ", Style::default()),
        Span::styled(format!("{block_icon} "), Style::default().fg(color)),
        Span::styled(tc.name.clone(), Style::default().fg(color).add_modifier(Modifier::BOLD)),
    ];

    if let Some(subtitle) = tool_subtitle(&tc.name, &tc.arguments) {
        spans.push(Span::styled(format!(" {subtitle}"), Style::default().fg(dim())));
    }

    if let Some((added, removed)) = edit_diff_counts(&tc.name, &tc.arguments) {
        if added + removed > 0 {
            spans.push(Span::styled("  ", Style::default()));
            spans.extend(diff_bars::diff_bar_spans(added as u32, removed as u32, green(), red()));
            spans
                .push(Span::styled(format!(" +{added} -{removed}"), Style::default().fg(dimmer())));
        }
    }

    spans.push(Span::styled(duration, Style::default().fg(dimmer())));
    lines.push(Line::from(spans));
}

/// Subtítulo corto para el header colapsado: path si el tool trabaja sobre
/// archivos, comando si es Bash/shell, query si es Grep/Glob. `None` fuerza
/// el fallback al preview genérico de args.
fn tool_subtitle(name: &str, args_json: &str) -> Option<String> {
    let normalized = name.rsplit(':').next().unwrap_or(name).to_ascii_lowercase();
    let value: serde_json::Value = serde_json::from_str(args_json).ok()?;
    let obj = value.as_object()?;
    let pick = |keys: &[&str]| -> Option<String> {
        keys.iter().find_map(|k| obj.get(*k).and_then(|v| v.as_str()).map(|s| s.to_string()))
    };
    let raw = match normalized.as_str() {
        "read" | "read_file" | "write" | "write_file" | "create" | "edit" | "edit_file"
        | "patch" => pick(&["path", "file_path", "filepath", "file"]),
        "bash" | "shell" | "run" | "run_shell_command" => pick(&["command", "cmd"]),
        "grep" | "search" | "glob" => pick(&["pattern", "query", "regex"]),
        _ => return None,
    }?;
    Some(truncate_str(&raw, 48))
}

/// Cuando el tool es Edit/Write y los args contienen old_string/new_string,
/// calcula (added, removed) sin renderizar. `None` si no aplica o JSON inválido.
fn edit_diff_counts(name: &str, args_json: &str) -> Option<(usize, usize)> {
    let normalized = name.rsplit(':').next().unwrap_or(name).to_ascii_lowercase();
    if !matches!(
        normalized.as_str(),
        "edit" | "edit_file" | "patch" | "write" | "write_file" | "create"
    ) {
        return None;
    }
    let value: serde_json::Value = serde_json::from_str(args_json).ok()?;
    let obj = value.as_object()?;
    let old = obj.get("old_string").and_then(|v| v.as_str()).unwrap_or("");
    let new =
        obj.get("new_string").or_else(|| obj.get("content")).and_then(|v| v.as_str()).unwrap_or("");
    if old.is_empty() && new.is_empty() {
        return None;
    }
    Some(super::diff_render::diff_stats(old, new))
}

/// Render a tool call with full arguments (expanded mode).
pub(super) fn render_tool_call_expanded(lines: &mut Vec<Line<'_>>, tc: &ToolCall) {
    let (icon, color) = match tc.status {
        ToolCallStatus::Pending => (super::theme::GLYPH_PENDING, yellow()),
        ToolCallStatus::Success => (super::theme::GLYPH_SUCCESS, green()),
        ToolCallStatus::Error => (super::theme::GLYPH_ERROR, red()),
    };
    let duration = tc.duration_ms.map(|ms| format!(" {ms}ms")).unwrap_or_default();
    lines.push(Line::from(vec![
        Span::styled("    ", Style::default()),
        Span::styled(format!("{icon} "), Style::default().fg(color)),
        Span::styled(tc.name.clone(), Style::default().fg(color).add_modifier(Modifier::BOLD)),
        Span::styled(duration, Style::default().fg(dim())),
    ]));
    // Pretty-print JSON arguments
    let pretty = serde_json::from_str::<serde_json::Value>(&tc.arguments)
        .ok()
        .and_then(|v| serde_json::to_string_pretty(&v).ok())
        .unwrap_or_else(|| tc.arguments.clone());
    for arg_line in pretty.lines() {
        lines.push(Line::from(vec![
            Span::styled("      ", Style::default()),
            Span::styled(arg_line.to_string(), Style::default().fg(dim())),
        ]));
    }
}

/// Render a tool result expanded con limites por-tool (E32) + diff visual
/// para Edit/Write (E41).
///
/// `tool_name` se usa para resolver los limites via `tool_display::result_limits`:
/// - Read: 80 lineas / 6KB
/// - Bash: 60 lineas / 4KB
/// - Glob/Grep/ls: 40 lineas / 3KB
/// - Write/Edit: 20 lineas / 2KB
/// - Default: 30 lineas / 2KB
///
/// `tool_args` (JSON) se inspecciona solo para Edit/Write: si contiene
/// old_string/new_string se renderiza un diff inline antes del contenido.
pub(super) fn render_tool_result_expanded(
    lines: &mut Vec<Line<'_>>,
    content: &str,
    tool_name: &str,
    tool_args: &str,
) {
    maybe_render_edit_diff(lines, tool_name, tool_args);
    let (max_lines, max_chars) = super::tool_display::result_limits(tool_name);
    let content_lines: Vec<&str> = content.lines().collect();
    let total_lines = content_lines.len();
    let mut shown_chars: usize = 0;
    let mut shown_lines: usize = 0;
    for line in content_lines.iter().take(max_lines) {
        if shown_chars + line.len() > max_chars {
            break;
        }
        lines.push(Line::from(vec![
            Span::styled("      ", Style::default()),
            Span::styled((*line).to_string(), Style::default().fg(dim())),
        ]));
        shown_chars += line.len() + 1;
        shown_lines += 1;
    }
    if shown_lines < total_lines {
        let remaining = total_lines - shown_lines;
        lines.push(Line::from(vec![
            Span::styled("      ", Style::default()),
            Span::styled(
                format!(
                    "… ({remaining} lineas mas, presiona 't' para ver/colapsar • limite {tool_name}: {max_lines}L/{max_chars}ch)"
                ),
                Style::default().fg(dimmer()),
            ),
        ]));
    }
}

/// Renderiza un diff inline cuando el tool es Edit/Write y los args tienen
/// old_string/new_string parseables. Silencioso en cualquier otro caso.
fn maybe_render_edit_diff(lines: &mut Vec<Line<'_>>, tool_name: &str, tool_args: &str) {
    let normalized = tool_name.rsplit(':').next().unwrap_or(tool_name);
    if !matches!(normalized, "edit" | "edit_file" | "patch" | "write" | "write_file" | "create") {
        return;
    }
    let value: serde_json::Value = match serde_json::from_str(tool_args) {
        Ok(v) => v,
        Err(_) => return,
    };
    let obj = match value.as_object() {
        Some(o) => o,
        None => return,
    };
    let old = obj.get("old_string").and_then(|v| v.as_str()).unwrap_or("");
    let new =
        obj.get("new_string").or_else(|| obj.get("content")).and_then(|v| v.as_str()).unwrap_or("");
    if old.is_empty() && new.is_empty() {
        return;
    }
    let (added, removed) = super::diff_render::diff_stats(old, new);
    lines.push(Line::from(vec![
        Span::styled("      ", Style::default()),
        Span::styled("diff ", Style::default().fg(dim())),
        Span::styled(
            format!("+{added} -{removed}"),
            Style::default().fg(dim()).add_modifier(ratatui::style::Modifier::BOLD),
        ),
    ]));
    let diff_lines = super::diff_render::render_diff_lines(
        old,
        new,
        super::diff_render::DiffRenderOpts {
            max_lines: 16,
            indent: "      ",
            context: 1,
            file_path: None,
        },
    );
    lines.extend(diff_lines);
}

/// Render collapsed summary for 3+ tool calls.
pub(super) fn render_tool_calls_collapsed(lines: &mut Vec<Line<'_>>, tcs: &[ToolCall]) {
    let ok = tcs.iter().filter(|t| t.status == ToolCallStatus::Success).count();
    let pending = tcs.iter().filter(|t| t.status == ToolCallStatus::Pending).count();
    let err = tcs.iter().filter(|t| t.status == ToolCallStatus::Error).count();
    let mut parts = Vec::new();
    if ok > 0 {
        parts.push(format!("{ok} {}", super::theme::GLYPH_SUCCESS));
    }
    if pending > 0 {
        parts.push(format!("{pending} pending"));
    }
    if err > 0 {
        parts.push(format!("{err} {}", super::theme::GLYPH_ERROR));
    }
    lines.push(Line::from(vec![
        Span::styled("  ", Style::default()),
        Span::styled(
            format!("■ {} herramientas ({})", tcs.len(), parts.join(", ")),
            Style::default().fg(dim()),
        ),
    ]));
}

/// Extract a short preview of tool arguments (JSON -> key highlights).
pub(super) fn tool_args_preview(args_json: &str, max_len: usize) -> String {
    if let Ok(val) = serde_json::from_str::<serde_json::Value>(args_json) {
        if let Some(obj) = val.as_object() {
            let parts: Vec<String> = obj
                .iter()
                .take(3)
                .map(|(k, v)| {
                    let v_str = match v {
                        serde_json::Value::String(s) => truncate_str(s, 20),
                        other => truncate_str(&other.to_string(), 20),
                    };
                    format!("{k}={v_str}")
                })
                .collect();
            let joined = parts.join(" ");
            return truncate_str(&joined, max_len);
        }
    }
    truncate_str(args_json, max_len)
}

/// Extract first meaningful line of tool result as preview.
pub(super) fn tool_result_preview(content: &str, max_len: usize) -> String {
    let first_line = content.lines().find(|l| !l.trim().is_empty()).unwrap_or("(vacio)");
    truncate_str(first_line.trim(), max_len)
}

fn truncate_str(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        let truncate_at = max.saturating_sub(3);
        let end =
            s.char_indices().map(|(i, _)| i).take_while(|&i| i <= truncate_at).last().unwrap_or(0);
        format!("{}...", &s[..end])
    }
}
