// ── Re-exports from ingenieria-ui crate (E28 Phase 2a) ───────────────────────
pub use ingenieria_ui::a11y;
pub use ingenieria_ui::buffer_diff;
pub use ingenieria_ui::design_system;
pub use ingenieria_ui::frame_throttle;
pub use ingenieria_ui::primitives;
#[allow(unused_imports)]
pub use ingenieria_ui::style_pool;
pub use ingenieria_ui::theme;

// ── Local modules (not yet extracted) ──────────────────────────────────────
mod chat;
mod chat_render;
mod chat_tools;
mod dashboard;
pub mod diff_render;
pub mod diff_syntax;
mod diff_word;
pub mod hyperlinks;
mod init;
#[expect(
    dead_code,
    reason = "E29 spec — consumed when chat.rs adopts MarkdownStreamState for incremental render"
)]
pub mod markdown;
mod msg_height;
pub(super) mod sidebar;
pub mod tool_display;
pub mod virtual_scroll;
pub mod widgets;
mod wizard;
mod wizard_auth;
mod wizard_model;

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph},
    Frame,
};
use tui_big_text::{BigText, PixelSize};

use theme::{
    dim, factory_color, green, red, GLYPH_CURSOR_BLOCK, GLYPH_HEART, GLYPH_IDLE, GLYPH_PENDING,
};

use crate::state::{AppMode, AppScreen, AppState, ServerStatus};

pub fn render(f: &mut Frame, state: &AppState) {
    // Set active theme for the frame so that `theme::bg()`, `theme::blue()`,
    // etc. resolve to the user's currently selected theme. All widgets below
    // read from this thread-local.
    theme::set_active_theme(state.active_theme.colors());
    match state.screen {
        AppScreen::Wizard => wizard::render(f, state),
        AppScreen::Splash => render_splash(f, state),
        AppScreen::Dashboard => dashboard::render(f, state),
        AppScreen::Init => init::render(f, state),
        AppScreen::Chat => chat::render(f, state),
    }
    // Command palette overlay — global, works from any screen
    if state.mode == AppMode::Command {
        widgets::render_command_palette(f, f.area(), state);
    }
    // Theme picker overlay — global
    if state.mode == AppMode::ThemePicker {
        widgets::render_theme_picker(f, f.area(), state);
    }
    // Autoskill picker overlay — global (feature-gated)
    #[cfg(feature = "autoskill")]
    if state.mode == AppMode::AutoskillPicker {
        widgets::render_autoskill_picker(f, f.area(), state);
    }
    // Tool monitor overlay
    if state.panels.show_tool_monitor {
        widgets::render_tool_monitor(f, f.area(), state);
    }
    // Enforcement dashboard overlay
    if state.panels.show_enforcement {
        widgets::render_enforcement(f, f.area(), state);
    }
    // Agent panel overlay
    if state.panels.show_agents {
        widgets::render_agent_panel(f, f.area(), state);
    }
    // Cost detail panel overlay
    if state.panels.show_cost_panel {
        widgets::render_cost_panel(f, f.area(), state);
    }
    // Notification center overlay
    if state.panels.show_notifications {
        widgets::render_notifications(f, f.area(), state);
    }
    // Doctor overlay
    if state.panels.show_doctor {
        if let Some(ref report) = state.doctor_report {
            widgets::render_doctor(f, f.area(), report);
        }
    }
    // Permission modal (above other overlays)
    if state.screen == AppScreen::Chat && !state.chat.pending_approvals.is_empty() {
        widgets::render_permission_modal(f, f.area(), state);
    }
    // Elicitation modal (E18): por encima del permission modal pero debajo de toasts
    if state.chat.pending_elicitation.is_some() {
        widgets::render_elicitation_modal(f, f.area(), state);
    }
    // Toasts overlay — se renderiza sobre cualquier pantalla
    if !state.toasts.is_empty() {
        widgets::render_toasts(f, f.area(), state);
    }
}

// ── Splash ────────────────────────────────────────────────────────────────────

