//! Store de memorias en `~/.config/ingenieria-tui/memory/`.
//!
//! Layout:
//! ```text
//! memory/
//! ├── MEMORY.md            ← indice auto-generado
//! ├── user_role.md
//! ├── feedback_testing.md
//! └── project_roadmap.md
//! ```
//!
//! El indice `MEMORY.md` se regenera cada vez que se guarda/borra una memoria.
//! Mantenerlo corto (<200 lineas) — se inyecta entero en el system prompt.

use std::path::{Path, PathBuf};

use super::parser::{parse_memory, serialize_memory};
use super::types::{MemoryEntry, MemoryFrontmatter};

const INDEX_FILENAME: &str = "MEMORY.md";
const MAX_INDEX_LINES: usize = 200;

/// `~/.config/ingenieria-tui/memory/`. `None` si no hay config dir.
pub fn memory_dir() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("ingenieria-tui").join("memory"))
}

/// Ruta de `MEMORY.md`.
pub fn index_path() -> Option<PathBuf> {
    memory_dir().map(|d| d.join(INDEX_FILENAME))
}

/// Asegura que el directorio existe.
fn ensure_dir() -> anyhow::Result<PathBuf> {
    let dir = memory_dir().ok_or_else(|| anyhow::anyhow!("sin config dir"))?;
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Valida y normaliza un filename — solo basename (sin `/`), extension `.md`.
fn validate_filename(filename: &str) -> anyhow::Result<String> {
    if filename.is_empty()
        || filename.contains('/')
        || filename.contains('\\')
        || filename.contains("..")
    {
        anyhow::bail!("filename invalido: '{filename}'");
    }
    if filename == INDEX_FILENAME {
        anyhow::bail!("'{INDEX_FILENAME}' es reservado para el indice");
    }
    if filename.ends_with(".md") {
        Ok(filename.to_string())
    } else {
        Ok(format!("{filename}.md"))
    }
}

/// Guarda/actualiza una memoria. Regenera el indice automaticamente.
pub fn save_memory(
    filename: &str,
    frontmatter: MemoryFrontmatter,
    body: &str,
) -> anyhow::Result<PathBuf> {
    let dir = ensure_dir()?;
    let fname = validate_filename(filename)?;
    let path = dir.join(&fname);
    let content = serialize_memory(&frontmatter, body);
    std::fs::write(&path, content)?;
    regenerate_index(&dir)?;
    Ok(path)
}

/// Carga una memoria por filename.
pub fn load_memory(filename: &str) -> anyhow::Result<MemoryEntry> {
    let dir = memory_dir().ok_or_else(|| anyhow::anyhow!("sin config dir"))?;
    let fname = validate_filename(filename)?;
    let path = dir.join(&fname);
    let raw = std::fs::read_to_string(&path)?;
    let (frontmatter, body) =
        parse_memory(&raw).map_err(|e| anyhow::anyhow!("parse {fname}: {e}"))?;
    Ok(MemoryEntry { filename: fname, frontmatter, body })
}

/// Borra una memoria + regenera el indice. Idempotente.
pub fn delete_memory(filename: &str) -> anyhow::Result<bool> {
    let dir = memory_dir().ok_or_else(|| anyhow::anyhow!("sin config dir"))?;
    let fname = validate_filename(filename)?;
    let path = dir.join(&fname);
    let removed = if path.exists() {
        std::fs::remove_file(&path)?;
        true
    } else {
        false
    };
    if removed && dir.exists() {
        regenerate_index(&dir)?;
    }
    Ok(removed)
}

/// Lista todas las memorias (parseadas). Ignora archivos corruptos.
pub fn list_memories() -> anyhow::Result<Vec<MemoryEntry>> {
    let Some(dir) = memory_dir() else {
        return Ok(Vec::new());
    };
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut entries = Vec::new();
    for item in std::fs::read_dir(&dir)? {
        let item = item?;
        let path = item.path();
        if !is_memory_file(&path) {
            continue;
        }
        let Ok(raw) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Ok((frontmatter, body)) = parse_memory(&raw) else {
            continue; // skip corruptos
        };
        let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
        entries.push(MemoryEntry { filename, frontmatter, body });
    }
    entries.sort_by(|a, b| a.filename.cmp(&b.filename));
    Ok(entries)
}

/// Regenera `MEMORY.md` como indice de todas las memorias validas.
pub fn regenerate_index(dir: &Path) -> anyhow::Result<()> {
    let entries = list_memories()?;
    let mut out = String::from("# Memory index\n\n");
    let mut line_count = 2;
    for entry in &entries {
        if line_count >= MAX_INDEX_LINES {
            out.push_str("\n<!-- truncated — demasiadas memorias -->\n");
            break;
        }
        out.push_str(&entry.index_line());
        out.push('\n');
        line_count += 1;
    }
    std::fs::write(dir.join(INDEX_FILENAME), out)?;
    Ok(())
}

/// Lee el indice completo (para inyectar en system prompt).
pub fn read_index() -> anyhow::Result<String> {
    let Some(path) = index_path() else {
        return Ok(String::new());
    };
    if !path.exists() {
        return Ok(String::new());
    }
    Ok(std::fs::read_to_string(path)?)
}

fn is_memory_file(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    name.ends_with(".md") && name != INDEX_FILENAME
}

#[cfg(test)]
mod tests {
    use super::super::types::{MemoryFrontmatter, MemoryType};
    use super::*;
    use crate::TEST_ENV_LOCK;

    fn setup_tmp_home() -> tempfile::TempDir {
        let tmp = tempfile::tempdir().unwrap();
        // En macOS `dirs::config_dir()` usa `$HOME/Library/Application Support`
        // ignorando XDG. Seteamos ambas para funcionar en macOS + Linux.
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", tmp.path());
            std::env::set_var("HOME", tmp.path());
        }
        tmp
    }

    fn sample_fm(name: &str, t: MemoryType) -> MemoryFrontmatter {
        MemoryFrontmatter { name: name.into(), description: "test".into(), memory_type: t }
    }

    #[test]
    fn save_and_load_roundtrip() {
        let _lock = TEST_ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let _tmp = setup_tmp_home();
        let fm = sample_fm("Role", MemoryType::User);
        save_memory("user_role", fm.clone(), "body text").unwrap();
        let loaded = load_memory("user_role").unwrap();
        assert_eq!(loaded.frontmatter, fm);
        assert_eq!(loaded.body, "body text");
    }

    #[test]
    fn list_sorted_and_skips_index() {
        let _lock = TEST_ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let _tmp = setup_tmp_home();
        save_memory("z_one", sample_fm("Z", MemoryType::User), "b").unwrap();
        save_memory("a_two", sample_fm("A", MemoryType::Feedback), "b").unwrap();
        let entries = list_memories().unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].filename, "a_two.md");
        assert_eq!(entries[1].filename, "z_one.md");
    }

    #[test]
    fn index_regenerated_on_save() {
        let _lock = TEST_ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let _tmp = setup_tmp_home();
        save_memory("x", sample_fm("Xfile", MemoryType::Project), "b").unwrap();
        let idx = read_index().unwrap();
        assert!(idx.contains("Xfile"));
        assert!(idx.contains("x.md"));
    }

    #[test]
    fn delete_removes_and_updates_index() {
        let _lock = TEST_ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let _tmp = setup_tmp_home();
        save_memory("y", sample_fm("Yfile", MemoryType::Reference), "b").unwrap();
        let removed = delete_memory("y").unwrap();
        assert!(removed);
        assert!(load_memory("y").is_err());
        let idx = read_index().unwrap();
        assert!(!idx.contains("Yfile"));
    }

    #[test]
    fn filename_traversal_rejected() {
        let _lock = TEST_ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let _tmp = setup_tmp_home();
        let fm = sample_fm("X", MemoryType::User);
        assert!(save_memory("../evil", fm.clone(), "b").is_err());
        assert!(save_memory("a/b", fm.clone(), "b").is_err());
        assert!(save_memory("MEMORY.md", fm, "b").is_err());
    }

    #[test]
    fn delete_nonexistent_is_idempotent() {
        let _lock = TEST_ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let _tmp = setup_tmp_home();
        assert!(!delete_memory("ghost").unwrap());
    }
}
