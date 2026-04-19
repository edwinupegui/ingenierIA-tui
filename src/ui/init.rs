use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use super::theme::{
    bg, blue, dim, green, red, surface, white, yellow, GLYPH_SUCCESS, GLYPH_WARNING,
};
use crate::{
    domain::init_types::{InitClient, ProjectType},
    state::{AppState, InitStep},
};

pub fn render(f: &mut Frame, state: &AppState) {
    let bg = Block::default().style(Style::default().bg(bg()));
    f.render_widget(bg, f.area());

    let init = &state.init;

    let card_height: u16 = match init.step {
        InitStep::SelectType => 5 + ProjectType::ALL.len() as u16,
        InitStep::SelectClient => 8,
        InitStep::Confirm => 12,
        InitStep::Running => 5,
        InitStep::Done => 4 + init.results.len().min(10) as u16,
    };

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Fill(1),
            Constraint::Length(3),           // title
            Constraint::Length(1),           // spacing
            Constraint::Length(1),           // step info
            Constraint::Length(1),           // spacing
            Constraint::Length(card_height), // content card
            Constraint::Length(2),           // spacing
            Constraint::Length(1),           // hints
            Constraint::Fill(1),
        ])
        .split(f.area());

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Fill(1),
            Constraint::Length(60.min(f.area().width.saturating_sub(4))),
            Constraint::Fill(1),
        ])
        .split(rows[5]);

    render_title(f, rows[1]);
    render_step_info(f, rows[3], state);
    render_card(f, cols[1], state);
    render_hints(f, rows[7], state);
}

fn render_title(f: &mut Frame, area: Rect) {
    let title = Paragraph::new(Line::from(vec![
        Span::styled(
            " INGENIERiA INIT ",
            Style::default().fg(bg()).bg(blue()).add_modifier(Modifier::BOLD),
        ),
        Span::styled(" Inicializar proyecto ", Style::default().fg(dim())),
    ]))
    .alignment(Alignment::Center);
    f.render_widget(title, area);
}

fn render_step_info(f: &mut Frame, area: Rect, state: &AppState) {
    let init = &state.init;
    let (step_num, step_label) = match init.step {
        InitStep::SelectType => (1, "Tipo de proyecto"),
        InitStep::SelectClient => (2, "Cliente AI"),
        InitStep::Confirm => (3, "Confirmar"),
        InitStep::Running => (3, "Escribiendo archivos..."),
        InitStep::Done => (3, "Completado"),
    };

    let dir_display = if init.project_dir.is_empty() {
        "...".to_string()
    } else {
        // Solo mostrar el último directorio
        std::path::Path::new(&init.project_dir)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| init.project_dir.clone())
    };

    let info = Paragraph::new(Line::from(vec![
        Span::styled(
            format!("  Paso {step_num}/3"),
            Style::default().fg(blue()).add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!("  {step_label}"), Style::default().fg(dim())),
        Span::styled("  │  ", Style::default().fg(dim())),
        Span::styled(format!("Proyecto: {dir_display}"), Style::default().fg(white())),
    ]))
    .alignment(Alignment::Center);
    f.render_widget(info, area);
}

fn render_card(f: &mut Frame, area: Rect, state: &AppState) {
    let init = &state.init;

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(dim()))
        .style(Style::default().bg(surface()));
    let inner = block.inner(area);
    f.render_widget(block, area);

    match init.step {
        InitStep::SelectType => render_select_type(f, inner, state),
        InitStep::SelectClient => render_select_client(f, inner, state),
        InitStep::Confirm => render_confirm(f, inner, state),
        InitStep::Running => render_running(f, inner),
        InitStep::Done => render_done(f, inner, state),
    }
}

fn render_select_type(f: &mut Frame, area: Rect, state: &AppState) {
    let init = &state.init;
    let mut lines = vec![
        Line::from(Span::styled(
            "  Tipo de proyecto detectado:",
            Style::default().fg(white()).add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            format!("  Detectado: {}", init.detected_type.label()),
            Style::default().fg(yellow()),
        )),
        Line::from(""),
    ];

    for (i, pt) in ProjectType::ALL.iter().enumerate() {
        let selected = i == init.type_cursor;
        let marker = if selected { " > " } else { "   " };
        let style = if selected {
            Style::default().fg(blue()).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(white())
        };
        lines.push(Line::from(Span::styled(format!("{marker}{}", pt.label()), style)));
    }

    let type_list = Paragraph::new(lines);
    f.render_widget(type_list, area);
}

