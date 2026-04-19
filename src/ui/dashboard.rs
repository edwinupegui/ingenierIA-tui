use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use super::theme::{
    bg, blue, border, cyan, dim, green, white, yellow, GLYPH_CURSOR, GLYPH_IDLE, GLYPH_PENDING,
};
use super::widgets::{
    self, capitalize, render_model_picker, render_search_overlay, render_sessions_panel, truncate,
};
use crate::state::{AppMode, AppState, ServerStatus, UiFactory};

pub fn render(f: &mut Frame, state: &AppState) {
    let bg = Block::default().style(Style::default().bg(bg()));
    f.render_widget(bg, f.area());

    let hint_pairs: Vec<(&str, &str)> = match state.mode {
        AppMode::Command => vec![("↑↓ ", "navegar"), ("enter ", "ejecutar"), ("esc ", "cancelar")],
        AppMode::Search => vec![("↑↓ ", "navegar"), ("enter ", "abrir"), ("esc ", "cancelar")],
        AppMode::ModelPicker | AppMode::ThemePicker => {
            vec![("↑↓ ", "navegar"), ("enter ", "seleccionar"), ("esc ", "cancelar")]
        }
        #[cfg(feature = "autoskill")]
        AppMode::AutoskillPicker => vec![
            ("↑↓ ", "navegar"),
            ("space ", "toggle"),
            ("enter ", "instalar"),
            ("esc ", "cerrar"),
        ],
        AppMode::Normal => vec![
            ("tab ", "factory"),
            (": ", "paleta"),
            ("/ ", "buscar"),
            ("↑↓ ", "navegar"),
            ("alt+↑↓ ", "desplazar"),
            ("y ", "copiar"),
            ("espacio ", "expandir"),
            ("esc ", "chat"),
        ],
    };
    let hint_h = widgets::hints::hint_rows_needed(&hint_pairs, f.area().width);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Fill(1), Constraint::Length(hint_h)])
        .split(f.area());

    render_header(f, rows[0], state);
    render_content(f, rows[1], state);
    widgets::hints::render_hints_bar(f, rows[2], state, &hint_pairs);

    if state.mode == AppMode::Search {
        render_search_overlay(f, f.area(), state);
    }
    // Command palette is rendered globally in ui/mod.rs
    if state.mode == AppMode::ModelPicker {
        render_model_picker(f, f.area(), state);
    }
    if state.panels.show_sessions {
        render_sessions_panel(f, f.area(), state);
    }
}

// ── Header ────────────────────────────────────────────────────────────────────

fn render_header(f: &mut Frame, area: Rect, state: &AppState) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1)])
        .split(area);

    let (status_color, status_text): (Color, String) = match &state.server_status {
        ServerStatus::Unknown => (dim(), format!("{GLYPH_IDLE} Conectando")),
        ServerStatus::Online(_) => (green(), format!("{GLYPH_PENDING} En línea")),
        ServerStatus::Offline(_) => (super::theme::red(), format!("{GLYPH_PENDING} Sin conexión")),
    };

    let mut left_spans = vec![
        Span::styled(" INGENIERiA ", Style::default().fg(white()).add_modifier(Modifier::BOLD)),
        Span::styled("── ", Style::default().fg(border())),
        factory_tab("Net", state.factory == UiFactory::Net),
        Span::raw(" "),
        factory_tab("Ang", state.factory == UiFactory::Ang),
        Span::raw(" "),
        factory_tab("Nest", state.factory == UiFactory::Nest),
        Span::raw(" "),
        factory_tab("All", state.factory == UiFactory::All),
        Span::styled(" ── ", Style::default().fg(border())),
        Span::styled(status_text, Style::default().fg(status_color)),
    ];
    if let Some(total) = state.server_status.docs_total() {
        left_spans.push(Span::styled(format!("  {total} docs"), Style::default().fg(dim())));
    }
    // Activity indicator for recent tool events
    if !state.tool_events.is_empty() {
        let recent = state.tool_events.iter().filter(|e| e.event_type == "tool:invoke").count();
        if recent > 0 {
            left_spans.push(Span::styled(
                format!("  {GLYPH_PENDING}{recent}"),
                Style::default().fg(yellow()),
            ));
        }
    }

    let identity = if state.developer.is_empty() {
        String::new()
    } else {
        format!(" {} · {} ", state.developer, state.model)
    };
    let identity_len = identity.len() as u16;

    let header_cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Fill(1), Constraint::Length(identity_len.max(1))])
        .split(rows[0]);

    f.render_widget(Paragraph::new(Line::from(left_spans)), header_cols[0]);
    f.render_widget(
        Paragraph::new(Line::from(vec![Span::styled(identity, Style::default().fg(dim()))])),
        header_cols[1],
    );

    let breadcrumb = if let Some(doc) = &state.dashboard.preview.doc {
        Line::from(vec![
            Span::styled(" ", Style::default()),
            Span::styled(
                capitalize(&doc.factory),
                Style::default().fg(blue()).add_modifier(Modifier::BOLD),
            ),
            Span::styled(format!(" {GLYPH_CURSOR} "), Style::default().fg(border())),
            Span::styled(capitalize(&doc.doc_type), Style::default().fg(yellow())),
            Span::styled(format!(" {GLYPH_CURSOR} "), Style::default().fg(border())),
            Span::styled(doc.name.clone(), Style::default().fg(white())),
        ])
    } else {
        Line::from(Span::styled(
            " ─────────────────────────────────────────",
            Style::default().fg(border()),
        ))
    };
    f.render_widget(Paragraph::new(breadcrumb), rows[1]);
}

