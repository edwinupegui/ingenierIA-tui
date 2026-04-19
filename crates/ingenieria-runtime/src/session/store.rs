//! `JsonlSessionStore`: persistencia append-only crash-safe.
//!
//! - Append por linea → cada mensaje sobrevive crashes a mitad de escritura.
//! - Rotacion a 256KB → previene archivos monstruosos.
//! - Sidecar `.meta.json` → listar sesiones sin parsear JSONL completo.
//!
//! Layout en disco (`~/.config/ingenieria-tui/sessions/`):
//! ```text
//! <id>.jsonl           ← archivo actual (append activo)
//! <id>.part1.jsonl     ← archivo rotado mas viejo
//! <id>.part2.jsonl     ← segunda rotacion
//! <id>.meta.json       ← metadata sidecar (title, stats, fork info)
//! ```

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use super::entry::{SessionEntry, TimedEntry};
use super::meta::SessionMeta;

/// Umbral de rotacion (256 KB). Cuando el `.jsonl` activo supera este tamano
/// se rota a `<id>.partN.jsonl` y se empieza uno fresco.
pub const ROTATE_AFTER_BYTES: u64 = 256 * 1024;

/// Devuelve `~/.config/ingenieria-tui/sessions/`. `None` si no hay config dir.
pub fn sessions_dir() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("ingenieria-tui").join("sessions"))
}

/// Ruta del archivo activo `<id>.jsonl`.
pub fn active_path(id: &str) -> Option<PathBuf> {
    sessions_dir().map(|d| d.join(format!("{id}.jsonl")))
}

/// Ruta del archivo de metadata `<id>.meta.json`.
pub fn meta_path(id: &str) -> Option<PathBuf> {
    sessions_dir().map(|d| d.join(format!("{id}.meta.json")))
}

/// Ruta de la parte rotada numero `n`.
fn part_path(id: &str, n: u32) -> Option<PathBuf> {
    sessions_dir().map(|d| d.join(format!("{id}.part{n}.jsonl")))
}

/// Append de una entrada al `<id>.jsonl`. Crea el archivo/dir si no existe.
/// Rota si supera `ROTATE_AFTER_BYTES`.
pub fn append_entry(id: &str, entry: &TimedEntry) -> anyhow::Result<()> {
    let path = active_path(id).ok_or_else(|| anyhow::anyhow!("No config dir"))?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    rotate_if_needed(id, &path)?;

    let mut file = std::fs::OpenOptions::new().create(true).append(true).open(&path)?;
    let line = serde_json::to_string(entry)?;
    file.write_all(line.as_bytes())?;
    file.write_all(b"\n")?;
    file.flush()?;
    Ok(())
}

/// Si el archivo activo excede el umbral, lo renombra a la siguiente parte
/// libre (`<id>.part1.jsonl`, `<id>.part2.jsonl`, ...).
fn rotate_if_needed(id: &str, active: &Path) -> anyhow::Result<()> {
    let Ok(metadata) = std::fs::metadata(active) else {
        return Ok(()); // no existe aun
    };
    if metadata.len() <= ROTATE_AFTER_BYTES {
        return Ok(());
    }
    let next_n = next_free_part_number(id);
    let dest =
        part_path(id, next_n).ok_or_else(|| anyhow::anyhow!("No config dir for rotation"))?;
    std::fs::rename(active, dest)?;
    Ok(())
}

/// Busca el siguiente N libre para `<id>.partN.jsonl`.
fn next_free_part_number(id: &str) -> u32 {
    let mut n = 1u32;
    loop {
        match part_path(id, n) {
            Some(p) if p.exists() => n += 1,
            _ => return n,
        }
    }
}

/// Carga todas las entradas de una sesion (partes rotadas + activo) en orden.
/// Lineas corruptas se saltan con warning (log).
pub fn load_all_entries(id: &str) -> Vec<TimedEntry> {
    let mut out = Vec::new();
    // Partes rotadas en orden cronologico (part1, part2, ...).
    let mut n = 1u32;
    while let Some(p) = part_path(id, n) {
        if !p.exists() {
            break;
        }
        append_from_file(&p, &mut out);
        n += 1;
    }
    // Archivo activo.
    if let Some(p) = active_path(id) {
        if p.exists() {
            append_from_file(&p, &mut out);
        }
    }
    out
}

