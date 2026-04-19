use std::path::{Path, PathBuf};

use serde::Deserialize;

use super::{Tool, ToolPermission};
use crate::services::chat::ToolDefinition;

const DEFAULT_MAX_GREP_RESULTS: usize = 30;

/// Variant for paths that may not exist yet (e.g. write_file). Validates
/// that the resolved *parent* is within the sandbox.
fn sandbox_path_for_write(user_path: &str) -> anyhow::Result<PathBuf> {
    let cwd = std::env::current_dir()
        .map_err(|e| anyhow::anyhow!("Error: cannot determine working directory: {e}"))?;

    let candidate = if Path::new(user_path).is_absolute() {
        PathBuf::from(user_path)
    } else {
        cwd.join(user_path)
    };

    // For new files, canonicalize the parent directory
    let parent =
        candidate.parent().ok_or_else(|| anyhow::anyhow!("Error: invalid path: {user_path}"))?;

    // Parent must exist and be inside cwd
    let resolved_parent = parent
        .canonicalize()
        .map_err(|_| anyhow::anyhow!("Error: parent directory not found: {user_path}"))?;

    if !resolved_parent.starts_with(&cwd) {
        anyhow::bail!(
            "Error: acceso denegado — la ruta '{}' está fuera del directorio del proyecto",
            user_path
        );
    }

    Ok(candidate)
}

/// Resolves a user-provided path against the current working directory and
/// ensures it stays within the sandbox (project root). Returns an error string
/// if the path escapes the sandbox via `..` or absolute path tricks.
fn sandbox_path(user_path: &str) -> anyhow::Result<PathBuf> {
    let cwd = std::env::current_dir()
        .map_err(|e| anyhow::anyhow!("Error: cannot determine working directory: {e}"))?;

    let candidate = if Path::new(user_path).is_absolute() {
        PathBuf::from(user_path)
    } else {
        cwd.join(user_path)
    };

    let resolved = candidate
        .canonicalize()
        .map_err(|_| anyhow::anyhow!("Error: path not found or inaccessible: {user_path}"))?;

    if !resolved.starts_with(&cwd) {
        anyhow::bail!(
            "Error: acceso denegado — la ruta '{}' está fuera del directorio del proyecto",
            user_path
        );
    }

    Ok(resolved)
}

// ── write_file ──────────────────────────────────────────────────────────────

pub struct WriteFileTool;

#[derive(Deserialize)]
struct WriteFileArgs {
    path: String,
    content: String,
}

#[async_trait::async_trait]
impl Tool for WriteFileTool {
    fn name(&self) -> &str {
        "write_file"
    }

    fn permission(&self) -> ToolPermission {
        ToolPermission::Ask
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            json: serde_json::json!({
                "type": "function",
                "function": {
                    "name": "write_file",
                    "description": "Escribe contenido a un archivo. Crea directorios padre si no existen. SOBRESCRIBE el archivo si ya existe.",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "path": { "type": "string", "description": "Ruta del archivo a escribir" },
                            "content": { "type": "string", "description": "Contenido a escribir" }
                        },
                        "required": ["path", "content"]
                    }
                }
            }),
        }
    }

    async fn execute(&self, arguments: &str) -> String {
        let args: WriteFileArgs = match serde_json::from_str(arguments) {
            Ok(a) => a,
            Err(e) => return format!("Error parsing arguments: {e}"),
        };
        write_file(&args.path, &args.content).await
    }
}

async fn write_file(path: &str, content: &str) -> String {
    let p = match sandbox_path_for_write(path) {
        Ok(p) => p,
        Err(e) => return e.to_string(),
    };
    if let Some(parent) = p.parent() {
        if !parent.exists() {
            if let Err(e) = tokio::fs::create_dir_all(parent).await {
                return format!("Error creating directories: {e}");
            }
        }
    }
    match tokio::fs::write(&p, content).await {
        Ok(()) => format!("Archivo escrito: {path} ({} bytes)", content.len()),
        Err(e) => format!("Error writing file: {e}"),
    }
}

// ── grep_files ──────────────────────────────────────────────────────────────

pub struct GrepFilesTool;

#[derive(Deserialize)]
struct GrepFilesArgs {
    pattern: String,
    path: Option<String>,
    max_results: Option<usize>,
}

#[async_trait::async_trait]
impl Tool for GrepFilesTool {
    fn name(&self) -> &str {
        "grep_files"
    }

    fn permission(&self) -> ToolPermission {
        ToolPermission::Safe
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            json: serde_json::json!({
                "type": "function",
                "function": {
                    "name": "grep_files",
                    "description": "Busca un patron de texto dentro de archivos. Retorna las lineas que coinciden con contexto.",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "pattern": { "type": "string", "description": "Texto o regex a buscar" },
                            "path": { "type": "string", "description": "Directorio donde buscar (default: .)" },
                            "max_results": { "type": "integer", "description": "Max resultados (default: 30)" }
                        },
                        "required": ["pattern"]
                    }
                }
            }),
        }
    }

    async fn execute(&self, arguments: &str) -> String {
        let args: GrepFilesArgs = match serde_json::from_str(arguments) {
            Ok(a) => a,
            Err(e) => return format!("Error parsing arguments: {e}"),
        };
        grep_files(
            &args.pattern,
            args.path.as_deref().unwrap_or("."),
            args.max_results.unwrap_or(DEFAULT_MAX_GREP_RESULTS),
        )
        .await
    }
}

async fn grep_files(pattern: &str, dir: &str, max_results: usize) -> String {
    let dir_path = match sandbox_path(dir) {
        Ok(p) => p,
        Err(e) => return e.to_string(),
    };
    if !dir_path.is_dir() {
        return format!("Error: not a directory: {dir}");
    }

    let mut results = Vec::new();
    let dir_str = dir_path.display().to_string();
    let glob_pattern = format!("{}/**/*", dir_str.trim_end_matches('/'));

    let paths = match glob::glob(&glob_pattern) {
        Ok(p) => p,
        Err(e) => return format!("Error: {e}"),
    };

    for entry in paths {
        if results.len() >= max_results {
            break;
        }
        let path = match entry {
            Ok(p) if p.is_file() => p,
            _ => continue,
        };
        let display = path.display().to_string();
        if display.split('/').any(|s| s.starts_with('.')) {
            continue;
        }
        // Skip binary-looking extensions
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if matches!(ext, "png" | "jpg" | "gif" | "wasm" | "bin" | "exe" | "dll" | "so") {
            continue;
        }
        let content = match tokio::fs::read_to_string(&path).await {
            Ok(c) => c,
            Err(_) => continue,
        };
        for (i, line) in content.lines().enumerate() {
            if results.len() >= max_results {
                break;
            }
            if line.contains(pattern) {
                results.push(format!("{}:{}: {}", display, i + 1, line.trim()));
            }
        }
    }

    if results.is_empty() {
        format!("No matches for \"{pattern}\" in {dir}")
    } else {
        let count = results.len();
        format!("{count} coincidencias:\n{}", results.join("\n"))
    }
}
