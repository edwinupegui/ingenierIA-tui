use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph},
    Frame,
};

use super::theme::{bg, blue, dim, dimmer, factory_color, green, red, yellow, SPINNERS};
use super::widgets;
use crate::state::{AppState, ChatDisplayMode, ChatMode, ChatStatus};

pub fn render(f: &mut Frame, state: &AppState) {
    let bg = Block::default().style(Style::default().bg(bg()));
    f.render_widget(bg, f.area());
    render_chat_panel(f, f.area(), state);
}

fn render_chat_panel(f: &mut Frame, area: Rect, state: &AppState) {
    let show_sidebar = state.chat.sidebar_visible
        && area.width >= super::sidebar::SIDEBAR_MIN_TERMINAL_WIDTH;

    let hints: Vec<(&str, &str)> =
        if state.chat.mode == ChatMode::PlanReview && state.chat.status == ChatStatus::Ready {
            vec![("a ", "aprobar"), ("e ", "editar"), ("r ", "rechazar"), ("esc ", "panel")]
        } else {
            match state.chat.status {
                ChatStatus::Ready => {
                    vec![
                        ("enter ", "enviar"),
                        ("alt+enter ", "nueva línea"),
                        ("↑/↓ ", "historial"),
                        ("/ ", "buscar"),
                        (": ", "paleta"),
                        ("ctrl+↑↓ ", "turnos"),
                        ("esc ", "panel"),
                    ]
                }
                ChatStatus::Streaming => {
                    vec![("enter ", "encolar"), ("esc ", "abortar/limpiar")]
                }
                ChatStatus::ExecutingTools if !state.chat.pending_approvals.is_empty() => {
                    vec![
                        ("enter ", "aprobar"),
                        ("esc ", "denegar"),
                        ("↑↓ ", "mover"),
                        ("Y ", "aceptar todo"),
                        ("N ", "denegar todo"),
                        ("a ", "siempre"),
                    ]
                }
                ChatStatus::ExecutingTools => vec![("", "ejecutando herramientas...")],
                ChatStatus::LoadingContext => vec![("esc ", "cancelar")],
                ChatStatus::Error(_) => vec![("enter ", "reintentar"), ("esc ", "panel")],
            }
        };
    let hint_h = 1_u16; // barra inferior siempre una línea (estilo compacto)

    let available_width = area.width.saturating_sub(3) as usize;
    let input_lines = if available_width == 0 {
        1
    } else {
        let text_len = if state.chat.input.is_empty() { 22 } else { state.chat.input.len() + 1 };
        ((text_len as f64 / available_width as f64).ceil() as u16).clamp(2, 6)
    };
    let input_height = input_lines + 2; // top padding + text + bottom padding
    let queue_h: u16 = if state.chat.message_queue.has_items() { 1 } else { 0 };

    // rows: [0]=header [1]=messages [2]=gap [3]=status-near-input [4]=input [5]=queue [6]=pad [7]=hints
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(5),
            Constraint::Length(1),       // gap between messages and status
            Constraint::Length(1),       // status bar near input
            Constraint::Length(input_height),
            Constraint::Length(queue_h), // queue footer (conditional)
            Constraint::Length(1),       // padding below input
            Constraint::Length(hint_h),  // hints bar (multi-row)
        ])
        .split(area);

    render_header(f, rows[0], state);

    // Split rows[1] horizontalmente: mensajes a la izq, sidebar a la der.
    // El sidebar solo ocupa el área de mensajes — header/input/hints quedan a ancho completo.
    let (messages_row, sidebar_area) = if show_sidebar {
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Min(60),
                Constraint::Length(super::sidebar::SIDEBAR_WIDTH),
            ])
            .split(rows[1]);
        (cols[0], Some(cols[1]))
    } else {
        (rows[1], None)
    };

    // Reservar strip a la derecha para el message navigator. Solo si hay
    // al menos un user message: sin turnos el nav no aporta y comemos cols.
    let nav_needed = state.chat.messages.iter().any(|m| m.role == crate::state::ChatRole::User);
    let nav_width =
        if nav_needed { widgets::message_nav::width(state.chat.nav_expanded) } else { 0 };
    let (messages_area, nav_area) = if nav_width > 0 && messages_row.width > nav_width + 10 {
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(10), Constraint::Length(nav_width)])
            .split(messages_row);
        (cols[0], Some(cols[1]))
    } else {
        (messages_row, None)
    };

    super::chat_render::render_messages(f, messages_area, state);
    if let Some(sb_area) = sidebar_area {
        super::sidebar::render(f, sb_area, state);
    }
    if let Some(nav_rect) = nav_area {
        widgets::message_nav::render(
            f,
            nav_rect,
            &state.chat.messages,
            state.chat.nav_user_cursor,
            state.chat.nav_expanded,
            state.active_theme.colors(),
        );
    }

    // rows[2] = gap (empty, background only)
    render_status_bar(f, rows[3], state);
    super::chat_render::render_input(f, rows[4], state);

    if queue_h > 0 {
        super::chat_render::render_queue_footer(f, rows[5], &state.chat);
    }

    // Slash command autocomplete popup (rendered above input, anchored a chat panel)
    widgets::slash_autocomplete::render(f, &state.chat.slash_autocomplete, rows[4], area);

    // Mention picker popup (`@` trigger).
    widgets::mention_picker::render(
        f,
        &state.chat.mention_picker,
        rows[4],
        area,
        state.active_theme.colors(),
    );

    // MCP document picker overlay
    widgets::doc_picker::render(f, &state.chat.doc_picker);

    // Si hay cost data, renderizar status line enriquecido en lugar de hints simples
    let cost = &state.chat.cost;
    if cost.total_tokens() > 0 {
        widgets::hints::render_cost_bar(f, rows[7], state, &hints);
    } else {
        widgets::hints::render_hints_bar(f, rows[7], state, &hints);
    }

    // E39: tip contextual si aplica (scope Chat o Any), sobre la barra de hints.
    render_chat_tip(f, rows[7], state);

    // E26: Monitor output panel overlay.
    widgets::render_monitor_panel(f, f.area(), state);

    // E33: Transcript overlay siempre se dibuja al final para quedar por encima.
    widgets::render_transcript_modal(f, f.area(), state);

    // E30b: Modal de historial Ctrl+R por encima del chat (debajo de transcript).
    widgets::history_search::render(
        f,
        f.area(),
        state.chat.history_search.as_ref(),
        &state.chat.input_history,
    );
}