fn factory_tab(label: &'static str, active: bool) -> Span<'static> {
    if active {
        Span::styled(
            format!("[{label}]"),
            Style::default().fg(bg()).bg(blue()).add_modifier(Modifier::BOLD),
        )
    } else {
        Span::styled(format!("[{label}]"), Style::default().fg(dim()))
    }
}

// ── Content ─────────────────────────────────────────────────────────────────

fn render_content(f: &mut Frame, area: Rect, state: &AppState) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(26), Constraint::Fill(1)])
        .split(area);

    render_sidebar(f, cols[0], state);
    render_preview(f, cols[1], state);
}

// ── Sidebar ─────────────────────────────────────────────────────────────────

fn render_sidebar(f: &mut Frame, area: Rect, state: &AppState) {
    let sidebar_block = Block::default()
        .borders(Borders::RIGHT)
        .border_style(Style::default().fg(border()))
        .style(Style::default().bg(bg()));
    let inner = sidebar_block.inner(area);
    f.render_widget(sidebar_block, area);

    let event_count = state.events.len().min(5);
    let events_height = if event_count > 0 { event_count as u16 + 1 } else { 0 };

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Fill(1), Constraint::Length(events_height)])
        .split(inner);

    render_doc_tree(f, rows[0], state);
    if events_height > 0 {
        render_events_log(f, rows[1], state);
    }
}

fn render_doc_tree(f: &mut Frame, area: Rect, state: &AppState) {
    let sidebar = &state.dashboard.sidebar;

    if sidebar.loading {
        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::raw(" "),
                Span::styled("Cargando...", Style::default().fg(dim())),
            ])),
            area,
        );
        return;
    }

    if let Some(err) = &sidebar.error {
        let msg = super::widgets::truncate(err, 22);
        f.render_widget(Paragraph::new(super::primitives::error_line(&msg)), area);
        return;
    }

    if sidebar.all_docs.is_empty() {
        super::primitives::render_empty_state(
            f,
            area,
            GLYPH_IDLE,
            "Sin documentos",
            Some("Verifica el servidor"),
        );
        return;
    }

    let height = area.height as usize;
    let mut entries: Vec<(String, Option<usize>, bool, bool, bool)> = Vec::new();
    let mut flat = 0usize;

    if sidebar.is_cached {
        entries.push((" ⚠ CACHED (offline)".to_string(), None, false, true, false));
        flat += 1;
    }

    let sync = &state.dashboard.sync;
    for section in &sidebar.sections {
        let badge = sync.badge_for(section.doc_type);
        entries.push((section.header.clone(), badge, flat == sidebar.cursor_pos, true, false));
        flat += 1;

        if section.expanded {
            for item in &section.items {
                let cursor = if flat == sidebar.cursor_pos { GLYPH_CURSOR } else { " " };
                let name = truncate(&item.name, 20);
                let is_priority =
                    state.detected_factory.as_deref().is_some_and(|df| item.factory == df);
                entries.push((
                    format!("  {cursor} {name}"),
                    None,
                    flat == sidebar.cursor_pos,
                    false,
                    is_priority,
                ));
                flat += 1;
            }
        }
    }

    let cursor = sidebar.cursor_pos.min(entries.len().saturating_sub(1));
    let start = cursor.saturating_sub(height / 2).min(entries.len().saturating_sub(height));

    let lines: Vec<Line<'static>> = entries
        .iter()
        .skip(start)
        .take(height)
        .map(|(text, badge, is_cursor, is_section, is_priority)| {
            let style = if *is_cursor {
                Style::default().fg(bg()).bg(blue()).add_modifier(Modifier::BOLD)
            } else if *is_section {
                Style::default().fg(white()).add_modifier(Modifier::BOLD)
            } else if *is_priority {
                Style::default().fg(green())
            } else {
                Style::default().fg(dim())
            };
            let mut spans = vec![Span::styled(text.clone(), style)];
            if let Some(n) = badge {
                let badge_style = if *is_cursor {
                    Style::default().fg(yellow()).bg(blue()).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(yellow()).add_modifier(Modifier::BOLD)
                };
                spans.push(Span::styled(format!(" {GLYPH_PENDING}{n}"), badge_style));
            }
            Line::from(spans)
        })
        .collect();

    f.render_widget(Paragraph::new(lines), area);
}