fn render_splash(f: &mut Frame, state: &AppState) {
    let colors = state.active_theme.colors();
    let bg = Block::default().style(Style::default().bg(colors.bg));
    f.render_widget(bg, f.area());

    let show_checklist = state.onboarding.checklist.should_display();
    let checklist_h: u16 = if show_checklist { widgets::CHECKLIST_HEIGHT } else { 0 };
    let tip_h: u16 = if state.current_tip.is_some() && !show_checklist { 1 } else { 0 };

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Fill(1),
            Constraint::Length(4), // ingenierIA big text
            Constraint::Length(1), // "hecho con ❤ por EdwinDev"
            Constraint::Length(1), // tagline
            Constraint::Length(2),
            Constraint::Length(3), // input card (only input text, no pills)
            Constraint::Length(1), // status pills row (factory + model), below card
            Constraint::Length(1), // spacer
            Constraint::Length(1), // keyboard hints
            Constraint::Length(checklist_h), // onboarding checklist (E39, conditional)
            Constraint::Fill(1),
            Constraint::Length(tip_h), // tip card — anchored above status bar
            Constraint::Length(1),     // status bar
        ])
        .split(f.area());

    render_title(f, rows[1], state);
    render_made_by(f, rows[2]);
    render_tagline(f, rows[3], state);
    let input_rect = render_input_area(f, rows[5], state, &colors);
    render_status_pills_row(f, rows[6], state, &colors);
    render_keyboard_hints(f, rows[8], state);
    if checklist_h > 0 {
        let checklist_rect = centered_rect(rows[9], 78);
        widgets::render_checklist(f, checklist_rect, &state.onboarding.checklist);
    }
    if tip_h > 0 {
        if let Some(tip) = state.current_tip.as_ref() {
            widgets::render_tip(f, rows[11], tip);
        }
    }
    // Slash autocomplete popup (above the input area, constrained to input width)
    widgets::slash_autocomplete::render(f, &state.splash_autocomplete, input_rect, input_rect);
    render_status_bar(f, rows[12], state);
}

/// Renderiza factory + model pills en una fila dedicada debajo del card del
/// input, centradas respecto al ancho del card.
fn render_status_pills_row(
    f: &mut Frame,
    area: Rect,
    state: &AppState,
    colors: &theme::ColorTheme,
) {
    let box_width = (area.width * 60 / 100).clamp(44, 72);
    let h_padding = (area.width.saturating_sub(box_width)) / 2;
    let pills_rect = Rect { x: area.x + h_padding, y: area.y, width: box_width, height: 1 };
    let line = build_status_pills(state);
    f.render_widget(Paragraph::new(line).style(Style::default().bg(colors.bg)), pills_rect);
}

/// Centra horizontalmente un area manteniendo altura completa. `max_w` es el
/// ancho maximo permitido en columnas.
fn centered_rect(area: Rect, max_w: u16) -> Rect {
    let w = area.width.min(max_w);
    let h_pad = area.width.saturating_sub(w) / 2;
    Rect { x: area.x + h_pad, y: area.y, width: w, height: area.height }
}

fn render_tagline(f: &mut Frame, area: Rect, state: &AppState) {
    let color = factory_color(&state.factory);
    let line = Line::from(vec![
        Span::styled("Tu equipo dice ", Style::default().fg(dim())),
        Span::styled("QUÉ", Style::default().fg(color).add_modifier(Modifier::BOLD)),
        Span::styled(". ingenierIA construye el ", Style::default().fg(dim())),
        Span::styled("CÓMO", Style::default().fg(color).add_modifier(Modifier::BOLD)),
        Span::styled(".", Style::default().fg(dim())),
    ]);
    f.render_widget(Paragraph::new(line).alignment(Alignment::Center), area);
}

fn render_made_by(f: &mut Frame, area: Rect) {
    let line = Line::from(vec![
        Span::styled("hecho con ", Style::default().fg(dim())),
        Span::styled(GLYPH_HEART, Style::default().fg(Color::Red)),
        Span::styled(" por ", Style::default().fg(dim())),
        Span::styled(
            "EdwinDev",
            Style::default().fg(theme::highlight()).add_modifier(Modifier::BOLD),
        ),
    ]);
    f.render_widget(Paragraph::new(line).alignment(Alignment::Center), area);
}

fn render_title(f: &mut Frame, area: Rect, state: &AppState) {
    let color = factory_color(&state.factory);

    let big_text = BigText::builder()
        .pixel_size(PixelSize::Quadrant)
        .style(Style::default().fg(color))
        .lines(vec!["INGENIERiA".into()])
        .build();

    let text_w: u16 = 40; // 10 chars × 4px (PixelSize::Quadrant)
    let h_pad = area.width.saturating_sub(text_w) / 2;
    let text_area = Rect { x: area.x + h_pad, y: area.y, width: text_w, height: area.height };

    f.render_widget(big_text, text_area);
}

/// Render the splash input area and return the centered input Rect for popup positioning.
fn render_input_area(
    f: &mut Frame,
    area: Rect,
    state: &AppState,
    colors: &theme::ColorTheme,
) -> Rect {
    let box_width = (area.width * 60 / 100).clamp(44, 72);
    let h_padding = (area.width.saturating_sub(box_width)) / 2;
    let input_area =
        Rect { x: area.x + h_padding, y: area.y, width: box_width, height: area.height };

    let accent = factory_color(&state.factory);

    let card_bg = Block::default().style(Style::default().bg(colors.surface));
    f.render_widget(card_bg, input_area);

    primitives::render_accent_bar(f, input_area, accent, true, state.tick_count);

    let content_area = primitives::input_inner(input_area);

    let content_rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Fill(1),   // top padding (flex)
            Constraint::Length(1), // input text
            Constraint::Fill(1),   // bottom padding (flex)
        ])
        .split(content_area);

    let input_line = if state.input.is_empty() {
        // Cursor parpadeante sobre el placeholder: visible la mitad del ciclo
        // (250ms on / 250ms off @ 4Hz) para indicar input activo.
        let blink_on = state.tick_count % 4 < 2;
        let cursor_glyph = if blink_on { GLYPH_CURSOR_BLOCK } else { " " };
        Line::from(vec![
            Span::styled(cursor_glyph, Style::default().fg(accent)),
            Span::styled(" ", Style::default()),
            Span::styled(
                crate::services::onboarding::dynamic_placeholder(&state.factory, state.tick_count),
                Style::default().fg(colors.text_dim),
            ),
        ])
    } else {
        Line::from(vec![
            Span::styled(state.input.clone(), Style::default().fg(colors.text)),
            Span::styled(GLYPH_CURSOR_BLOCK, Style::default().fg(accent)),
        ])
    };
    f.render_widget(
        Paragraph::new(input_line).style(Style::default().bg(colors.surface)),
        content_rows[1],
    );

    input_area
}

