//! Virtual scroll window calculator (E30b).
//!
//! Helper puro que decide que rango de mensajes debe renderizar el chat
//! para una sesion larga. La idea: para chats con cientos de mensajes, no
//! tiene sentido construir Lines de los que claramente quedan fuera del
//! viewport — incluso si el cache de markdown ya esta materializado, evitar
//! el clone + push de Spans ahorra ciclos por frame.
//!
//! El calculo opera sobre `MessageHeight` — un par `(idx, lines)` que el
//! caller deriva de `cached_lines.len()` u otra heuristica. La salida es un
//! rango `[start, end)` de indices a renderizar y un offset adicional de
//! scroll dentro del primer mensaje visible.
//!
//! Sprint 10 expone la utilidad + tests; la integracion en `chat_render.rs`
//! se hara en un PR separado para mantener el cambio aislado.

use std::ops::Range;

/// Cantidad de mensajes contiguos que se conservan por encima/debajo del
/// viewport como buffer (para que un pequeno scroll no obligue a re-construir
/// la lista renderizada).
pub const VIRTUAL_OVERSCAN: usize = 2;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MessageHeight {
    pub index: usize,
    pub lines: u16,
}

#[derive(Debug, Clone, PartialEq)]
pub struct VirtualWindow {
    /// Rango de indices de mensajes que deben renderizarse.
    pub range: Range<usize>,
    /// Offset extra de scroll que el caller debe restar a `scroll_offset`
    /// para alinear el primer mensaje del rango con el viewport.
    pub line_offset: u16,
    /// Total de lineas estimadas (suma de `lines` en `heights`).
    pub total_lines: u16,
}

impl VirtualWindow {
    /// Calcula la ventana visible dado un scroll absoluto en lineas y la
    /// altura disponible del viewport.
    ///
    /// `scroll` es el offset desde la primera linea del primer mensaje;
    /// si vale `u16::MAX` se interpreta como "ir al final".
    pub fn compute(heights: &[MessageHeight], scroll: u16, viewport: u16) -> Self {
        let total_lines: u32 = heights.iter().map(|h| h.lines as u32).sum();
        let total_lines_u16 = total_lines.min(u16::MAX as u32) as u16;

        if heights.is_empty() || viewport == 0 {
            return Self { range: 0..heights.len(), line_offset: 0, total_lines: total_lines_u16 };
        }

        let max_scroll = total_lines_u16.saturating_sub(viewport);
        let target_scroll = if scroll == u16::MAX { max_scroll } else { scroll.min(max_scroll) };

        let (mut start_idx, line_offset) = locate_index(heights, target_scroll);
        let end_idx = locate_end(heights, start_idx, line_offset, viewport);

        // Aplica overscan superior, ajustando line_offset si crecimos.
        let saved_start = start_idx;
        start_idx = start_idx.saturating_sub(VIRTUAL_OVERSCAN);
        let extra: u32 = heights[start_idx..saved_start].iter().map(|h| h.lines as u32).sum();
        let new_line_offset = (line_offset as u32 + extra).min(u16::MAX as u32) as u16;

        // Overscan inferior.
        let end_idx = (end_idx + VIRTUAL_OVERSCAN).min(heights.len());

        Self {
            range: start_idx..end_idx,
            line_offset: new_line_offset,
            total_lines: total_lines_u16,
        }
    }
}

/// Para un scroll absoluto en lineas, devuelve `(idx, offset_dentro_del_msg)`.
fn locate_index(heights: &[MessageHeight], scroll: u16) -> (usize, u16) {
    let mut acc: u32 = 0;
    for (i, h) in heights.iter().enumerate() {
        let next = acc + h.lines as u32;
        if next > scroll as u32 {
            return (i, (scroll as u32 - acc) as u16);
        }
        acc = next;
    }
    (heights.len().saturating_sub(1), 0)
}

/// Encuentra el ultimo mensaje (exclusivo) que entra en el viewport.
fn locate_end(
    heights: &[MessageHeight],
    start_idx: usize,
    line_offset: u16,
    viewport: u16,
) -> usize {
    let mut consumed: u32 = 0;
    let target = viewport as u32 + line_offset as u32;
    for (i, h) in heights.iter().enumerate().skip(start_idx) {
        consumed += h.lines as u32;
        if consumed >= target {
            return (i + 1).min(heights.len());
        }
    }
    heights.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn h(idx: usize, lines: u16) -> MessageHeight {
        MessageHeight { index: idx, lines }
    }

    #[test]
    fn empty_input_returns_empty_range() {
        let w = VirtualWindow::compute(&[], 0, 10);
        assert_eq!(w.range, 0..0);
        assert_eq!(w.total_lines, 0);
    }

    #[test]
    fn viewport_zero_returns_full_range() {
        let heights = vec![h(0, 5), h(1, 5)];
        let w = VirtualWindow::compute(&heights, 0, 0);
        assert_eq!(w.range, 0..2);
    }

    #[test]
    fn small_history_renders_everything() {
        let heights = vec![h(0, 3), h(1, 4), h(2, 5)];
        let w = VirtualWindow::compute(&heights, 0, 20);
        assert_eq!(w.range, 0..3);
        assert_eq!(w.total_lines, 12);
    }

    #[test]
    fn scroll_at_max_shows_tail() {
        let heights: Vec<_> = (0..50).map(|i| h(i, 4)).collect();
        let w = VirtualWindow::compute(&heights, u16::MAX, 12);
        // El final debe estar incluido y el rango ser pequeno (overscan + window).
        assert_eq!(w.range.end, 50);
        assert!(w.range.len() < heights.len());
    }

    #[test]
    fn scroll_zero_starts_at_zero() {
        let heights: Vec<_> = (0..30).map(|i| h(i, 3)).collect();
        let w = VirtualWindow::compute(&heights, 0, 9);
        assert_eq!(w.range.start, 0);
        assert!(w.range.len() < heights.len());
    }

    #[test]
    fn middle_scroll_centers_on_target() {
        let heights: Vec<_> = (0..40).map(|i| h(i, 2)).collect();
        // Total = 80 lineas. Scroll a 30 deberia poner el indice ~15.
        let w = VirtualWindow::compute(&heights, 30, 8);
        assert!(w.range.start <= 15);
        assert!(w.range.end >= 19);
    }

    #[test]
    fn overscan_added_above_below() {
        let heights: Vec<_> = (0..20).map(|i| h(i, 5)).collect();
        let w = VirtualWindow::compute(&heights, 50, 10);
        // Overscan = 2, viewport = 10 (=2 mensajes), esperamos rango de ~6.
        assert!(w.range.len() >= 4);
    }

    #[test]
    fn locate_index_finds_message_at_offset() {
        let heights = vec![h(0, 5), h(1, 5), h(2, 5)];
        let (idx, off) = locate_index(&heights, 7);
        assert_eq!(idx, 1);
        assert_eq!(off, 2);
    }

    #[test]
    fn locate_end_respects_viewport() {
        let heights = vec![h(0, 3), h(1, 3), h(2, 3), h(3, 3)];
        // start=1, offset=0, viewport=4 → necesitamos ~4 lineas más.
        // h[1]=3 + h[2]=3 → 6 >= 4. end=3.
        assert_eq!(locate_end(&heights, 1, 0, 4), 3);
    }
}
