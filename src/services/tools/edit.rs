//! Edit tool: sustituye `old_string` por `new_string` en un archivo existente.
//!
//! Contrato (Claude Code-style):
//! - `old_string` debe coincidir EXACTO en el archivo.
//! - Si `replace_all == false` (default), `old_string` debe ser único (1 match).
//! - Si `replace_all == true`, reemplaza todas las ocurrencias.
//! - La ruta debe existir (no crea archivos; para eso hay `write_file`).
//! - Sandbox: la ruta resuelta debe quedar dentro del cwd.
//!
//! El enforcement de permission (Ask) y workspace boundary se resuelven
//! en `PermissionEnforcer` antes de llegar aquí.
use std::path::{Path, PathBuf};

use serde::Deserialize;

use super::{Tool, ToolPermission};
use crate::services::chat::ToolDefinition;

pub struct EditFileTool;

#[derive(Deserialize)]
struct EditArgs {
    #[serde(alias = "file_path")]
    path: String,
    old_string: String,
    new_string: String,
    #[serde(default)]
    replace_all: bool,
}

#[async_trait::async_trait]
impl Tool for EditFileTool {
    fn name(&self) -> &str {
        "edit_file"
    }

    fn permission(&self) -> ToolPermission {
        ToolPermission::Ask
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            json: serde_json::json!({
                "type": "function",
                "function": {
                    "name": "edit_file",
                    "description": "Reemplaza `old_string` por `new_string` en un archivo existente. Requiere match exacto. Si `replace_all` es false (default), `old_string` debe ser único en el archivo. Para crear archivos nuevos usa `write_file`.",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "path": { "type": "string", "description": "Ruta del archivo a editar" },
                            "old_string": { "type": "string", "description": "Texto exacto a reemplazar" },
                            "new_string": { "type": "string", "description": "Texto de reemplazo" },
                            "replace_all": { "type": "boolean", "description": "Reemplazar todas las ocurrencias (default false)" }
                        },
                        "required": ["path", "old_string", "new_string"]
                    }
                }
            }),
        }
    }

    async fn execute(&self, arguments: &str) -> String {
        let args: EditArgs = match serde_json::from_str(arguments) {
            Ok(a) => a,
            Err(e) => return format!("Error parsing arguments: {e}"),
        };
        edit_file(&args.path, &args.old_string, &args.new_string, args.replace_all).await
    }
}

async fn edit_file(path: &str, old: &str, new: &str, replace_all: bool) -> String {
    if old == new {
        return "Error: old_string y new_string son idénticos — nada que editar".into();
    }
    if old.is_empty() {
        return "Error: old_string vacío — usa write_file para crear contenido".into();
    }

    let resolved = match sandbox_path(path) {
        Ok(p) => p,
        Err(e) => return e.to_string(),
    };
    if !resolved.is_file() {
        return format!("Error: no es un archivo: {path}");
    }

    let content = match tokio::fs::read_to_string(&resolved).await {
        Ok(c) => c,
        Err(e) => return format!("Error leyendo archivo: {e}"),
    };

    let count = content.matches(old).count();
    if count == 0 {
        return format!(
            "Error: old_string no encontrado en {path}. Verifica indentación / espacios / saltos de línea."
        );
    }
    if count > 1 && !replace_all {
        return format!(
            "Error: old_string aparece {count} veces en {path}. Amplía el contexto para que sea único, o usa replace_all=true."
        );
    }

    let updated =
        if replace_all { content.replace(old, new) } else { content.replacen(old, new, 1) };

    if let Err(e) = tokio::fs::write(&resolved, &updated).await {
        return format!("Error escribiendo archivo: {e}");
    }

    let removed = old.lines().count();
    let added = new.lines().count();
    let replacements = if replace_all { count } else { 1 };
    format!("Archivo editado: {path} ({replacements} reemplazo(s), +{added} -{removed} líneas)")
}

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
            "Error: acceso denegado — la ruta '{user_path}' está fuera del directorio del proyecto"
        );
    }
    Ok(resolved)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn tmpfile(content: &str) -> tempfile::NamedTempFile {
        let mut f = tempfile::NamedTempFile::new_in(std::env::current_dir().unwrap()).unwrap();
        f.write_all(content.as_bytes()).unwrap();
        f.flush().unwrap();
        f
    }

    #[tokio::test]
    async fn edit_single_match_replaces() {
        let f = tmpfile("let x = 1;\nlet y = 2;\n");
        let path = f.path().to_string_lossy().to_string();
        let args = format!(
            r#"{{"path":"{}","old_string":"let x = 1;","new_string":"let x = 42;"}}"#,
            path
        );
        let out = EditFileTool.execute(&args).await;
        assert!(out.starts_with("Archivo editado"), "got: {out}");
        let after = std::fs::read_to_string(f.path()).unwrap();
        assert!(after.contains("let x = 42;"));
        assert!(after.contains("let y = 2;"));
    }

    #[tokio::test]
    async fn edit_ambiguous_without_replace_all_fails() {
        let f = tmpfile("foo\nfoo\n");
        let path = f.path().to_string_lossy().to_string();
        let args = format!(r#"{{"path":"{}","old_string":"foo","new_string":"bar"}}"#, path);
        let out = EditFileTool.execute(&args).await;
        assert!(out.contains("aparece 2 veces"), "got: {out}");
    }

    #[tokio::test]
    async fn edit_replace_all_works() {
        let f = tmpfile("foo\nfoo\n");
        let path = f.path().to_string_lossy().to_string();
        let args = format!(
            r#"{{"path":"{}","old_string":"foo","new_string":"bar","replace_all":true}}"#,
            path
        );
        let out = EditFileTool.execute(&args).await;
        assert!(out.starts_with("Archivo editado"));
        let after = std::fs::read_to_string(f.path()).unwrap();
        assert_eq!(after, "bar\nbar\n");
    }

    #[tokio::test]
    async fn edit_no_match_errors() {
        let f = tmpfile("hello\n");
        let path = f.path().to_string_lossy().to_string();
        let args = format!(r#"{{"path":"{}","old_string":"missing","new_string":"x"}}"#, path);
        let out = EditFileTool.execute(&args).await;
        assert!(out.contains("no encontrado"));
    }

    #[tokio::test]
    async fn edit_same_old_new_errors() {
        let args = r#"{"path":"a","old_string":"x","new_string":"x"}"#;
        let out = EditFileTool.execute(args).await;
        assert!(out.contains("idénticos"));
    }
}
