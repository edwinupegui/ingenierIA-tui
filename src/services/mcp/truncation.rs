//! Truncation de respuestas MCP oversized.
//!
//! Los tool results pueden ser enormes (logs, listados, JSON grandes) y saturar
//! el contexto del modelo. `truncate_if_oversized` recorta preservando inicio
//! y final con un marcador del tamano original.

/// Umbral default: mas alla de 10KB se trunca.
pub const DEFAULT_MAX_BYTES: usize = 10 * 1024;

/// Fraccion del budget que va al inicio (resto al final).
const HEAD_RATIO: f64 = 0.6;

/// Si `content` excede `max_bytes` lo recorta preservando inicio y cola, con
/// un marcador `[...TRUNCATED N bytes...]` en el medio. Si cabe, se devuelve
/// sin cambios. Respeta boundaries UTF-8.
pub fn truncate_if_oversized(content: &str, max_bytes: usize) -> String {
    if content.len() <= max_bytes {
        return content.to_string();
    }
    if max_bytes < 100 {
        // Budget demasiado pequeno — devolver cola simple
        return format!("[...truncated to {max_bytes} bytes]");
    }

    let marker_template = "\n\n[...TRUNCATED {} bytes...]\n\n";
    let truncated_bytes = content.len().saturating_sub(max_bytes);
    let marker = marker_template.replace("{}", &truncated_bytes.to_string());

    let usable = max_bytes.saturating_sub(marker.len());
    let head_len = safe_boundary(content, (usable as f64 * HEAD_RATIO) as usize);
    let tail_len = usable.saturating_sub(head_len);
    let tail_start = safe_boundary_from_end(content, tail_len);

    let mut out = String::with_capacity(max_bytes);
    out.push_str(&content[..head_len]);
    out.push_str(&marker);
    out.push_str(&content[tail_start..]);
    out
}

/// Ajusta `target` hacia abajo hasta llegar a un char boundary UTF-8.
fn safe_boundary(s: &str, target: usize) -> usize {
    let mut t = target.min(s.len());
    while t > 0 && !s.is_char_boundary(t) {
        t -= 1;
    }
    t
}

/// Offset desde el inicio tal que la cola tenga `tail_len` bytes
/// (aproximadamente, ajustando a boundary UTF-8).
fn safe_boundary_from_end(s: &str, tail_len: usize) -> usize {
    let target = s.len().saturating_sub(tail_len);
    let mut t = target;
    while t < s.len() && !s.is_char_boundary(t) {
        t += 1;
    }
    t
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_content_unchanged() {
        let s = "hola mundo";
        assert_eq!(truncate_if_oversized(s, DEFAULT_MAX_BYTES), s);
    }

    #[test]
    fn oversized_gets_truncated_with_marker() {
        let s = "a".repeat(20_000);
        let out = truncate_if_oversized(&s, DEFAULT_MAX_BYTES);
        assert!(out.len() <= DEFAULT_MAX_BYTES + 50); // margen del marker
        assert!(out.contains("TRUNCATED"));
    }

    #[test]
    fn preserves_head_and_tail() {
        let s = format!("START{}END", "x".repeat(20_000));
        let out = truncate_if_oversized(&s, DEFAULT_MAX_BYTES);
        assert!(out.starts_with("START"));
        assert!(out.ends_with("END"));
    }

    #[test]
    fn respects_utf8_boundary() {
        // Construir string que pase el limite con emoji cerca del boundary
        let s = format!("🎉{}🎊", "x".repeat(20_000));
        let out = truncate_if_oversized(&s, 500);
        // Si rompe boundary, este to_string fallaria. Como String valida UTF-8
        // con String::from_utf8, el simple hecho de que `out` sea String prueba
        // que es UTF-8 valido. Hacer una verificacion explicita:
        assert!(out.chars().count() > 0);
    }

    #[test]
    fn tiny_budget_returns_placeholder() {
        let s = "a".repeat(1000);
        let out = truncate_if_oversized(&s, 50);
        assert!(out.contains("truncated"));
    }
}
