use ratatui::{
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::state::{AppState, ToolMonitorFilter};
use crate::ui::design_system::Pane;
use crate::ui::theme::{
    cyan, dim, green, red, yellow, GLYPH_ERROR, GLYPH_PENDING, GLYPH_SUCCESS, GLYPH_TOOL,
};

pub fn render_tool_monitor(f: &mut Frame, area: Rect, state: &AppState) {
    let filter = &state.tool_monitor_filter;
    let filtered: Vec<_> = state
        .tool_events
        .iter()
        .filter(|e| match filter {
            ToolMonitorFilter::All => true,
            ToolMonitorFilter::Ok => e.is_complete(),
            ToolMonitorFilter::Errors => e.is_error(),
        })
        .collect();

    let count = filtered.len().min(20);
    let overlay_h = (4 + count as u16).min(area.height.saturating_sub(4));
    let overlay_w = area.width.clamp(50, 76);

    let overlay = Rect {
        x: area.x + (area.width.saturating_sub(overlay_w)) / 2,
        y: area.y + 2,
        width: overlay_w,
        height: overlay_h,
    };

    let total = state.tool_events.len();
    let ok_count = state.tool_events.iter().filter(|e| e.is_complete()).count();
    let err_count = state.tool_events.iter().filter(|e| e.is_error()).count();
    let filter_label = filter.label();
    let title = format!(
        "Monitor de Tools ({total} eventos, {ok_count} ok, {err_count} err) [f:{filter_label}]"
    );

    let inner =
        Pane::new().title(&title).border_style(Style::default().fg(cyan())).render(f, overlay);

    if filtered.is_empty() {
        let (msg, hint) = if state.tool_events.is_empty() {
            ("Sin eventos de tools registrados", None)
        } else {
            ("Sin eventos para el filtro actual", Some("f para cambiar"))
        };
        crate::ui::primitives::render_empty_state(f, inner, GLYPH_TOOL, msg, hint);
        return;
    }

    let lines: Vec<Line<'static>> = filtered
        .iter()
        .take(inner.height as usize)
        .map(|e| {
            let (icon, color) = if e.is_complete() {
                (GLYPH_SUCCESS, green())
            } else if e.is_error() {
                (GLYPH_ERROR, red())
            } else {
                (GLYPH_PENDING, yellow())
            };

            let duration = e.duration_ms.map(|ms| format!("{ms}ms")).unwrap_or_default();
            let factory = e.factory.as_deref().unwrap_or("");
            let time = super::extract_time(&e.timestamp);

            crate::ui::primitives::list_row(
                icon,
                color,
                &e.tool,
                crate::ui::primitives::COL_NAME_W,
                vec![
                    Span::styled(format!("{factory:<5}"), Style::default().fg(dim())),
                    Span::styled(format!(" {duration:<6}"), Style::default().fg(dim())),
                    Span::styled(format!(" {time}"), Style::default().fg(dim())),
                ],
            )
        })
        .collect();

    f.render_widget(Paragraph::new(lines), inner);
}
