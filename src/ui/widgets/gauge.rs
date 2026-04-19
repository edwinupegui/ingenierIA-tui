use ratatui::{style::Style, text::Span};

use crate::ui::theme::{green, red, yellow};

/// Caracteres Unicode para llenado fraccional de barras.
const FILL_CHARS: &[char] = &['░', '▏', '▎', '▍', '▌', '▋', '▊', '▉', '█'];

/// Renderiza una barra de progreso con llenado fraccional Unicode.
#[expect(dead_code, reason = "planned for D.2 Context Compaction gauge widget")]
///
/// Retorna un Span con la barra renderizada del ancho solicitado.
/// El color cambia segun el porcentaje: green() (0-60%), yellow() (60-80%), red() (80-100%).
pub fn gauge_span(percent: f64, width: usize) -> Span<'static> {
    let clamped = percent.clamp(0.0, 100.0);
    let color = if clamped < 60.0 {
        green()
    } else if clamped < 80.0 {
        yellow()
    } else {
        red()
    };

    let fill = (clamped / 100.0) * width as f64;
    let full_blocks = fill as usize;
    let fractional = fill - full_blocks as f64;
    let frac_idx = (fractional * (FILL_CHARS.len() - 1) as f64).round() as usize;

    let mut bar = String::with_capacity(width);
    for _ in 0..full_blocks.min(width) {
        bar.push('█');
    }
    if full_blocks < width && frac_idx > 0 {
        bar.push(FILL_CHARS[frac_idx]);
    }
    while bar.chars().count() < width {
        bar.push('░');
    }

    Span::styled(bar, Style::default().fg(color))
}
