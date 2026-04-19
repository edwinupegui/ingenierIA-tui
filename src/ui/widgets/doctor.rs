use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::domain::doctor::{CheckStatus, DoctorReport};
use crate::ui::design_system::Pane;
use crate::ui::theme::{dim, dimmer, green, red, white, yellow};

pub fn render_doctor(f: &mut Frame, area: Rect, report: &DoctorReport) {
    let row_count = report.checks.len();
    let overlay_h = (5 + row_count as u16).min(area.height.saturating_sub(4));
    let overlay_w = area.width.clamp(50, 76);

    let overlay = Rect {
        x: area.x + (area.width.saturating_sub(overlay_w)) / 2,
        y: area.y + 2,
        width: overlay_w,
        height: overlay_h,
    };

    let overall = report.overall();
    let overall_icon = overall.glyph();
    let title = format!("Doctor {overall_icon} — {} checks", row_count);

    let border_color = match overall {
        CheckStatus::Green => green(),
        CheckStatus::Yellow => yellow(),
        CheckStatus::Red => red(),
    };

    let inner = Pane::new()
        .title(&title)
        .border_style(Style::default().fg(border_color))
        .render(f, overlay);

    let mut lines: Vec<Line<'static>> = Vec::with_capacity(row_count + 2);
    for check in &report.checks {
        let (icon, color) = status_style(&check.status);
        let mut spans = vec![
            Span::styled(format!(" {icon} "), Style::default().fg(color)),
            Span::styled(
                format!("{:<16}", check.name),
                Style::default().fg(white()).add_modifier(Modifier::BOLD),
            ),
            Span::styled(check.detail.clone(), Style::default().fg(dim())),
        ];
        if let Some(ref hint) = check.hint {
            spans.push(Span::styled(format!("  ({hint})"), Style::default().fg(dimmer())));
        }
        lines.push(Line::from(spans));
    }

    if lines.len() < inner.height as usize {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled("  Esc para cerrar", Style::default().fg(dimmer()))));
    }

    f.render_widget(Paragraph::new(lines), inner);
}

fn status_style(status: &CheckStatus) -> (&'static str, ratatui::style::Color) {
    match status {
        CheckStatus::Green => ("✓", green()),
        CheckStatus::Yellow => ("⚠", yellow()),
        CheckStatus::Red => ("✗", red()),
    }
}

pub fn render_health_indicator(status: &CheckStatus) -> Span<'static> {
    let (icon, color) = match status {
        CheckStatus::Green => ("✓", green()),
        CheckStatus::Yellow => ("⚠", yellow()),
        CheckStatus::Red => ("✗", red()),
    };
    Span::styled(format!(" {icon}"), Style::default().fg(color))
}
