use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph},
    Frame,
};
use tui_big_text::{BigText, PixelSize};

use super::theme::{
    bg, blue, brand_blue, brand_green, dim, dimmer, green, purple, red, surface, white, yellow,
    GLYPH_CHECKING, GLYPH_CURSOR, GLYPH_CURSOR_BLOCK, GLYPH_ERROR, GLYPH_IDLE, GLYPH_PENDING,
};
use crate::state::{
    AppState, UrlValidation, WizardModelPhase, WizardStep, WIZARD_PROVIDERS, WIZARD_ROLES,
};

pub fn render(f: &mut Frame, state: &AppState) {
    let bg = Block::default().style(Style::default().bg(bg()));
    f.render_widget(bg, f.area());

    let card_height: u16 = match state.wizard.step {
        WizardStep::ServerUrl => 5,
        WizardStep::Name => 4,
        WizardStep::Model => match state.wizard.model_phase {
            WizardModelPhase::SelectProvider => (2 + WIZARD_PROVIDERS.len() as u16).min(9),
            WizardModelPhase::Authenticating => 7,
            WizardModelPhase::SelectModel => {
                let n = state.wizard.copilot.models.len().max(1) as u16;
                (3 + n).min(16)
            }
        },
        WizardStep::Role => (2 + WIZARD_ROLES.len() as u16 * 2).min(10),
    };

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Fill(1),
            Constraint::Length(1),           // extra spacing above title
            Constraint::Length(8),           // title
            Constraint::Length(2),           // spacing
            Constraint::Length(1),           // step indicator
            Constraint::Length(1),           // spacing
            Constraint::Length(card_height), // step card
            Constraint::Length(2),           // spacing
            Constraint::Length(1),           // hints
            Constraint::Fill(1),
            Constraint::Length(1), // status bar
        ])
        .split(f.area());

    render_title(f, rows[2]);
    render_step_indicator(f, rows[4], state);
    render_step_card(f, rows[6], state);
    render_hints(f, rows[8], state);
    render_status_bar(f, rows[10]);
}

fn render_title(f: &mut Frame, area: Rect) {
    let big_text = BigText::builder()
        .pixel_size(PixelSize::Quadrant)
        .style(Style::default().fg(blue()))
        .lines(vec!["INGENIERiA".into()])
        .centered()
        .build();
    f.render_widget(big_text, area);
}

fn render_step_indicator(f: &mut Frame, area: Rect, state: &AppState) {
    let current = state.wizard.step.step_number();
    let total = WizardStep::total();

    let mut spans: Vec<Span<'static>> = Vec::new();
    for i in 1..=total {
        if i > 1 {
            spans.push(Span::styled(
                " ── ",
                Style::default().fg(if i <= current {
                    super::theme::border()
                } else {
                    super::theme::step_inactive()
                }),
            ));
        }
        let (dot, color) = if i < current {
            (GLYPH_PENDING, green())
        } else if i == current {
            (GLYPH_PENDING, blue())
        } else {
            (GLYPH_IDLE, super::theme::border())
        };
        spans.push(Span::styled(dot, Style::default().fg(color)));
    }
    spans.push(Span::styled(format!("  {}/{}", current, total), Style::default().fg(dim())));

    f.render_widget(Paragraph::new(Line::from(spans)).alignment(Alignment::Center), area);
}

fn render_step_card(f: &mut Frame, area: Rect, state: &AppState) {
    let card_width = (area.width * 55 / 100).clamp(44, 65);
    let h_pad = (area.width.saturating_sub(card_width)) / 2;
    let card_area = Rect { x: area.x + h_pad, y: area.y, width: card_width, height: area.height };

    // Draw surface background
    let card_bg = Block::default().style(Style::default().bg(surface()));
    f.render_widget(card_bg, card_area);

    // Draw accent bar (uses shared primitives for visual consistency)
    let accent_color = step_accent(&state.wizard.step);
    super::primitives::render_accent_bar(f, card_area, accent_color, false, 0);

    // Content area (right of accent)
    let content = Rect {
        x: card_area.x + 2,
        y: card_area.y,
        width: card_area.width.saturating_sub(3),
        height: card_area.height,
    };

    match state.wizard.step {
        WizardStep::ServerUrl => render_server_url_step(f, content, state),
        WizardStep::Name => render_name_step(f, content, state),
        WizardStep::Model => render_model_step(f, content, state),
        WizardStep::Role => render_role_step(f, content, state),
    }
}