fn append_from_file(path: &Path, out: &mut Vec<TimedEntry>) {
    let Ok(file) = std::fs::File::open(path) else {
        return;
    };
    let reader = BufReader::new(file);
    for (i, line) in reader.lines().enumerate() {
        let Ok(line) = line else { continue };
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<TimedEntry>(&line) {
            Ok(entry) => out.push(entry),
            Err(e) => tracing::warn!(file = %path.display(), line = i + 1, error = %e,
                "skipping corrupt JSONL line"),
        }
    }
}

/// Lista todos los metas `.meta.json` en el dir de sesiones (newest first).
/// Si una sesion solo tiene `.jsonl` sin meta, se reconstruye on-the-fly.
pub fn list_metas() -> Vec<SessionMeta> {
    let Some(dir) = sessions_dir() else {
        return Vec::new();
    };
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };

    let mut seen_ids: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut metas: Vec<SessionMeta> = Vec::new();

    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if let Some(id) = name.strip_suffix(".meta.json") {
            if seen_ids.insert(id.to_string()) {
                if let Some(m) = SessionMeta::load(&path) {
                    metas.push(m);
                }
            }
        }
    }

    // Pasada 2: sesiones con .jsonl sin meta.
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if let Some(id) = name.strip_suffix(".jsonl") {
                // Saltar partes rotadas: "<id>.partN"
                if id.contains(".part") {
                    continue;
                }
                if seen_ids.insert(id.to_string()) {
                    if let Some(m) = reconstruct_meta(id) {
                        metas.push(m);
                    }
                }
            }
        }
    }

    metas.sort_by(|a, b| {
        b.updated_at.cmp(&a.updated_at).then_with(|| b.created_at.cmp(&a.created_at))
    });
    metas
}

/// Reconstruye un `SessionMeta` minimo a partir del JSONL (cuando falta meta sidecar).
fn reconstruct_meta(id: &str) -> Option<SessionMeta> {
    let entries = load_all_entries(id);
    if entries.is_empty() {
        return None;
    }
    let title = entries
        .iter()
        .find_map(|e| match &e.entry {
            SessionEntry::UserMessage { content } => Some(super::title_from_content(content)),
            _ => None,
        })
        .unwrap_or_else(|| "Sin titulo".to_string());

    let created_at = entries.first().map(|e| e.timestamp.clone()).unwrap_or_default();
    let updated_at = entries.last().map(|e| e.timestamp.clone()).unwrap_or_default();

    let mut meta = SessionMeta::new(id.to_string(), title, "?".into(), "?".into());
    meta.created_at = created_at;
    meta.updated_at = updated_at;
    meta.message_count = entries
        .iter()
        .filter(|e| {
            matches!(
                e.entry,
                SessionEntry::UserMessage { .. }
                    | SessionEntry::AssistantMessage { .. }
                    | SessionEntry::SystemMessage { .. }
            )
        })
        .count();
    meta.turn_count =
        entries.iter().filter(|e| matches!(e.entry, SessionEntry::UserMessage { .. })).count();

    // Si el ultimo snapshot tiene stats, usarlo.
    if let Some(last_snap) = entries.iter().rev().find_map(|e| match &e.entry {
        SessionEntry::MetaSnapshot {
            total_input_tokens,
            total_output_tokens,
            total_cost,
            mode,
            ..
        } => Some((*total_input_tokens, *total_output_tokens, *total_cost, mode.clone())),
        _ => None,
    }) {
        meta.total_input_tokens = last_snap.0;
        meta.total_output_tokens = last_snap.1;
        meta.total_cost = last_snap.2;
        meta.mode = last_snap.3;
    }

    Some(meta)
}