fn render_select_client(f: &mut Frame, area: Rect, state: &AppState) {
    let init = &state.init;
    let mut lines = vec![
        Line::from(Span::styled(
            "  Cliente AI a configurar:",
            Style::default().fg(white()).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
    ];

    for (i, c) in InitClient::ALL.iter().enumerate() {
        let selected = i == init.client_cursor;
        let marker = if selected { " > " } else { "   " };
        let style = if selected {
            Style::default().fg(blue()).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(white())
        };
        lines.push(Line::from(Span::styled(format!("{marker}{}", c.label()), style)));
    }

    let client_list = Paragraph::new(lines);
    f.render_widget(client_list, area);
}

fn render_confirm(f: &mut Frame, area: Rect, state: &AppState) {
    let init = &state.init;
    let project_type = ProjectType::ALL.get(init.type_cursor).map(|t| t.label()).unwrap_or("?");
    let client = InitClient::ALL.get(init.client_cursor).map(|c| c.label()).unwrap_or("?");

    let lines = vec![
        Line::from(Span::styled(
            "  Resumen de inicialización:",
            Style::default().fg(white()).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Directorio: ", Style::default().fg(dim())),
            Span::styled(&init.project_dir, Style::default().fg(white())),
        ]),
        Line::from(vec![
            Span::styled("  Tipo:       ", Style::default().fg(dim())),
            Span::styled(project_type, Style::default().fg(yellow())),
        ]),
        Line::from(vec![
            Span::styled("  Cliente:    ", Style::default().fg(dim())),
            Span::styled(client, Style::default().fg(blue())),
        ]),
        Line::from(vec![
            Span::styled("  Servidor:   ", Style::default().fg(dim())),
            Span::styled(
                {
                    let url = &state.wizard.server_url_input;
                    if url.is_empty() {
                        "configurado"
                    } else {
                        url.as_str()
                    }
                },
                Style::default().fg(green()),
            ),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "  Se crearán: .mcp.json, CLAUDE.md, .cloud/, .gitignore",
            Style::default().fg(dim()),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "  Enter para confirmar │ Esc para volver",
            Style::default().fg(blue()),
        )),
    ];

    let summary_paragraph = Paragraph::new(lines);
    f.render_widget(summary_paragraph, area);
}

fn render_running(f: &mut Frame, area: Rect) {
    let lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            "  Escribiendo archivos...",
            Style::default().fg(yellow()).add_modifier(Modifier::BOLD),
        )),
    ];
    let progress_paragraph = Paragraph::new(lines);
    f.render_widget(progress_paragraph, area);
}

fn render_done(f: &mut Frame, area: Rect, state: &AppState) {
    let init = &state.init;
    let mut lines = Vec::new();

    if let Some(err) = &init.error {
        lines.push(Line::from(Span::styled(
            format!("  Error: {err}"),
            Style::default().fg(red()).add_modifier(Modifier::BOLD),
        )));
    } else {
        lines.push(Line::from(Span::styled(
            "  ingenierIA inicializado!",
            Style::default().fg(green()).add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(""));

        for r in &init.results {
            let (icon, color) =
                if r.created { (GLYPH_SUCCESS, green()) } else { (GLYPH_WARNING, yellow()) };
            lines.push(Line::from(vec![
                Span::styled(format!("  {icon} "), Style::default().fg(color)),
                Span::styled(&r.path, Style::default().fg(white())),
                Span::styled(format!("  {}", r.description), Style::default().fg(dim())),
            ]));
        }
    }

    let results_paragraph = Paragraph::new(lines);
    f.render_widget(results_paragraph, area);
}

fn render_hints(f: &mut Frame, area: Rect, state: &AppState) {
    let hints = match state.init.step {
        InitStep::SelectType | InitStep::SelectClient => {
            "j/k navegar  Enter seleccionar  Esc volver"
        }
        InitStep::Confirm => "Enter confirmar  Esc volver",
        InitStep::Running => "Procesando...",
        InitStep::Done => "Enter continuar  Esc volver",
    };

    let hints_paragraph =
        Paragraph::new(Line::from(Span::styled(hints, Style::default().fg(dim()))))
            .alignment(Alignment::Center);
    f.render_widget(hints_paragraph, area);
}
