//! Glob tool: busca archivos por patrón glob, ordenados por mtime descendente.
//!
//! Respeta el sandbox del cwd: paths resueltos fuera del proyecto se filtran.
//! Oculta dotfiles (paths que comienzan con `.` en cualquier segmento).
use std::path::PathBuf;
use std::time::SystemTime;

use serde::Deserialize;

use super::{Tool, ToolPermission};
use crate::services::chat::ToolDefinition;

const MAX_RESULTS: usize = 200;

pub struct GlobTool;

#[derive(Deserialize)]
struct GlobArgs {
    pattern: String,
    #[serde(default)]
    path: Option<String>,
}

#[async_trait::async_trait]
impl Tool for GlobTool {
    fn name(&self) -> &str {
        "glob"
    }

    fn permission(&self) -> ToolPermission {
        ToolPermission::Safe
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            json: serde_json::json!({
                "type": "function",
                "function": {
                    "name": "glob",
                    "description": "Busca archivos por patrón glob (ej: `src/**/*.rs`, `*.toml`, `**/test_*.py`). Devuelve paths ordenados por fecha de modificación descendente. Máximo 200 resultados.",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "pattern": { "type": "string", "description": "Patrón glob" },
                            "path": { "type": "string", "description": "Directorio base (default: cwd)" }
                        },
                        "required": ["pattern"]
                    }
                }
            }),
        }
    }

    async fn execute(&self, arguments: &str) -> String {
        let args: GlobArgs = match serde_json::from_str(arguments) {
            Ok(a) => a,
            Err(e) => return format!("Error parsing arguments: {e}"),
        };
        glob_search(&args.pattern, args.path.as_deref())
    }
}

fn glob_search(pattern: &str, base: Option<&str>) -> String {
    let cwd = match std::env::current_dir() {
        Ok(d) => d,
        Err(e) => return format!("Error: cannot determine working directory: {e}"),
    };

    let full_pattern = match base {
        Some(b) => format!("{}/{pattern}", b.trim_end_matches('/')),
        None => pattern.to_string(),
    };

    let paths = match glob::glob(&full_pattern) {
        Ok(p) => p,
        Err(e) => return format!("Error: patrón inválido: {e}"),
    };

    let mut matches: Vec<(PathBuf, SystemTime)> = Vec::new();
    for entry in paths {
        let path = match entry {
            Ok(p) => p,
            Err(_) => continue,
        };
        let resolved = match path.canonicalize() {
            Ok(r) => r,
            Err(_) => continue,
        };
        if !resolved.starts_with(&cwd) {
            continue;
        }
        let display = path.display().to_string();
        if display.split('/').any(|s| s.starts_with('.')) {
            continue;
        }
        let mtime =
            std::fs::metadata(&path).and_then(|m| m.modified()).unwrap_or(SystemTime::UNIX_EPOCH);
        matches.push((path, mtime));
    }

    matches.sort_by(|a, b| b.1.cmp(&a.1));
    matches.truncate(MAX_RESULTS);

    if matches.is_empty() {
        format!("Sin coincidencias para patrón: {pattern}")
    } else {
        let count = matches.len();
        let list: Vec<String> = matches.iter().map(|(p, _)| p.display().to_string()).collect();
        format!("{count} archivos encontrados (ordenados por mtime desc):\n{}", list.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn invalid_json_returns_error() {
        let out = GlobTool.execute("{bad").await;
        assert!(out.starts_with("Error parsing"));
    }

    #[tokio::test]
    async fn empty_pattern_no_match() {
        let args = r#"{"pattern":"zzzzznonexistent_file_pattern_xyz_*"}"#;
        let out = GlobTool.execute(args).await;
        assert!(out.starts_with("Sin coincidencias"));
    }

    #[tokio::test]
    async fn invalid_pattern_errors() {
        let args = r#"{"pattern":"[unclosed"}"#;
        let out = GlobTool.execute(args).await;
        assert!(out.contains("inválido"));
    }
}