/// Dibuja el tip activo sobre la primera linea de la barra de hints. Solo se
/// muestra si el scope aplica al chat y solo durante los primeros 30s de la
/// sesion (evita distraer durante chats largos).
fn render_chat_tip(f: &mut Frame, area: Rect, state: &AppState) {
    use crate::services::onboarding::TipScope;
    const TIP_VISIBLE_TICKS: u64 = 120; // 30s @ 4Hz
    if state.tick_count > TIP_VISIBLE_TICKS {
        return;
    }
    let Some(tip) = state.current_tip.as_ref() else {
        return;
    };
    if !matches!(tip.scope, TipScope::Chat | TipScope::Any) {
        return;
    }
    // Reutilizamos la primera linea del area de hints.
    let tip_rect = Rect { x: area.x, y: area.y, width: area.width, height: 1 };
    widgets::render_tip(f, tip_rect, tip);
}

/// Barra de estado inmediatamente encima del input, siempre visible.
fn render_status_bar(f: &mut Frame, area: Rect, state: &AppState) {
    use crate::state::AgentMode;
    use super::theme::surface;
    let bar = Block::default().style(Style::default().bg(surface()));
    f.render_widget(bar, area);

    let spinner = pick_spinner_frame(state.tick_count);
    let elapsed = state.chat.stream_elapsed_secs;

    let mut left_spans: Vec<Span> = match &state.chat.status {
        ChatStatus::Streaming => {
            let elapsed_str = if elapsed > 0 { format!("  {elapsed}s") } else { String::new() };
            vec![
                Span::styled(
                    format!(" {spinner} {} Pensando…", super::theme::GLYPH_THINKING),
                    Style::default().fg(blue()).add_modifier(Modifier::ITALIC),
                ),
                Span::styled(elapsed_str, Style::default().fg(dimmer())),
            ]
        }
        ChatStatus::ExecutingTools => {
            let round = state.chat.tool_rounds;
            vec![Span::styled(
                format!(" {spinner} ejecutando tools [{round}]"),
                Style::default().fg(yellow()),
            )]
        }
        ChatStatus::LoadingContext => vec![Span::styled(
            format!(" {spinner} cargando contexto…"),
            Style::default().fg(yellow()),
        )],
        ChatStatus::Error(_) => vec![Span::styled(
            " ✗ error — Enter para reintentar",
            Style::default().fg(red()),
        )],
        ChatStatus::Ready => vec![Span::styled(" ✓ listo", Style::default().fg(dimmer()))],
    };

    // Badge de modo a la derecha: [⚡ AUTO] / [? ASK] / [◈ PLAN]
    let (mode_color, mode_text) = match state.chat.agent_mode {
        AgentMode::Ask => (yellow(), format!(" [{}] ", state.chat.agent_mode.label())),
        AgentMode::Auto => (green(), format!(" [{}] ", state.chat.agent_mode.label())),
        AgentMode::Plan => (blue(), format!(" [{}] ", state.chat.agent_mode.label())),
    };
    let badge_width = mode_text.len() as u16;
    let left_width = area.width.saturating_sub(badge_width);

    let badge_area = Rect { x: area.x + left_width, y: area.y, width: badge_width, height: 1 };
    let left_area = Rect { x: area.x, y: area.y, width: left_width, height: 1 };

    // Hint "Shift+Tab" inline en el badge cuando está listo
    left_spans.push(Span::styled(
        "  ⇧Tab",
        Style::default().fg(dimmer()),
    ));

    f.render_widget(Paragraph::new(Line::from(left_spans)), left_area);
    f.render_widget(
        Paragraph::new(Line::from(vec![Span::styled(
            mode_text,
            Style::default().fg(mode_color).add_modifier(Modifier::BOLD),
        )])),
        badge_area,
    );
}

