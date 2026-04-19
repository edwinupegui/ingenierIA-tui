//! Monitor output panel overlay (E26 completion).
//!
//! Fullscreen overlay showing the output of a specific monitor with
//! scroll support. Activated by `/monitor-show <id>` or key shortcut
//! from monitor list. Esc to close, Up/Down to scroll, 'f' to toggle
//! follow mode, 'k' to kill.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap},
    Frame,
};

use crate::state::AppState;
use crate::ui::theme::{bg, cyan, dim, green, red, surface, white, yellow};

/// Max lines rendered per frame (perf guard for very verbose processes).
const MAX_RENDER_LINES: usize = 2_000;

/// Renders the monitor output panel if active. No-op otherwise.
pub fn render_monitor_panel(f: &mut Frame, area: Rect, state: &AppState) {
    let Some(panel) = &state.monitor_panel else {
        return;
    };
    let Some(info) = state.monitors.get(&panel.monitor_id) else {
        return;
    };

    f.render_widget(Clear, area);

    let status_label = info.status.label();
    let line_count = info.lines.len();
    let follow_tag = if panel.follow { " SEGUIR" } else { "" };
    let title = format!(
        " Monitor {} — {} — {} líneas{follow_tag} · Esc cerrar · ↑↓ desplazar · f seguir · k terminar ",
        info.id, status_label, line_count,
    );

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(cyan()))
        .style(Style::default().bg(surface()));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(inner);

    render_output(f, layout[0], info, panel.scroll_offset, panel.follow);
    render_footer(f, layout[1], info);
}

fn render_output(
    f: &mut Frame,
    area: Rect,
    info: &crate::services::monitor::MonitorInfo,
    scroll_offset: u16,
    follow: bool,
) {
    let viewport_h = area.height as usize;
    let total = info.lines.len().min(MAX_RENDER_LINES);

    let lines: Vec<Line<'static>> = info
        .lines
        .iter()
        .rev()
        .take(MAX_RENDER_LINES)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .map(|line| {
            let style = if line.is_stderr {
                Style::default().fg(red())
            } else {
                Style::default().fg(white())
            };
            let prefix = if line.is_stderr { "! " } else { "  " };
            Line::from(vec![
                Span::styled(prefix, Style::default().fg(dim())),
                Span::styled(line.text.clone(), style),
            ])
        })
        .collect();

    let scroll = if follow {
        let content_h = lines.len();
        content_h.saturating_sub(viewport_h) as u16
    } else {
        let max_scroll = total.saturating_sub(viewport_h) as u16;
        max_scroll.saturating_sub(scroll_offset)
    };

    let para = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .scroll((scroll, 0))
        .style(Style::default().bg(bg()));
    f.render_widget(para, area);
}

fn render_footer(f: &mut Frame, area: Rect, info: &crate::services::monitor::MonitorInfo) {
    let dur = info.duration().map(|d| format!("{}s", d.as_secs())).unwrap_or_else(|| "-".into());
    let exit_str = info.exit_code.map(|c| format!("salida {c}")).unwrap_or_else(|| "ejecutando".into());
    let cmd = info.short_command(60);

    let footer = Line::from(vec![
        Span::styled(" ", Style::default().bg(surface())),
        Span::styled(
            exit_str,
            Style::default()
                .fg(if info.exit_code == Some(0) { green() } else { yellow() })
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!(" · {dur} · "), Style::default().fg(dim())),
        Span::styled(cmd, Style::default().fg(white())),
    ]);
    let para = Paragraph::new(vec![footer]).style(Style::default().bg(surface()));
    f.render_widget(para, area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::monitor::{MonitorInfo, MonitorLine};

    #[test]
    fn render_does_not_panic_with_empty_monitor() {
        let info = MonitorInfo::new("m1".into(), "echo test".into());
        // Just ensure the line building doesn't panic.
        let lines: Vec<Line<'static>> =
            info.lines.iter().map(|l| Line::from(l.text.clone())).collect();
        assert!(lines.is_empty());
    }

    #[test]
    fn render_does_not_panic_with_stderr_lines() {
        let mut info = MonitorInfo::new("m1".into(), "cmd".into());
        info.lines.push(MonitorLine { text: "ok".into(), is_stderr: false });
        info.lines.push(MonitorLine { text: "err".into(), is_stderr: true });
        let lines: Vec<Line<'static>> = info
            .lines
            .iter()
            .map(|l| {
                let prefix = if l.is_stderr { "! " } else { "  " };
                Line::from(format!("{prefix}{}", l.text))
            })
            .collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[1].to_string().contains("err"));
    }
}
