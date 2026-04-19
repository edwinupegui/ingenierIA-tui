//! Calculo del ancho visible en columnas de terminal.
//!
//! Referencia: claude-code `stringWidth`. Strip ANSI escape sequences antes
//! de medir, y usa `unicode-width` para manejar CJK (ancho 2), emoji y
//! combinaciones correctamente.

#![cfg_attr(not(test), allow(dead_code, reason = "E37 toolkit — integracion pendiente"))]

use unicode_width::UnicodeWidthStr;

/// Elimina ANSI escape sequences (CSI + SGR) del input.
///
/// Soporta:
///   - `ESC [ ... m` (SGR: colores, bold, etc.)
///   - `ESC [ ... <final>` (CSI genericas, cualquier letra ASCII final)
///   - `ESC ]` ... `BEL` u `ESC \` (OSC: hyperlinks, titulos)
pub fn strip_ansi(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = String::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        if c == 0x1B && i + 1 < bytes.len() {
            let next = bytes[i + 1];
            if next == b'[' {
                i += 2;
                // Avanzar hasta encontrar un byte final (letra ASCII @-~).
                while i < bytes.len() && !(0x40..=0x7E).contains(&bytes[i]) {
                    i += 1;
                }
                i = i.saturating_add(1); // consumir el byte final
                continue;
            } else if next == b']' {
                // OSC: terminar en BEL (0x07) o ST (ESC \).
                i += 2;
                while i < bytes.len() {
                    if bytes[i] == 0x07 {
                        i += 1;
                        break;
                    }
                    if bytes[i] == 0x1B && i + 1 < bytes.len() && bytes[i + 1] == b'\\' {
                        i += 2;
                        break;
                    }
                    i += 1;
                }
                continue;
            }
        }
        // Push byte actual preservando UTF-8 multi-byte.
        let len = utf8_byte_len(c);
        let end = (i + len).min(bytes.len());
        if let Ok(s) = std::str::from_utf8(&bytes[i..end]) {
            out.push_str(s);
        }
        i = end;
    }
    out
}

/// Longitud en bytes de un caracter UTF-8 dado su primer byte.
/// Para continuation bytes (0x80..=0xBF) devuelve 1 (se saltan defensivamente).
fn utf8_byte_len(first: u8) -> usize {
    match first {
        0..=0xBF => 1,
        0xC0..=0xDF => 2,
        0xE0..=0xEF => 3,
        _ => 4,
    }
}

/// Calcula el ancho visible en columnas de terminal, ignorando ANSI escapes.
pub fn visible_width(input: &str) -> usize {
    strip_ansi(input).width()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_width_equals_len() {
        assert_eq!(visible_width("hello"), 5);
        assert_eq!(visible_width(""), 0);
    }

    #[test]
    fn cjk_chars_are_width_2() {
        assert_eq!(visible_width("中文"), 4);
        assert_eq!(visible_width("日本語"), 6);
    }

    #[test]
    fn emoji_width_2() {
        // Emoji basicos son width 2.
        assert_eq!(visible_width("❤"), 1); // heart simbolo (narrow)
        assert_eq!(visible_width("🎉"), 2); // party popper
    }

    #[test]
    fn ansi_escapes_are_stripped() {
        let colored = "\x1b[31mred\x1b[0m";
        assert_eq!(visible_width(colored), 3);
    }

    #[test]
    fn ansi_complex_sequence() {
        // 24-bit color + bold
        let s = "\x1b[1;38;2;255;0;0mhola\x1b[0m";
        assert_eq!(visible_width(s), 4);
    }

    #[test]
    fn osc_hyperlinks_are_stripped() {
        // OSC 8: ESC ] 8 ; ; url BEL text ESC ] 8 ; ; BEL
        let s = "\x1b]8;;http://example.com\x07click\x1b]8;;\x07";
        assert_eq!(visible_width(s), 5);
    }

    #[test]
    fn mixed_ascii_and_cjk() {
        assert_eq!(visible_width("hola 中"), 7); // 4 + 1 + 2
    }

    #[test]
    fn strip_ansi_preserves_content() {
        assert_eq!(strip_ansi("\x1b[31mabc\x1b[0m"), "abc");
        assert_eq!(strip_ansi("plain"), "plain");
    }
}
