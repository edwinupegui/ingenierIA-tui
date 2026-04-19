use std::path::{Path, PathBuf};

use serde::Deserialize;

use super::{Tool, ToolPermission};
use crate::services::chat::ToolDefinition;

const DEFAULT_MAX_LINES: usize = 200;
const MAX_SEARCH_RESULTS: usize = 50;
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

    // Canonicalize to resolve any ".." or symlinks
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

// ── read_file ───────────────────────────────────────────────────────────────

pub struct ReadFileTool;

#[derive(Deserialize, Default)]
struct ReadFileArgs {
    path: String,
    max_lines: Option<usize>,
}

#[async_trait::async_trait]
impl Tool for ReadFileTool {
    fn name(&self) -> &str {
        "read_file"
    }

    fn permission(&self) -> ToolPermission {
        ToolPermission::Safe
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            json: serde_json::json!({
                "type": "function",
                "function": {
                    "name": "read_file",
                    "description": "Lee el contenido de un archivo del sistema de archivos local.",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "path": { "type": "string", "description": "Ruta al archivo" },
                            "max_lines": { "type": "integer", "description": "Max lineas (default: 200)" }
                        },
                        "required": ["path"]
                    }
                }
            }),
        }
    }

    async fn execute(&self, arguments: &str) -> String {
        let args: ReadFileArgs = match serde_json::from_str(arguments) {
            Ok(a) => a,
            Err(e) => return format!("Error parsing arguments: {e}"),
        };
        read_file(&args.path, args.max_lines.unwrap_or(DEFAULT_MAX_LINES)).await
    }
}

async fn read_file(path: &str, max_lines: usize) -> String {
    let resolved_path = match sandbox_path(path) {
        Ok(valid) => valid,
        Err(e) => return e.to_string(),
    };
    if !resolved_path.is_file() {
        return format!("Error: not a file: {path}");
    }
    match tokio::fs::read_to_string(resolved_path).await {
        Ok(content) => {
            let lines: Vec<&str> = content.lines().collect();
            let total = lines.len();
            let truncated = total > max_lines;
            let display: String = lines
                .iter()
                .take(max_lines)
                .enumerate()
                .map(|(i, l)| format!("{:>4} | {}", i + 1, l))
                .collect::<Vec<_>>()
                .join("\n");
            if truncated {
                format!(
                    "{display}\n\n... ({total} lineas total, mostrando las primeras {max_lines})"
                )
            } else {
                display
            }
        }
        Err(e) => format!("Error reading file: {e}"),
    }
}

// ── list_directory ──────────────────────────────────────────────────────────

pub struct ListDirectoryTool;

#[derive(Deserialize, Default)]
struct ListDirArgs {
    path: Option<String>,
}

#[async_trait::async_trait]
impl Tool for ListDirectoryTool {
    fn name(&self) -> &str {
        "list_directory"
    }

    fn permission(&self) -> ToolPermission {
        ToolPermission::Safe
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            json: serde_json::json!({
                "type": "function",
                "function": {
                    "name": "list_directory",
                    "description": "Lista archivos y directorios en una ruta dada.",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "path": { "type": "string", "description": "Ruta del directorio (default: .)" }
                        },
                        "required": []
                    }
                }
            }),
        }
    }

    async fn execute(&self, arguments: &str) -> String {
        let args: ListDirArgs = serde_json::from_str(arguments).unwrap_or_default();
        list_directory(args.path.as_deref().unwrap_or(".")).await
    }
}

async fn list_directory(path: &str) -> String {
    let resolved_path = match sandbox_path(path) {
        Ok(valid) => valid,
        Err(e) => return e.to_string(),
    };
    if !resolved_path.is_dir() {
        return format!("Error: not a directory: {path}");
    }
    match tokio::fs::read_dir(resolved_path).await {
        Ok(mut entries) => {
            let mut items: Vec<String> = Vec::new();
            while let Ok(Some(entry)) = entries.next_entry().await {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with('.') {
                    continue;
                }
                let meta = entry.metadata().await;
                let (kind, size) = match meta {
                    Ok(m) if m.is_dir() => ("dir ", String::new()),
                    Ok(m) => {
                        let s = m.len();
                        let size_str = if s < 1024 {
                            format!("{s}B")
                        } else if s < 1024 * 1024 {
                            format!("{}KB", s / 1024)
                        } else {
                            format!("{}MB", s / (1024 * 1024))
                        };
                        ("file", size_str)
                    }
                    Err(_) => ("?   ", String::new()),
                };
                items.push(format!("  {kind}  {size:>8}  {name}"));
            }
            items.sort();
            if items.is_empty() {
                format!("(directorio vacio: {path})")
            } else {
                format!("Contenido de {path}:\n{}", items.join("\n"))
            }
        }
        Err(e) => format!("Error listing directory: {e}"),
    }
}

// ── search_files ────────────────────────────────────────────────────────────

pub struct SearchFilesTool;

#[derive(Deserialize, Default)]
struct SearchFilesArgs {
    pattern: String,
}

#[async_trait::async_trait]
impl Tool for SearchFilesTool {
    fn name(&self) -> &str {
        "search_files"
    }

    fn permission(&self) -> ToolPermission {
        ToolPermission::Safe
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            json: serde_json::json!({
                "type": "function",
                "function": {
                    "name": "search_files",
                    "description": "Busca archivos por patron glob en el directorio actual.",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "pattern": { "type": "string", "description": "Patron glob (ej: '*.rs')" }
                        },
                        "required": ["pattern"]
                    }
                }
            }),
        }
    }

    async fn execute(&self, arguments: &str) -> String {
        let args: SearchFilesArgs = match serde_json::from_str(arguments) {
            Ok(a) => a,
            Err(e) => return format!("Error parsing arguments: {e}"),
        };
        search_files(&args.pattern)
    }
}

fn search_files(pattern: &str) -> String {
    let cwd = match std::env::current_dir() {
        Ok(d) => d,
        Err(e) => return format!("Error: cannot determine working directory: {e}"),
    };

    let glob_pattern = if pattern.contains('/') || pattern.contains('*') {
        pattern.to_string()
    } else {
        format!("**/{pattern}")
    };

    let mut matches = Vec::new();
    match glob::glob(&glob_pattern) {
        Ok(paths) => {
            for entry in paths.take(MAX_SEARCH_RESULTS) {
                match entry {
                    Ok(path) => {
                        // Validate resolved path stays within project sandbox
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
                        matches.push(display);
                    }
                    Err(e) => matches.push(format!("(error: {e})")),
                }
            }
        }
        Err(e) => return format!("Invalid pattern: {e}"),
    }

    if matches.is_empty() {
        format!("No files found matching: {pattern}")
    } else {
        let count = matches.len();
        let list = matches.join("\n  ");
        format!("{count} archivos encontrados:\n  {list}")
    }
}
