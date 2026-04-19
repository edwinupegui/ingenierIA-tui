use ratatui::{
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::services::agents::{AgentRegistry, AgentStatus};
use crate::state::AppState;
use crate::ui::design_system::Pane;
use crate::ui::theme::{
    blue, cyan, dim, green, red, yellow, GLYPH_COLLAPSED, GLYPH_ERROR, GLYPH_IDLE, GLYPH_PENDING,
    GLYPH_SUCCESS,
};

/// Filas reservadas en el overlay para la seccion de subagentes (E22a).
const SUBAGENT_SECTION_ROWS: u16 = 8;

/// Renders a multi-agent visualization panel showing tool call flows
/// grouped as sequential "agent tasks" from tool events.
pub fn render_agent_panel(f: &mut Frame, area: Rect, state: &AppState) {
    let events = &state.tool_events;
    let overlay_h = area.height.saturating_sub(6).min(24);
    let overlay_w = area.width.clamp(54, 80);

    let overlay = Rect {
        x: area.x + (area.width.saturating_sub(overlay_w)) / 2,
        y: area.y + 2,
        width: overlay_w,
        height: overlay_h,
    };

    // Count active (invoke without complete) vs completed
    let invokes: Vec<_> = events.iter().filter(|e| e.event_type == "tool:invoke").collect();
    let completes: Vec<_> = events.iter().filter(|e| e.event_type == "tool:complete").collect();
    let active = invokes.len().saturating_sub(completes.len());
    let total_tasks = invokes.len();

    let title =
        format!("Agent Tasks ({total_tasks} total, {active} active, {} done)", completes.len());

    let inner =
        Pane::new().title(&title).border_style(Style::default().fg(blue())).render(f, overlay);

    // Todos (E12) en la parte superior del panel si la lista no esta vacia.
    let (todo_area, rest_after_todos) = split_todo_area(inner, &state.chat.todos);
    if let Some(area) = todo_area {
        super::todo_panel::render(f, area, &state.chat.todos);
    }
    // Sub-Agents (E22a) debajo de los todos. El active count del worktree
    // manager (E24) se muestra en el header del panel.
    let (sub_area, tools_area) = split_subagent_area(rest_after_todos, &state.agents);
    if let Some(area) = sub_area {
        render_subagents(f, area, &state.agents, state.worktree_manager.active_count());
    }
    let inner = tools_area;

    if events.is_empty() {
        crate::ui::primitives::render_empty_state(
            f,
            inner,
            GLYPH_IDLE,
            "Sin tareas de agentes registradas",
            Some("Las tool calls aparecen como tareas secuenciales"),
        );
        return;
    }

    // Build task tree: group by tool name, show most recent status
    let mut tool_stats: Vec<(&str, usize, usize, usize, Option<u64>)> = Vec::new();

    for invoke in &invokes {
        let name = invoke.tool.as_str();
        if let Some(entry) = tool_stats.iter_mut().find(|(n, _, _, _, _)| *n == name) {
            entry.1 += 1; // total
        } else {
            tool_stats.push((name, 1, 0, 0, None));
        }
    }
    for complete in &completes {
        let name = complete.tool.as_str();
        if let Some(entry) = tool_stats.iter_mut().find(|(n, _, _, _, _)| *n == name) {
            entry.2 += 1; // completed
            if let Some(ms) = complete.duration_ms {
                entry.4 = Some(entry.4.unwrap_or(0) + ms);
            }
        }
    }
    for err in events.iter().filter(|e| e.event_type == "tool:error") {
        let name = err.tool.as_str();
        if let Some(entry) = tool_stats.iter_mut().find(|(n, _, _, _, _)| *n == name) {
            entry.3 += 1; // errors
        }
    }

    let mut lines: Vec<Line<'static>> = Vec::new();

    for (name, total, completed, errors, total_ms) in &tool_stats {
        let pending = total.saturating_sub(*completed).saturating_sub(*errors);
        let (icon, color) = if *errors > 0 {
            (GLYPH_ERROR, red())
        } else if pending > 0 {
            (GLYPH_PENDING, yellow())
        } else {
            (GLYPH_SUCCESS, green())
        };

        let avg_ms = if *completed > 0 { total_ms.unwrap_or(0) / *completed as u64 } else { 0 };

        lines.push(crate::ui::primitives::list_row(
            icon,
            color,
            name,
            crate::ui::primitives::COL_NAME_W,
            vec![
                Span::styled(format!("{completed}/{total}"), Style::default().fg(cyan())),
                if *errors > 0 {
                    Span::styled(format!(" {errors} err"), Style::default().fg(red()))
                } else {
                    Span::raw("")
                },
                Span::styled(format!("  avg {avg_ms}ms"), Style::default().fg(dim())),
            ],
        ));

        // Show recent invocations as sub-items
        let recent: Vec<_> = events.iter().filter(|e| e.tool == *name).take(3).collect();
        for evt in recent {
            let (sub_icon, sub_color) = match evt.event_type.as_str() {
                "tool:complete" => (GLYPH_SUCCESS, green()),
                "tool:error" => (GLYPH_ERROR, red()),
                _ => (GLYPH_PENDING, yellow()),
            };
            let time = super::extract_time(&evt.timestamp);
            let dur = evt.duration_ms.map(|ms| format!("{ms}ms")).unwrap_or_default();
            let factory = evt.factory.as_deref().unwrap_or("");

            lines.push(Line::from(vec![
                Span::styled(format!("   {GLYPH_COLLAPSED} "), Style::default().fg(dim())),
                Span::styled(format!("{sub_icon} "), Style::default().fg(sub_color)),
                Span::styled(format!("{factory:<5}"), Style::default().fg(dim())),
                Span::styled(format!(" {dur:<6}"), Style::default().fg(dim())),
                Span::styled(format!(" {time}"), Style::default().fg(dim())),
            ]));
        }
    }

    // Truncate to fit
    lines.truncate(inner.height as usize);
    f.render_widget(Paragraph::new(lines), inner);
}

/// Divide el area en (todos, resto). Reserva `TODO_SECTION_ROWS` si la lista
/// tiene items y el area alcanza. Retorna `None` cuando no hay items o cuando
/// el area residual quedaria demasiado chica (< 4 filas).
fn split_todo_area(area: Rect, list: &crate::domain::todos::TodoList) -> (Option<Rect>, Rect) {
    use super::todo_panel::TODO_SECTION_ROWS;
    if list.is_empty() {
        return (None, area);
    }
    let want = TODO_SECTION_ROWS.min(area.height.saturating_sub(4));
    if want == 0 {
        return (None, area);
    }
    let todos = Rect { x: area.x, y: area.y, width: area.width, height: want };
    let rest = Rect {
        x: area.x,
        y: area.y + want,
        width: area.width,
        height: area.height.saturating_sub(want),
    };
    (Some(todos), rest)
}

/// Divide el area en (subagentes, tools) — None si no hay subagentes.
fn split_subagent_area(area: Rect, registry: &AgentRegistry) -> (Option<Rect>, Rect) {
    if registry.agents.is_empty() {
        return (None, area);
    }
    let want = SUBAGENT_SECTION_ROWS.min(area.height.saturating_sub(4));
    if want == 0 {
        return (None, area);
    }
    let sub = Rect { x: area.x, y: area.y, width: area.width, height: want };
    let tools = Rect {
        x: area.x,
        y: area.y + want,
        width: area.width,
        height: area.height.saturating_sub(want),
    };
    (Some(sub), tools)
}

/// Render compacto del registry de subagentes (E22a) — header + lista.
/// `worktrees_active` es el conteo del WorktreeManager (E24) — se muestra en
/// el header como ` · 🌿 N` cuando hay worktrees aislados.
fn render_subagents(f: &mut Frame, area: Rect, registry: &AgentRegistry, worktrees_active: usize) {
    let mut header_spans = vec![
        Span::styled("SubAgents ", Style::default().fg(blue())),
        Span::styled(
            format!("(running={}, total={})", registry.active_count(), registry.agents.len()),
            Style::default().fg(dim()),
        ),
    ];
    if worktrees_active > 0 {
        header_spans
            .push(Span::styled(format!(" · 🌿 {worktrees_active}"), Style::default().fg(green())));
    }
    let mut lines: Vec<Line<'static>> = vec![Line::from(header_spans)];
    let max_rows = area.height.saturating_sub(1) as usize;
    for info in registry.recent(max_rows) {
        let (icon, color) = subagent_glyph(&info.status);
        let dur =
            info.duration().map(|d| format!("{}s", d.as_secs())).unwrap_or_else(|| "-".to_string());
        let worktree_mark = if info.worktree.is_some() { "🌿 " } else { "" };
        lines.push(Line::from(vec![
            Span::styled(format!(" {icon} "), Style::default().fg(color)),
            Span::styled(format!("{:<5}", info.id), Style::default().fg(cyan())),
            Span::styled(format!(" {:<12}", info.role), Style::default().fg(blue())),
            Span::styled(format!(" {:<8}", info.status.label()), Style::default().fg(color)),
            Span::styled(format!(" {dur:<5}"), Style::default().fg(dim())),
            Span::styled(
                format!(" {worktree_mark}{}", info.short_prompt(38)),
                Style::default().fg(dim()),
            ),
        ]));
    }
    f.render_widget(Paragraph::new(lines), area);
}

fn subagent_glyph(status: &AgentStatus) -> (&'static str, ratatui::style::Color) {
    match status {
        AgentStatus::Running => (GLYPH_PENDING, yellow()),
        AgentStatus::Done => (GLYPH_SUCCESS, green()),
        AgentStatus::Failed => (GLYPH_ERROR, red()),
        AgentStatus::Cancelled => (GLYPH_COLLAPSED, dim()),
        AgentStatus::Pending => (GLYPH_IDLE, dim()),
    }
}