fn step_accent(step: &WizardStep) -> Color {
    match step {
        WizardStep::ServerUrl => blue(),
        WizardStep::Name => blue(),
        WizardStep::Model => green(),
        WizardStep::Role => purple(),
    }
}

fn render_server_url_step(f: &mut Frame, area: Rect, state: &AppState) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // label
            Constraint::Length(1), // spacing
            Constraint::Length(1), // input
            Constraint::Length(1), // validation
            Constraint::Fill(1),
        ])
        .split(area);

    // Label
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                "URL del servidor",
                Style::default().fg(white()).add_modifier(Modifier::BOLD),
            ),
            Span::styled("  Pregunta a tu tech lead si no la sabes", Style::default().fg(dim())),
        ])),
        rows[0],
    );

    // Input with cursor
    let url_text = if state.wizard.server_url_input.is_empty() {
        Line::from(vec![
            Span::styled(GLYPH_CURSOR_BLOCK, Style::default().fg(blue())),
            Span::styled("http://localhost:3001", Style::default().fg(dim())),
        ])
    } else {
        render_text_with_cursor(&state.wizard.server_url_input, state.wizard.server_url_cursor)
    };
    f.render_widget(Paragraph::new(url_text), rows[2]);

    // Validation
    let validation = match &state.wizard.url_validation {
        UrlValidation::Idle => Line::from(Span::styled(
            "Ej: http://ingenieria.example.com:3001",
            Style::default().fg(dimmer()),
        )),
        UrlValidation::Checking => Line::from(Span::styled(
            format!("{GLYPH_CHECKING} Verificando conexión..."),
            Style::default().fg(yellow()),
        )),
        UrlValidation::Valid => Line::from(Span::styled(
            format!("{GLYPH_PENDING} Servidor encontrado"),
            Style::default().fg(green()),
        )),
        UrlValidation::Invalid(msg) => Line::from(vec![
            Span::styled(format!("{GLYPH_ERROR} "), Style::default().fg(red())),
            Span::styled(msg.clone(), Style::default().fg(red())),
        ]),
    };
    f.render_widget(Paragraph::new(validation), rows[3]);
}

fn render_name_step(f: &mut Frame, area: Rect, state: &AppState) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // label
            Constraint::Length(1), // spacing
            Constraint::Length(1), // input
            Constraint::Fill(1),
        ])
        .split(area);

    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("Tu nombre", Style::default().fg(white()).add_modifier(Modifier::BOLD)),
            Span::styled("  Visible en el panel de sesiones", Style::default().fg(dim())),
        ])),
        rows[0],
    );

    let name_text = if state.wizard.name_input.is_empty() {
        Line::from(vec![
            Span::styled(GLYPH_CURSOR_BLOCK, Style::default().fg(blue())),
            Span::styled("Escribe tu nombre...", Style::default().fg(dim())),
        ])
    } else {
        render_text_with_cursor(&state.wizard.name_input, state.wizard.name_cursor)
    };
    f.render_widget(Paragraph::new(name_text), rows[2]);
}

fn render_model_step(f: &mut Frame, area: Rect, state: &AppState) {
    match state.wizard.model_phase {
        WizardModelPhase::SelectProvider => {
            super::wizard_model::render_provider_phase(f, area, state)
        }
        WizardModelPhase::Authenticating => super::wizard_auth::render_auth_phase(f, area, state),
        WizardModelPhase::SelectModel => {
            super::wizard_model::render_select_model_phase(f, area, state)
        }
    }
}

