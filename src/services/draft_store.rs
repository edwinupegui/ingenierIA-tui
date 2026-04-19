//! Draft persistence (E40).
//!
//! Guarda el texto actual del input de chat en `~/.config/ingenieria-tui/drafts/`
//! para que sobreviva cierres/crashes. Cada sesion (`session_id`) tiene su
//! propio archivo `<session_id>.txt`. Al abrir chat se intenta cargar y si
//! existe, se restaura silenciosamente con un toast.
//!
//! Anti-pattern evitado: NO usar arboard/locks aqui. Este es I/O sincrono
//! trivial (< 10KB) y se ejecuta en el handler de tick, fuera del hot path
//! de render.

use std::fs;
use std::io;
use std::path::PathBuf;

use anyhow::Result;

const DRAFT_EXT: &str = "txt";

/// Path al directorio de drafts. Usa XDG config dir.
/// Resuelve como `$XDG_CONFIG_HOME/ingenieria-tui/drafts/` o
/// `~/.config/ingenieria-tui/drafts/` en su defecto.
pub fn drafts_dir() -> Option<PathBuf> {
    let base = if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        PathBuf::from(xdg)
    } else {
        let home = std::env::var("HOME").ok()?;
        PathBuf::from(home).join(".config")
    };
    Some(base.join("ingenieria-tui").join("drafts"))
}

/// Path al archivo de draft de una sesion.
pub fn draft_path_for(session_id: &str) -> Option<PathBuf> {
    let dir = drafts_dir()?;
    Some(dir.join(format!("{session_id}.{DRAFT_EXT}")))
}

/// Guarda el draft de una sesion. Si `content` esta vacio, borra el archivo
/// (no queremos dejar artefactos para sesiones sin draft). Escribe via
/// archivo temporal + rename para evitar corrupcion parcial en crashes.
pub fn save_draft(session_id: &str, content: &str) -> Result<()> {
    let path = draft_path_for(session_id)
        .ok_or_else(|| anyhow::anyhow!("no se pudo resolver drafts_dir (HOME?)"))?;

    if content.is_empty() {
        // Limpieza: eliminamos archivos huerfanos cuando el input queda vacio.
        match fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    } else {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let tmp = path.with_extension(format!("{DRAFT_EXT}.tmp"));
        fs::write(&tmp, content)?;
        fs::rename(&tmp, &path)?;
        Ok(())
    }
}

/// Carga el draft de una sesion. Retorna `None` si no existe (caso comun,
/// no es error).
pub fn load_draft(session_id: &str) -> Option<String> {
    let path = draft_path_for(session_id)?;
    fs::read_to_string(&path).ok()
}

/// Elimina el archivo de draft tras envio exitoso.
pub fn clear_draft(session_id: &str) -> Result<()> {
    let Some(path) = draft_path_for(session_id) else {
        return Ok(());
    };
    match fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::sync::Mutex;

    // Serializa los tests porque manipulan `XDG_CONFIG_HOME`.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn with_temp_xdg<R>(body: impl FnOnce(&std::path::Path) -> R) -> R {
        let _guard = ENV_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let original = env::var("XDG_CONFIG_HOME").ok();
        env::set_var("XDG_CONFIG_HOME", tmp.path());
        let result = body(tmp.path());
        match original {
            Some(v) => env::set_var("XDG_CONFIG_HOME", v),
            None => env::remove_var("XDG_CONFIG_HOME"),
        }
        result
    }

    #[test]
    fn save_and_load_roundtrip() {
        with_temp_xdg(|_| {
            save_draft("sess-1", "hola mundo").unwrap();
            assert_eq!(load_draft("sess-1").as_deref(), Some("hola mundo"));
        });
    }

    #[test]
    fn save_empty_removes_file() {
        with_temp_xdg(|_| {
            save_draft("sess-2", "data").unwrap();
            assert!(load_draft("sess-2").is_some());
            save_draft("sess-2", "").unwrap();
            assert!(load_draft("sess-2").is_none());
        });
    }

    #[test]
    fn load_missing_returns_none() {
        with_temp_xdg(|_| {
            assert!(load_draft("nonexistent").is_none());
        });
    }

    #[test]
    fn clear_is_idempotent() {
        with_temp_xdg(|_| {
            clear_draft("missing").unwrap();
            save_draft("sess-3", "x").unwrap();
            clear_draft("sess-3").unwrap();
            clear_draft("sess-3").unwrap();
            assert!(load_draft("sess-3").is_none());
        });
    }
}