/// Elimina todos los archivos de una sesion (jsonl, partes, meta).
#[cfg_attr(not(test), allow(dead_code, reason = "reservado para UI de borrado de historial"))]
pub fn delete_session(id: &str) -> anyhow::Result<()> {
    let Some(dir) = sessions_dir() else {
        return Ok(());
    };
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Ok(());
    };
    let prefix = format!("{id}.");
    for entry in entries.filter_map(|e| e.ok()) {
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        if name.starts_with(&prefix) {
            let _ = std::fs::remove_file(entry.path());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_tmp_home() -> (tempfile::TempDir, std::sync::MutexGuard<'static, ()>) {
        let guard = crate::TEST_ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = tempfile::tempdir().unwrap();
        // En macOS dirs::config_dir() usa $HOME/Library/Application Support o
        // $XDG_CONFIG_HOME. Preferimos XDG que funciona en ambos.
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", tmp.path());
            std::env::set_var("HOME", tmp.path());
        }
        (tmp, guard)
    }

    #[test]
    fn append_and_load_round_trip() {
        let (_tmp, _g) = setup_tmp_home();
        let id = format!("test-append-{}", std::process::id());
        let e1 = TimedEntry::with_timestamp(
            "2026-04-13T10:00:00Z".into(),
            SessionEntry::UserMessage { content: "primer".into() },
        );
        let e2 = TimedEntry::with_timestamp(
            "2026-04-13T10:00:01Z".into(),
            SessionEntry::AssistantMessage { content: "segundo".into(), tool_calls: vec![] },
        );
        append_entry(&id, &e1).unwrap();
        append_entry(&id, &e2).unwrap();
        let all = load_all_entries(&id);
        assert_eq!(all.len(), 2);
        let _ = delete_session(&id);
    }

    #[test]
    fn corrupt_line_is_skipped() {
        let (_tmp, _g) = setup_tmp_home();
        let id = format!("test-corrupt-{}", std::process::id());
        let e = TimedEntry::with_timestamp(
            "t".into(),
            SessionEntry::UserMessage { content: "ok".into() },
        );
        append_entry(&id, &e).unwrap();
        // Corromper: append raw bad JSON.
        let path = active_path(&id).unwrap();
        let mut f = std::fs::OpenOptions::new().append(true).open(&path).unwrap();
        writeln!(f, "{{not json").unwrap();
        append_entry(&id, &e).unwrap();
        let all = load_all_entries(&id);
        assert_eq!(all.len(), 2, "corrupt line should be skipped");
        let _ = delete_session(&id);
    }

    #[test]
    fn rotation_moves_to_part1_when_exceeds_threshold() {
        let (_tmp, _g) = setup_tmp_home();
        let id = format!("test-rotate-{}", std::process::id());
        // Escribir bytes crudos mas alla del threshold.
        let path = active_path(&id).unwrap();
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let big = "x".repeat((ROTATE_AFTER_BYTES + 100) as usize);
        std::fs::write(&path, big).unwrap();
        // El siguiente append debe rotar.
        let e = TimedEntry::with_timestamp(
            "t".into(),
            SessionEntry::UserMessage { content: "post-rotate".into() },
        );
        append_entry(&id, &e).unwrap();
        assert!(part_path(&id, 1).unwrap().exists(), "part1 should exist after rotation");
        let active_len = std::fs::metadata(&path).unwrap().len();
        assert!(active_len < ROTATE_AFTER_BYTES, "active file should be fresh after rotation");
        let _ = delete_session(&id);
    }

    #[test]
    fn list_metas_includes_reconstructed_when_sidecar_missing() {
        let (_tmp, _g) = setup_tmp_home();
        let id = format!("test-reconstruct-{}", std::process::id());
        let e = TimedEntry::with_timestamp(
            "2026-04-13T10:00:00Z".into(),
            SessionEntry::UserMessage { content: "hola mundo".into() },
        );
        append_entry(&id, &e).unwrap();
        // No guardamos meta; list_metas debe reconstruirlo.
        let metas = list_metas();
        assert!(metas.iter().any(|m| m.id == id));
        let _ = delete_session(&id);
    }

    #[test]
    fn delete_session_removes_all_related_files() {
        let (_tmp, _g) = setup_tmp_home();
        let id = format!("test-delete-{}", std::process::id());
        let e = TimedEntry::with_timestamp(
            "t".into(),
            SessionEntry::UserMessage { content: "x".into() },
        );
        append_entry(&id, &e).unwrap();
        let meta = SessionMeta::new(id.clone(), "t".into(), "f".into(), "m".into());
        meta.save(&meta_path(&id).unwrap()).unwrap();

        assert!(active_path(&id).unwrap().exists());
        assert!(meta_path(&id).unwrap().exists());
        delete_session(&id).unwrap();
        assert!(!active_path(&id).unwrap().exists());
        assert!(!meta_path(&id).unwrap().exists());
    }
}
