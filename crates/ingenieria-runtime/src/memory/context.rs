//! Construccion del bloque de memoria para inyectar en el system prompt.
//!
//! Formato:
//! ```text
//! # Persistent memory
//!
//! Contents of <path>/MEMORY.md (auto-memory index):
//!
//! - [Title](file.md) — description
//! - ...
//! ```
//!
//! Se inyecta al inicio de cada turno en el system prompt. Si no hay
//! memorias (o no se puede leer), retorna `None`.

use super::store::{index_path, memory_dir, read_index};

/// Maximo de caracteres del bloque de memoria (evitar blowup de contexto).
const MAX_CHARS: usize = 8 * 1024;

/// Construye el bloque de contexto persistente. `None` si no hay memorias o
/// el indice esta vacio.
pub fn build_memory_context() -> Option<String> {
    let index = read_index().ok()?;
    let trimmed = index.trim();
    if trimmed.is_empty() || !has_memory_entries(trimmed) {
        return None;
    }
    let path_display = index_path()
        .and_then(|p| p.to_str().map(|s| s.to_string()))
        .unwrap_or_else(|| "MEMORY.md".to_string());

    let mut out = String::with_capacity(trimmed.len() + 200);
    out.push_str("# Persistent memory\n\n");
    out.push_str(&format!(
        "Contents of {path_display} (auto-memory index, updated on every save):\n\n"
    ));
    out.push_str(trimmed);
    out.push('\n');

    if out.len() > MAX_CHARS {
        truncate_at_boundary(&mut out, MAX_CHARS);
        out.push_str("\n<!-- truncated -->\n");
    }
    Some(out)
}

/// Heuristica: el indice tiene al menos una linea `- [...]`.
fn has_memory_entries(index: &str) -> bool {
    index.lines().any(|l| l.trim_start().starts_with("- ["))
}

/// Trunca a `max` bytes respetando boundary UTF-8.
fn truncate_at_boundary(s: &mut String, max: usize) {
    if s.len() <= max {
        return;
    }
    let mut end = max;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    s.truncate(end);
}

/// Retorna el path del directorio de memoria si existe (para `/memories`).
pub fn memory_dir_display() -> Option<String> {
    memory_dir().and_then(|p| p.to_str().map(|s| s.to_string()))
}

#[cfg(test)]
mod tests {
    use super::super::store::save_memory;
    use super::super::types::{MemoryFrontmatter, MemoryType};
    use super::*;
    use crate::TEST_ENV_LOCK;

    fn setup_tmp_home() -> tempfile::TempDir {
        let tmp = tempfile::tempdir().unwrap();
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", tmp.path());
            std::env::set_var("HOME", tmp.path());
        }
        tmp
    }

    #[test]
    fn context_none_when_empty() {
        let _lock = TEST_ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let _tmp = setup_tmp_home();
        assert!(build_memory_context().is_none());
    }

    #[test]
    fn context_contains_entries() {
        let _lock = TEST_ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let _tmp = setup_tmp_home();
        let fm = MemoryFrontmatter {
            name: "RoleX".into(),
            description: "desc".into(),
            memory_type: MemoryType::User,
        };
        save_memory("role_x", fm, "body").unwrap();
        let ctx = build_memory_context().unwrap();
        assert!(ctx.contains("# Persistent memory"));
        assert!(ctx.contains("RoleX"));
        assert!(ctx.contains("role_x.md"));
    }

    #[test]
    fn truncate_respects_utf8() {
        let mut s = "🎉".repeat(100); // 400 bytes
        truncate_at_boundary(&mut s, 10);
        assert!(s.is_char_boundary(s.len()));
        assert!(s.len() <= 10);
    }
}
