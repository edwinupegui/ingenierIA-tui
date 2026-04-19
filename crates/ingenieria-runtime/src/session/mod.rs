//! Session System (E11): persistencia JSONL append-only + fork + export.
//!
//! Reemplaza el sistema viejo de `services/history.rs` (archivo JSON completo
//! reescrito en cada auto-save) por un modelo append-only crash-safe.
//!
//! - Cada mensaje se escribe inmediatamente como linea JSONL (append).
//! - Rotacion automatica a 256KB (ver `store::ROTATE_AFTER_BYTES`).
//! - Sidecar `.meta.json` para listar sesiones sin leer el JSONL completo.
//! - Fork preserva historial del padre y marca el punto de bifurcacion.
//!
//! Referencia: CLAW `rust/crates/runtime/src/session.rs` (1517 LOC).

pub mod entry;
pub mod forking;
pub mod meta;
pub mod store;

pub use entry::{SerializedToolCall, SessionEntry, TimedEntry};
pub use forking::{
    export_session, export_session_as, fork_session, fork_session_truncated, ExportFormat,
};
pub use meta::SessionMeta;
pub use store::{append_entry, list_metas, load_all_entries, meta_path};

// ForkInfo, active_path, delete_session, sessions_dir son publicos en sus
// modulos pero no se re-exportan desde aqui — se usan solo internamente o
// en tests.

/// Deriva un titulo corto desde el contenido de un mensaje (primera linea,
/// max 60 chars, respetando boundaries UTF-8).
pub fn title_from_content(content: &str) -> String {
    let first_line = content.lines().next().unwrap_or(content);
    let trimmed = first_line.trim();
    if trimmed.len() > 60 {
        let mut end = 60;
        while end > 0 && !trimmed.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}...", &trimmed[..end])
    } else {
        trimmed.to_string()
    }
}

/// Genera un id unico para una nueva sesion (timestamp hex).
pub fn generate_session_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ts = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis();
    format!("{ts:x}")
}

/// Re-export del lock compartido (ver `services::TEST_ENV_LOCK`).
#[cfg(test)]
#[cfg(test)]
pub(super) use crate::TEST_ENV_LOCK;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_truncates_at_sixty() {
        let long = "a".repeat(100);
        let title = title_from_content(&long);
        assert!(title.ends_with("..."));
        assert!(title.len() <= 63); // 60 + "..."
    }

    #[test]
    fn title_takes_only_first_line() {
        let content = "primera linea\nsegunda linea";
        assert_eq!(title_from_content(content), "primera linea");
    }

    #[test]
    fn title_respects_utf8_boundary() {
        // emoji ocupa 4 bytes
        let s = "🎉".repeat(20); // 80 bytes
        let title = title_from_content(&s);
        assert!(title.ends_with("..."));
        // No debe fallar por boundary invalido
        assert!(title.is_char_boundary(title.len() - 3));
    }

    #[test]
    fn generate_id_produces_hex() {
        let id = generate_session_id();
        assert!(id.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
