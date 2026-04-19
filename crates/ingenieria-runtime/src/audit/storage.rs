//! Storage del audit log: JSONL rotativo por dia.
//!
//! Layout: `~/.config/ingenieria-tui/audit/YYYY-MM-DD.jsonl` — un archivo por
//! dia. Entries append-only. Los archivos viejos (>30 dias) se purgan
//! on-demand via `prune_old()`.

use std::io::Write;
use std::path::PathBuf;

/// Dias antes de purgar audit logs viejos (usado por `prune_old`).
#[allow(dead_code, reason = "reservado para /audit --prune futura")]
const MAX_LOG_AGE_DAYS: u64 = 30;

/// Directorio del audit log.
pub fn audit_dir() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("ingenieria-tui").join("audit"))
}

/// Ruta del archivo del dia (YYYY-MM-DD).
pub fn today_path() -> Option<PathBuf> {
    let today = today_date();
    audit_dir().map(|d| d.join(format!("{today}.jsonl")))
}

fn today_date() -> String {
    // Usamos el ISO 8601 y cortamos a los primeros 10 chars (YYYY-MM-DD).
    let iso = ingenieria_domain::time::now_iso();
    iso.get(..10).unwrap_or(&iso).to_string()
}

/// Append atomico de una linea al log del dia. Crea dir si falta.
pub fn append_line(line: &str) -> anyhow::Result<()> {
    let path = today_path().ok_or_else(|| anyhow::anyhow!("no config dir"))?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = std::fs::OpenOptions::new().create(true).append(true).open(&path)?;
    file.write_all(line.as_bytes())?;
    if !line.ends_with('\n') {
        file.write_all(b"\n")?;
    }
    file.flush()?;
    Ok(())
}

/// Lista archivos del audit log (newest first).
pub fn list_log_files() -> Vec<PathBuf> {
    let Some(dir) = audit_dir() else {
        return Vec::new();
    };
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut files: Vec<PathBuf> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "jsonl"))
        .collect();
    files.sort_by(|a, b| b.file_name().cmp(&a.file_name()));
    files
}

/// Purga archivos con mtime > MAX_LOG_AGE_DAYS dias.
#[allow(dead_code, reason = "utilidad para /audit --prune futura")]
pub fn prune_old() {
    let Some(dir) = audit_dir() else {
        return;
    };
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return;
    };
    let now = std::time::SystemTime::now();
    let max_age = std::time::Duration::from_secs(MAX_LOG_AGE_DAYS * 24 * 60 * 60);
    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.extension().is_none_or(|ext| ext != "jsonl") {
            continue;
        }
        let too_old = entry
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|mtime| now.duration_since(mtime).ok())
            .is_some_and(|age| age > max_age);
        if too_old {
            let _ = std::fs::remove_file(&path);
        }
    }
}

/// Exporta todos los logs a un JSON array en `dest`.
pub fn export_to(dest: &PathBuf) -> anyhow::Result<usize> {
    let mut all_lines: Vec<serde_json::Value> = Vec::new();
    for path in list_log_files() {
        let Ok(content) = std::fs::read_to_string(&path) else { continue };
        for line in content.lines() {
            if line.trim().is_empty() {
                continue;
            }
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(line) {
                all_lines.push(val);
            }
        }
    }
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(&all_lines)?;
    std::fs::write(dest, json)?;
    Ok(all_lines.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn today_date_is_ten_chars() {
        let d = today_date();
        assert_eq!(d.len(), 10);
        assert_eq!(&d[4..5], "-");
        assert_eq!(&d[7..8], "-");
    }

    fn setup_tmp_home() -> (tempfile::TempDir, std::sync::MutexGuard<'static, ()>) {
        let guard = crate::TEST_ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = tempfile::tempdir().unwrap();
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", tmp.path());
            std::env::set_var("HOME", tmp.path());
        }
        (tmp, guard)
    }

    #[test]
    fn append_line_writes_file() {
        let (_tmp, _g) = setup_tmp_home();
        append_line(r#"{"x":1}"#).unwrap();
        let files = list_log_files();
        assert!(!files.is_empty());
        let content = std::fs::read_to_string(&files[0]).unwrap();
        assert!(content.contains("\"x\":1"));
    }

    #[test]
    fn export_produces_json_array() {
        let (tmp, _g) = setup_tmp_home();
        append_line(r#"{"a":1}"#).unwrap();
        append_line(r#"{"b":2}"#).unwrap();
        let dest = tmp.path().join("export.json");
        let count = export_to(&dest).unwrap();
        assert_eq!(count, 2);
        let content = std::fs::read_to_string(&dest).unwrap();
        assert!(content.starts_with('['));
        assert!(content.contains("\"a\""));
        assert!(content.contains("\"b\""));
    }
}
