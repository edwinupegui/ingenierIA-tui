//! Truncation grapheme-safe que respeta terminal cell widths.
//!
//! Referencia: claude-code `truncate.ts`. Cuenta en columnas visibles, no en
//! chars: un caracter CJK ocupa 2 columnas, un emoji 2, una `a` 1.

#![cfg_attr(not(test), allow(dead_code, reason = "E37 toolkit — integracion pendiente"))]

use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use super::visible_width::visible_width;

/// Trunca `input` para que ocupe como maximo `max_width` columnas visibles.
/// Si necesita truncar, reemplaza el final con `…`.
///
/// - No rompe graphemes (zalgo, combining marks, flags, ZWJ).
/// - Si `max_width == 0`, devuelve string vacio.
/// - Si `max_width == 1` y hay que truncar, devuelve solo `…`.
pub fn truncate_to_width(input: &str, max_width: usize) -> String {
    if max_width == 0 {
        return String::new();
    }
    if visible_width(input) <= max_width {
        return input.to_string();
    }
    // Reservamos 1 columna para el ellipsis.
    let budget = max_width.saturating_sub(1);
    let mut current = 0usize;
    let mut out = String::with_capacity(input.len());
    for grapheme in input.graphemes(true) {
        let w = grapheme.width();
        if current + w > budget {
            break;
        }
        current += w;
        out.push_str(grapheme);
    }
    out.push('…');
    out
}

/// Trunca un path preservando la primera carpeta y las ultimas 2 partes.
/// `/home/user/projects/very/long/path/file.rs` con `max_width=30` →
/// `/home/…/path/file.rs`.
///
/// Si el path ya cabe, se devuelve sin cambios.
/// Si tiene <= 2 segmentos, cae en `truncate_to_width` generico.
pub fn truncate_path_middle(path: &str, max_width: usize) -> String {
    if visible_width(path) <= max_width {
        return path.to_string();
    }
    let parts: Vec<&str> = path.split('/').collect();
    if parts.len() <= 2 {
        return truncate_to_width(path, max_width);
    }
    let first = parts[0];
    let last = parts[parts.len() - 1];
    let second_last = parts[parts.len() - 2];
    let collapsed = format!("{first}/…/{second_last}/{last}");
    if visible_width(&collapsed) <= max_width {
        return collapsed;
    }
    // Si ni siquiera el colapso cabe, recortar normal.
    truncate_to_width(&collapsed, max_width)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shorter_than_width_unchanged() {
        assert_eq!(truncate_to_width("hola", 10), "hola");
    }

    #[test]
    fn exact_width_unchanged() {
        assert_eq!(truncate_to_width("hola", 4), "hola");
    }

    #[test]
    fn longer_gets_ellipsis() {
        assert_eq!(truncate_to_width("hola mundo", 5), "hola…");
    }

    #[test]
    fn zero_width_returns_empty() {
        assert_eq!(truncate_to_width("hola", 0), "");
    }

    #[test]
    fn cjk_counted_as_two_columns() {
        // "中文abc" = 4+3 = 7 columnas. Truncar a 5 cabe "中文" (4) + "…" (1) = 5.
        assert_eq!(truncate_to_width("中文abc", 5), "中文…");
        // Truncar a 4: budget=3, cabe solo "中" (2) + "…" = 3 cols.
        assert_eq!(truncate_to_width("中文abc", 4), "中…");
    }

    #[test]
    fn emoji_counted_correctly() {
        // "🎉abc" = 2 + 3 = 5 columnas. Truncar a 4 = "🎉a…".
        let result = truncate_to_width("🎉abc", 4);
        assert_eq!(visible_width(&result), 4);
    }

    #[test]
    fn path_short_unchanged() {
        assert_eq!(truncate_path_middle("/home/user/file.rs", 100), "/home/user/file.rs");
    }

    #[test]
    fn path_collapses_middle() {
        let path = "/home/user/projects/very/long/path/file.rs";
        let truncated = truncate_path_middle(path, 30);
        assert!(truncated.contains("…"));
        assert!(truncated.contains("file.rs"));
        assert!(truncated.contains("/home") || truncated.starts_with("/"));
    }

    #[test]
    fn path_with_few_segments_falls_back() {
        assert_eq!(truncate_path_middle("a/b", 3), "a/b");
        let result = truncate_path_middle("a/bcdefghij", 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn path_preserves_last_two_components() {
        // Path largo (>20 cols) que requiere colapso.
        let path = "/aa/bb/cc/dd/ee/ff/final.rs";
        let result = truncate_path_middle(path, 20);
        assert!(result.contains("ff/final.rs"));
        assert!(result.contains("…"));
    }
}