fn build_status_pills(state: &AppState) -> Line<'static> {
    let factory_label = state.factory.label();

    let (status_color, status_icon): (Color, &'static str) = match &state.server_status {
        ServerStatus::Unknown => (dim(), GLYPH_IDLE),
        ServerStatus::Online(_) => (green(), GLYPH_PENDING),
        ServerStatus::Offline(_) => (red(), GLYPH_PENDING),
    };

    let mut spans: Vec<Span<'static>> = vec![
        Span::styled(
            factory_label,
            Style::default().fg(factory_color(&state.factory)).add_modifier(Modifier::BOLD),
        ),
        Span::styled("  ", Style::default()),
        Span::styled(format!("{} {}", status_icon, state.model), Style::default().fg(status_color)),
    ];

    if let ServerStatus::Online(h) = &state.server_status {
        spans.push(Span::styled("  ", Style::default()));
        spans.push(Span::styled(format!("{} docs", h.docs.total), Style::default().fg(dim())));
    }

    Line::from(spans)
}

fn render_keyboard_hints(f: &mut Frame, area: Rect, state: &AppState) {
    let armed = quit_armed(state);
    let spans = if armed {
        vec![
            Span::styled("ctrl+c ", Style::default().fg(theme::yellow())),
            Span::styled(
                "otra vez para salir",
                Style::default().fg(theme::yellow()).add_modifier(Modifier::BOLD),
            ),
        ]
    } else {
        vec![
            Span::styled("tab ", Style::default().fg(dim())),
            Span::styled("factory", Style::default().fg(theme::dimmer())),
            Span::styled("   ", Style::default()),
            Span::styled(": ", Style::default().fg(dim())),
            Span::styled("comandos", Style::default().fg(theme::dimmer())),
            Span::styled("   ", Style::default()),
            Span::styled("/ ", Style::default().fg(dim())),
            Span::styled("buscar", Style::default().fg(theme::dimmer())),
            Span::styled("   ", Style::default()),
            Span::styled("ctrl+c ", Style::default().fg(dim())),
            Span::styled("salir", Style::default().fg(theme::dimmer())),
        ]
    };
    f.render_widget(Paragraph::new(Line::from(spans)).alignment(Alignment::Center), area);
}

/// `true` si el usuario ya presiono Ctrl+C una vez y la ventana de confirmacion
/// sigue abierta.
pub(crate) fn quit_armed(state: &AppState) -> bool {
    state.quit_armed_until.is_some_and(|t| state.tick_count <= t)
}

fn render_status_bar(f: &mut Frame, area: Rect, state: &AppState) {
    let colors = state.active_theme.colors();
    let bar_block = Block::default().style(Style::default().bg(colors.bar_bg));
    f.render_widget(bar_block, area);

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Fill(1), Constraint::Fill(1)])
        .split(area);

    let mut left_spans: Vec<Span<'static>> = Vec::new();
    let (left_text, left_color) = match &state.server_status {
        ServerStatus::Online(h) => (
            format!(" {} · {} · uptime {}s", state.developer, h.version, h.uptime_seconds),
            colors.text_secondary,
        ),
        ServerStatus::Offline(e) => (format!(" {}", widgets::truncate(e, 50)), colors.red),
        ServerStatus::Unknown => {
            (format!(" {} · connecting...", state.developer), colors.text_secondary)
        }
    };
    left_spans.push(Span::styled(left_text, Style::default().fg(left_color)));
    if let Some(ref report) = state.doctor_report {
        left_spans.push(widgets::doctor::render_health_indicator(&report.overall()));
    }
    f.render_widget(Paragraph::new(Line::from(left_spans)), cols[0]);

    let brand = vec![
        Span::styled(
            "ingenier",
            Style::default().fg(colors.brand_primary).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "IA",
            Style::default().fg(colors.brand_secondary).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("  v{} ", env!("CARGO_PKG_VERSION")),
            Style::default().fg(colors.text_dim),
        ),
    ];
    f.render_widget(Paragraph::new(Line::from(brand)).alignment(Alignment::Right), cols[1]);
}
