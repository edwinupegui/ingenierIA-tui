//! Mini bar chart de diff +/- líneas.
//!
//! Render ratio-aware: bloques con altura proporcional al volumen relativo
//! de cada tipo de cambio. Max 5 bloques para caber en headers colapsados
//! de tool calls (ver `chat_tools.rs`).
//!
//! Inspirado en `diff-changes.tsx:84-114` de opencode-dev.
//!
//! # Ejemplo
//!
//! ```ignore
//! use ingenieria_ui::design_system::diff_bars::diff_bar_spans;
//! use ingenieria_ui::theme::DARK;
//!
//! let spans = diff_bar_spans(42, 8, DARK.green, DARK.red);
//! // spans contiene hasta 5 caracteres de bloque coloreados.
//! ```
use ratatui::style::{Color, Style};
use ratatui::text::Span;

/// Unicode block glyphs ordenados de menor a mayor altura.
/// Index 0 = ninguna altura (no se renderiza), 7 = bloque lleno.
const BLOCKS: [&str; 8] = [" ", "▁", "▂", "▃", "▄", "▅", "▆", "▇"];

/// Máximo número de bloques visibles en el mini chart.
pub const MAX_BLOCKS: usize = 5;

/// Genera spans para un mini bar chart que mezcla additions y deletions.
///
/// - `added`: líneas agregadas.
/// - `removed`: líneas eliminadas.
/// - `added_color`: color para la porción verde (típicamente `colors.green`).
/// - `removed_color`: color para la porción roja (típicamente `colors.red`).
///
/// Devuelve hasta `MAX_BLOCKS` spans con altura proporcional. Si ambos son 0
/// devuelve spans vacíos (no renderiza nada).
pub fn diff_bar_spans(
    added: u32,
    removed: u32,
    added_color: Color,
    removed_color: Color,
) -> Vec<Span<'static>> {
    let total = added.saturating_add(removed);
    if total == 0 {
        return Vec::new();
    }

    let added_blocks = ratio_blocks(added, total);
    let removed_blocks = MAX_BLOCKS.saturating_sub(added_blocks);
    let added_heights = height_ramp(added, added_blocks);
    let removed_heights = height_ramp(removed, removed_blocks);

    let mut out = Vec::with_capacity(MAX_BLOCKS);
    for h in added_heights {
        out.push(Span::styled(BLOCKS[h], Style::default().fg(added_color)));
    }
    for h in removed_heights {
        out.push(Span::styled(BLOCKS[h], Style::default().fg(removed_color)));
    }
    out
}

/// Calcula cuántos bloques ocupa `part` sobre `total` en el rango `[0, MAX_BLOCKS]`.
///
/// Reserva al menos 1 bloque si `part > 0`, y al menos 1 bloque libre si
/// `part < total` para que la porción opuesta también sea visible.
fn ratio_blocks(part: u32, total: u32) -> usize {
    if part == 0 {
        return 0;
    }
    if part == total {
        return MAX_BLOCKS;
    }
    let ratio = part as f32 / total as f32;
    let raw = (ratio * MAX_BLOCKS as f32).round() as usize;
    raw.clamp(1, MAX_BLOCKS.saturating_sub(1))
}

/// Genera una rampa de alturas decrecientes para `n` bloques.
///
/// Cuando `count` es grande los bloques empiezan altos; cuando es bajo se
/// mantienen cerca del mínimo. La rampa decrece para sugerir "picos" visuales
/// y asegurar que siempre haya al menos un bloque al nivel máximo permitido.
fn height_ramp(count: u32, n: usize) -> Vec<usize> {
    if n == 0 {
        return Vec::new();
    }
    let peak = peak_for_count(count);
    // Generar decrecientes: peak, peak-1, ... min 1.
    (0..n).map(|i| peak.saturating_sub(i).max(1)).collect()
}

/// Mapea el número de líneas cambiadas al glyph de bloque máximo a usar.
///
/// Curva log-like: pocos cambios = bloque bajo, muchos = lleno. Cap a 7 (el
/// glyph más alto en [`BLOCKS`]).
fn peak_for_count(count: u32) -> usize {
    match count {
        0 => 0,
        1..=2 => 2,
        3..=5 => 3,
        6..=15 => 4,
        16..=40 => 5,
        41..=100 => 6,
        _ => 7,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::Color;

    const GREEN_C: Color = Color::Rgb(0x48, 0xBB, 0x78);
    const RED_C: Color = Color::Rgb(0xFC, 0x81, 0x81);

    #[test]
    fn zero_zero_produces_no_spans() {
        assert!(diff_bar_spans(0, 0, GREEN_C, RED_C).is_empty());
    }

    #[test]
    fn only_additions_fills_with_added_color() {
        let spans = diff_bar_spans(10, 0, GREEN_C, RED_C);
        assert_eq!(spans.len(), MAX_BLOCKS);
        for s in &spans {
            assert_eq!(s.style.fg, Some(GREEN_C));
        }
    }

    #[test]
    fn only_removals_fills_with_removed_color() {
        let spans = diff_bar_spans(0, 10, GREEN_C, RED_C);
        assert_eq!(spans.len(), MAX_BLOCKS);
        for s in &spans {
            assert_eq!(s.style.fg, Some(RED_C));
        }
    }

    #[test]
    fn balanced_uses_both_colors() {
        let spans = diff_bar_spans(5, 5, GREEN_C, RED_C);
        assert_eq!(spans.len(), MAX_BLOCKS);
        let greens = spans.iter().filter(|s| s.style.fg == Some(GREEN_C)).count();
        let reds = spans.iter().filter(|s| s.style.fg == Some(RED_C)).count();
        assert!(greens > 0);
        assert!(reds > 0);
        assert_eq!(greens + reds, MAX_BLOCKS);
    }

    #[test]
    fn asymmetric_favors_majority() {
        let spans = diff_bar_spans(40, 2, GREEN_C, RED_C);
        let greens = spans.iter().filter(|s| s.style.fg == Some(GREEN_C)).count();
        let reds = spans.iter().filter(|s| s.style.fg == Some(RED_C)).count();
        assert!(greens > reds);
    }

    #[test]
    fn never_exceeds_max_blocks() {
        for (a, r) in [(1, 0), (1000, 1000), (0, 500), (7, 3)] {
            let spans = diff_bar_spans(a, r, GREEN_C, RED_C);
            assert!(spans.len() <= MAX_BLOCKS);
        }
    }

    #[test]
    fn high_volume_reaches_max_peak() {
        let spans = diff_bar_spans(500, 0, GREEN_C, RED_C);
        let first = spans.first().unwrap();
        assert_eq!(first.content, BLOCKS[7]);
    }

    #[test]
    fn low_volume_uses_low_peak() {
        let spans = diff_bar_spans(1, 0, GREEN_C, RED_C);
        let first = spans.first().unwrap();
        assert_eq!(first.content, BLOCKS[2]);
    }
}
