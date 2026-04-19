use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::theme::{
    dim, dimmer, lerp_color, red, surface, white, GLYPH_ACCENT_BAR, GLYPH_ERROR,
};

// ── Accent bar ──────────────────────────────────────────────────────────────

/// Render a vertical accent bar using the ▌ glyph. Gradient: full color at bottom, fades to
/// surface at top. `animated` and `tick` are kept for API compatibility but ignored.
pub fn render_accent_bar(
    f: &mut Frame,
    area: Rect,
    color: ratatui::style::Color,
    _animated: bool,
    _tick: u64,
) {
    for row in 0..area.height {
        let t = row as f32 / area.height.max(1) as f32;
        let c = lerp_color(color, surface(), (1.0 - t) * 0.65);
        let cell = Rect { x: area.x, y: area.y + row, width: 1, height: 1 };
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(GLYPH_ACCENT_BAR, Style::default().fg(c)))),
            cell,
        );
    }
}

// ── Input padding ───────────────────────────────────────────────────────────

/// Standard inner rect for input areas.
/// Padding: 2px left (accent bar), 1px top, 1px right, 1px bottom.
pub fn input_inner(area: Rect) -> Rect {
    Rect {
        x: area.x + 2,
        y: area.y,
        width: area.width.saturating_sub(3),
        height: area.height.saturating_sub(1),
    }
}

// ── Hints ───────────────────────────────────────────────────────────────────

/// Standard key+action hint pair. Keys in dim(), actions in dimmer().
pub fn hint_spans(key: &str, action: &str) -> Vec<Span<'static>> {
    vec![
        Span::styled(key.to_string(), Style::default().fg(dim())),
        Span::styled(action.to_string(), Style::default().fg(dimmer())),
    ]
}

// ── Empty states ────────────────────────────────────────────────────────────

/// Render a consistent empty state with icon + message + optional hint.
pub fn render_empty_state(
    f: &mut Frame,
    area: Rect,
    icon: &str,
    message: &str,
    hint: Option<&str>,
) {
    let mut lines = vec![
        Line::from(""),
        Line::from(Span::styled(format!("  {icon}  {message}"), Style::default().fg(dim()))),
    ];
    if let Some(h) = hint {
        lines.push(Line::from(Span::styled(format!("     {h}"), Style::default().fg(dimmer()))));
    }
    f.render_widget(Paragraph::new(lines), area);
}

// ── Error display ───────────────────────────────────────────────────────────

/// Standard error line: GLYPH_ERROR + red() text. Never plain "Error:" text.
pub fn error_line(msg: &str) -> Line<'static> {
    Line::from(error_span(msg))
}

/// Standard error spans: embeddable GLYPH_ERROR + red() inside composite lines.
pub fn error_span(msg: &str) -> Vec<Span<'static>> {
    vec![
        Span::styled(
            format!(" {GLYPH_ERROR} "),
            Style::default().fg(red()).add_modifier(Modifier::BOLD),
        ),
        Span::styled(msg.to_string(), Style::default().fg(red())),
    ]
}

// ── List columns ────────────────────────────────────────────────────────────

/// Standard column widths for tool/agent/enforcement list rows.
pub const COL_NAME_W: usize = 22;
pub const COL_FACTORY_W: usize = 5;
pub const COL_STATUS_W: usize = 8;

/// Build a standard list row: icon + name (bold, padded) + trailing spans.
/// Use in tool_monitor, agent_panel, enforcement for consistent column alignment.
pub fn list_row(
    icon: &str,
    icon_color: Color,
    name: &str,
    name_w: usize,
    trailing: Vec<Span<'static>>,
) -> Line<'static> {
    let mut spans = vec![
        Span::styled(format!(" {icon} "), Style::default().fg(icon_color)),
        Span::styled(
            format!("{:<w$}", name, w = name_w),
            Style::default().fg(white()).add_modifier(Modifier::BOLD),
        ),
    ];
    spans.extend(trailing);
    Line::from(spans)
}

// ── Overlay sizing ──────────────────────────────────────────────────────────

pub const OVERLAY_WIDTH_PCT: u16 = 65;
pub const OVERLAY_MIN_W: u16 = 48;
pub const OVERLAY_MAX_W: u16 = 88;
pub const OVERLAY_MARGIN_Y: u16 = 3;

/// Compute a centered overlay rect with standard sizing.
pub fn overlay_rect(parent: Rect) -> Rect {
    let w = (parent.width * OVERLAY_WIDTH_PCT / 100).clamp(OVERLAY_MIN_W, OVERLAY_MAX_W);
    let h = parent.height.saturating_sub(OVERLAY_MARGIN_Y * 2);
    Rect {
        x: parent.x + (parent.width.saturating_sub(w)) / 2,
        y: parent.y + OVERLAY_MARGIN_Y,
        width: w,
        height: h,
    }
}