fn render_role_step(f: &mut Frame, area: Rect, state: &AppState) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // label + spacing
            Constraint::Length(2), // role 1
            Constraint::Length(2), // role 2
            Constraint::Length(2), // role 3
            Constraint::Fill(1),
        ])
        .split(area);

    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("Tu rol", Style::default().fg(white()).add_modifier(Modifier::BOLD)),
            Span::styled("  Define el factory por defecto", Style::default().fg(dim())),
        ])),
        rows[0],
    );

    for (i, (_, label, desc)) in WIZARD_ROLES.iter().enumerate() {
        let is_selected = i == state.wizard.role_cursor;
        let row_idx = i + 1;
        if row_idx >= rows.len() {
            break;
        }

        let line = if is_selected {
            Line::from(vec![
                Span::styled(
                    format!("{GLYPH_CURSOR} "),
                    Style::default().fg(purple()).add_modifier(Modifier::BOLD),
                ),
                Span::styled(*label, Style::default().fg(white()).add_modifier(Modifier::BOLD)),
                Span::styled(format!("  {desc}"), Style::default().fg(dim())),
            ])
        } else {
            Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled(*label, Style::default().fg(dim())),
                Span::styled(format!("  {desc}"), Style::default().fg(super::theme::muted())),
            ])
        };

        let bg = if is_selected {
            Style::default().bg(super::theme::surface_purple())
        } else {
            Style::default().bg(surface())
        };
        f.render_widget(Paragraph::new(line).style(bg), rows[row_idx]);
    }
}

fn render_hints(f: &mut Frame, area: Rect, state: &AppState) {
    let hints = match state.wizard.step {
        WizardStep::ServerUrl => {
            vec![("←/→ ", "mover"), ("Home/End ", "inicio/fin"), ("enter ", "continuar")]
        }
        WizardStep::Name => vec![("←/→ ", "mover"), ("enter ", "continuar"), ("esc ", "volver")],
        WizardStep::Model => match state.wizard.model_phase {
            WizardModelPhase::Authenticating => vec![("c ", "copiar codigo"), ("esc ", "cancelar")],
            WizardModelPhase::SelectModel => {
                vec![("↑/↓ ", "navegar"), ("enter ", "confirmar"), ("esc ", "volver")]
            }
            _ => vec![("↑/↓ ", "navegar"), ("enter ", "confirmar"), ("esc ", "volver")],
        },
        WizardStep::Role => vec![
            ("1/2/3 ", "seleccionar"),
            ("↑/↓ ", "navegar"),
            ("enter ", "confirmar"),
            ("esc ", "volver"),
        ],
    };

    let mut spans: Vec<Span<'static>> = Vec::new();
    for (i, (key, action)) in hints.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled("   ", Style::default()));
        }
        spans.extend(super::primitives::hint_spans(key, action));
    }

    f.render_widget(Paragraph::new(Line::from(spans)).alignment(Alignment::Center), area);
}

fn render_status_bar(f: &mut Frame, area: Rect) {
    let bar = Block::default().style(Style::default().bg(super::theme::bar_bg()));
    f.render_widget(bar, area);

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Fill(1), Constraint::Fill(1)])
        .split(area);

    f.render_widget(
        Paragraph::new(Span::styled(" Primera configuración", Style::default().fg(dim()))),
        cols[0],
    );

    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("ingenier", Style::default().fg(brand_blue()).add_modifier(Modifier::BOLD)),
            Span::styled(
                "IA",
                Style::default().fg(brand_green()).add_modifier(Modifier::BOLD),
            ),
            Span::styled(format!("  v{} ", env!("CARGO_PKG_VERSION")), Style::default().fg(dim())),
        ]))
        .alignment(Alignment::Right),
        cols[1],
    );
}

// ── Text input with cursor ──────────────────────────────────────────────────

fn render_text_with_cursor(text: &str, cursor: usize) -> Line<'static> {
    let cursor = cursor.min(text.len());
    let before = &text[..cursor];
    let at_cursor = text[cursor..].chars().next();
    let after_start = cursor + at_cursor.map(|c| c.len_utf8()).unwrap_or(0);
    let after = &text[after_start..];

    let mut spans = Vec::with_capacity(3);
    if !before.is_empty() {
        spans.push(Span::styled(before.to_string(), Style::default().fg(white())));
    }
    // Cursor: highlight current char or show block if at end
    match at_cursor {
        Some(c) => spans.push(Span::styled(c.to_string(), Style::default().fg(bg()).bg(blue()))),
        None => spans.push(Span::styled(GLYPH_CURSOR_BLOCK, Style::default().fg(blue()))),
    }
    if !after.is_empty() {
        spans.push(Span::styled(after.to_string(), Style::default().fg(white())));
    }
    Line::from(spans)
}
