//! Slash commands para el memory system (E15): `/remember`, `/forget`.
//!
//! Sintaxis:
//! - `/remember <type> <filename>: <body>`
//!   ej: `/remember user role_rust: senior rust engineer, prefers concise code`
//!   tipos validos: `user`, `feedback`, `project`, `reference`
//! - `/forget <filename>`

use crate::app::App;
use crate::services::memory::{self, MemoryFrontmatter, MemoryType};

impl App {
    /// `/remember <type> <filename>: <body>`
    pub(crate) fn handle_remember_command(&mut self, arg: &str) {
        let arg = arg.trim();
        if arg.is_empty() {
            self.notify(
                "Uso: /remember <type> <filename>: <body>. Tipos: user, feedback, project, reference"
                    .to_string(),
            );
            return;
        }
        match parse_remember_args(arg) {
            Ok((mem_type, filename, body)) => self.save_remembered(mem_type, &filename, &body),
            Err(msg) => self.notify(format!("✗ /remember: {msg}")),
        }
    }

    /// `/forget <filename>`
    pub(crate) fn handle_forget_command(&mut self, arg: &str) {
        let name = arg.trim();
        if name.is_empty() {
            self.notify("Uso: /forget <filename>".to_string());
            return;
        }
        match memory::delete_memory(name) {
            Ok(true) => self.notify(format!("✓ Memoria '{name}' eliminada")),
            Ok(false) => self.notify(format!("Memoria '{name}' no existe")),
            Err(e) => self.notify(format!("✗ Error al borrar memoria: {e}")),
        }
    }

    fn save_remembered(&mut self, mem_type: MemoryType, filename: &str, body: &str) {
        let name = title_from_body(body);
        let description = short_desc_from_body(body);
        let fm = MemoryFrontmatter { name, description, memory_type: mem_type };
        match memory::save_memory(filename, fm, body) {
            Ok(path) => {
                let shown = path.file_name().and_then(|n| n.to_str()).unwrap_or(filename);
                self.notify(format!("✓ Memoria guardada: {shown}"));
            }
            Err(e) => self.notify(format!("✗ Error al guardar memoria: {e}")),
        }
    }
}

/// Parsea `<type> <filename>: <body>` → `(MemoryType, filename, body)`.
fn parse_remember_args(raw: &str) -> anyhow::Result<(MemoryType, String, String)> {
    let (head, body) =
        raw.split_once(':').ok_or_else(|| anyhow::anyhow!("falta separador ':' antes del body"))?;
    let body = body.trim();
    if body.is_empty() {
        anyhow::bail!("body no puede estar vacio");
    }
    let mut parts = head.split_whitespace();
    let type_raw = parts.next().ok_or_else(|| anyhow::anyhow!("falta <type>"))?;
    let filename = parts.next().ok_or_else(|| anyhow::anyhow!("falta <filename>"))?;
    let mem_type = MemoryType::from_label(type_raw).ok_or_else(|| {
        anyhow::anyhow!("tipo invalido '{type_raw}' (user|feedback|project|reference)")
    })?;
    Ok((mem_type, filename.to_string(), body.to_string()))
}

fn title_from_body(body: &str) -> String {
    let first_line = body.lines().next().unwrap_or(body).trim();
    let mut end = first_line.len().min(60);
    while end > 0 && !first_line.is_char_boundary(end) {
        end -= 1;
    }
    let base = &first_line[..end];
    if base.is_empty() {
        "memory".to_string()
    } else {
        base.to_string()
    }
}

fn short_desc_from_body(body: &str) -> String {
    let compact: String = body.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut end = compact.len().min(100);
    while end > 0 && !compact.is_char_boundary(end) {
        end -= 1;
    }
    compact[..end].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_remember_happy_path() {
        let (t, f, b) = parse_remember_args("user role_rust: senior rust engineer").unwrap();
        assert_eq!(t, MemoryType::User);
        assert_eq!(f, "role_rust");
        assert_eq!(b, "senior rust engineer");
    }

    #[test]
    fn parse_remember_missing_colon() {
        assert!(parse_remember_args("user role_rust senior").is_err());
    }

    #[test]
    fn parse_remember_invalid_type() {
        assert!(parse_remember_args("wrong name: body").is_err());
    }

    #[test]
    fn parse_remember_empty_body() {
        assert!(parse_remember_args("user name: ").is_err());
    }

    #[test]
    fn title_from_body_truncates() {
        let long = "a".repeat(100);
        let t = title_from_body(&long);
        assert!(t.len() <= 60);
    }

    #[test]
    fn title_respects_utf8() {
        let s = "🎉".repeat(20); // 80 bytes
        let t = title_from_body(&s);
        assert!(t.is_char_boundary(t.len()));
    }
}