fn render_events_log(f: &mut Frame, area: Rect, state: &AppState) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Fill(1)])
        .split(area);

    f.render_widget(
        Paragraph::new(Span::styled(
            "─".repeat(area.width as usize),
            Style::default().fg(border()),
        )),
        rows[0],
    );

    let max_rows = rows[1].height as usize;
    let lines: Vec<Line<'static>> = state
        .events
        .iter()
        .take(max_rows)
        .map(|te| {
            let (icon, color) = match te.event.kind_str() {
                "sync" => (GLYPH_PENDING, green()),
                "reload" => (GLYPH_IDLE, blue()),
                "session" => (GLYPH_PENDING, cyan()),
                "conn" => (GLYPH_PENDING, dim()),
                _ => (GLYPH_IDLE, dim()),
            };
            let kind = te.event.kind_str();
            let summary = te.event.summary();
            let summary = truncate(&summary, 12);

            Line::from(vec![
                Span::styled(format!(" {} ", icon), Style::default().fg(color)),
                Span::styled(te.time.clone(), Style::default().fg(dim())),
                Span::raw(" "),
                Span::styled(format!("{kind:<6}"), Style::default().fg(color)),
                Span::raw(" "),
                Span::styled(summary, Style::default().fg(dim())),
            ])
        })
        .collect();

    f.render_widget(Paragraph::new(lines), rows[1]);
}

// ── Preview ─────────────────────────────────────────────────────────────────

fn render_preview(f: &mut Frame, area: Rect, state: &AppState) {
    let preview_block = Block::default().style(Style::default().bg(bg()));
    f.render_widget(preview_block, area);

    let preview = &state.dashboard.preview;

    if preview.loading {
        f.render_widget(
            Paragraph::new(vec![
                Line::from(""),
                Line::from(Span::styled("  Cargando documento...", Style::default().fg(dim()))),
            ]),
            area,
        );
        return;
    }

    let Some(doc) = &preview.doc else {
        render_preview_empty(f, area);
        return;
    };

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Fill(1)])
        .split(area);

    f.render_widget(
        Paragraph::new(vec![
            Line::from(vec![
                Span::raw("  "),
                Span::styled(doc.uri.clone(), Style::default().fg(blue())),
            ]),
            Line::from(Span::styled(
                format!("  {}", "─".repeat(area.width as usize - 4)),
                Style::default().fg(border()),
            )),
        ]),
        rows[0],
    );

    // Use cached rendered lines when available (avoids markdown re-parse per frame)
    let content_lines: Vec<ratatui::text::Line<'_>> = match &preview.cached_lines {
        Some(cached) => cached.iter().cloned().collect(),
        None => {
            super::widgets::markdown::render_markdown(&doc.content, &state.active_theme.colors())
        }
    };
    f.render_widget(Paragraph::new(content_lines).scroll((preview.scroll, 0)), rows[1]);
}

fn render_preview_empty(f: &mut Frame, area: Rect) {
    f.render_widget(
        Paragraph::new(vec![
            Line::from(""),
            Line::from(""),
            Line::from(Span::styled(
                "  Selecciona un documento y presiona Enter",
                Style::default().fg(dim()),
            )),
            Line::from(""),
            Line::from(vec![
                Span::styled("  ↑↓ ", Style::default().fg(white()).add_modifier(Modifier::BOLD)),
                Span::styled("navegar   ", Style::default().fg(dim())),
                Span::styled("Enter ", Style::default().fg(white()).add_modifier(Modifier::BOLD)),
                Span::styled("abrir   ", Style::default().fg(dim())),
                Span::styled("y ", Style::default().fg(white()).add_modifier(Modifier::BOLD)),
                Span::styled("copiar   ", Style::default().fg(dim())),
                Span::styled("Space ", Style::default().fg(white()).add_modifier(Modifier::BOLD)),
                Span::styled("colapsar", Style::default().fg(dim())),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("  / ", Style::default().fg(white()).add_modifier(Modifier::BOLD)),
                Span::styled("buscar documentos   ", Style::default().fg(dim())),
                Span::styled("opt+↑↓ ", Style::default().fg(white()).add_modifier(Modifier::BOLD)),
                Span::styled("desplazar", Style::default().fg(dim())),
            ]),
        ])
        .alignment(Alignment::Left),
        area,
    );
}