fn render_header(f: &mut Frame, area: Rect, state: &AppState) {
    let bar = Block::default().style(Style::default().bg(super::theme::bar_bg()));
    f.render_widget(bar, area);

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Fill(1), Constraint::Fill(1)])
        .split(area);

    let factory_color = factory_color(&state.factory);
    let ctx_pct = state.chat.context_percent();
    let ctx_color = if ctx_pct < 60.0 {
        green()
    } else if ctx_pct < 80.0 {
        yellow()
    } else {
        red()
    };
    let ctx_bar = context_bar(ctx_pct);
    let compact_suffix =
        state.chat.last_compaction.map(|s| format!(" ◇{}", s.label())).unwrap_or_default();
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(" Chat — ", Style::default().fg(dim())),
            Span::styled(
                state.factory.label(),
                Style::default().fg(factory_color).add_modifier(Modifier::BOLD),
            ),
            Span::styled(format!("  {ctx_bar} {ctx_pct:.0}%"), Style::default().fg(ctx_color)),
            Span::styled(compact_suffix, Style::default().fg(dim())),
            match state.chat.mode {
                ChatMode::Planning => Span::styled(
                    "  PLAN",
                    Style::default().fg(yellow()).add_modifier(Modifier::BOLD),
                ),
                ChatMode::PlanReview => Span::styled(
                    "  PLAN REVIEW",
                    Style::default().fg(green()).add_modifier(Modifier::BOLD),
                ),
                ChatMode::Normal => Span::raw(""),
            },
            // E33: badge de modo brief visible en el header.
            if state.chat.display_mode == ChatDisplayMode::Brief {
                Span::styled("  BRIEF", Style::default().fg(blue()).add_modifier(Modifier::BOLD))
            } else {
                Span::raw("")
            },
        ])),
        cols[0],
    );

    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(&state.model, Style::default().fg(dimmer())),
            Span::styled(" ", Style::default()),
        ]))
        .alignment(Alignment::Right),
        cols[1],
    );
}

/// Devuelve el frame del spinner a renderizar.
/// Si `INGENIERIA_ACCESSIBILITY` o `INGENIERIA_REDUCE_MOTION` estan activas,
/// devuelve un frame estatico en lugar de la animacion.
fn pick_spinner_frame(tick: u64) -> &'static str {
    if super::a11y::should_reduce_motion() {
        super::a11y::STATIC_SPINNER_FRAME
    } else {
        SPINNERS[(tick as usize) % SPINNERS.len()]
    }
}

/// Barra visual ASCII de 8 slots para mostrar % de contexto (E14).
fn context_bar(pct: f64) -> String {
    const SLOTS: usize = 8;
    let clamped = pct.clamp(0.0, 100.0);
    let filled = ((clamped / 100.0) * SLOTS as f64).round() as usize;
    let filled = filled.min(SLOTS);
    let mut out = String::with_capacity(SLOTS);
    for i in 0..SLOTS {
        out.push(if i < filled { '█' } else { '░' });
    }
    out
}
