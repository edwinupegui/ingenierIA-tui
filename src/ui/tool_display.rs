//! Presentacion especifica por tool (E32).
//!
//! Helpers puros que toman el nombre del tool + argumentos JSON y generan:
//! - Iconos semanticos
//! - Previews estructurados para el permission modal
//! - Limites de truncacion por tool para el render del resultado
//!
//! Sin efectos secundarios. Se consumen desde `ui/chat_tools.rs` y
//! `ui/widgets/permission_modal.rs`.

use serde_json::Value;

/// Glyph + etiqueta corta (3-5 chars) que resume la naturaleza del tool.
/// Usado como prefijo en el permission modal y como agrupador en chat.
pub fn tool_icon(tool_name: &str) -> (&'static str, &'static str) {
    match normalize_tool_name(tool_name) {
        "bash" | "shell" | "exec" => ("⚡", "bash"),
        "read_file" | "read" | "view" => ("📖", "read"),
        "write_file" | "write" | "create" => ("✎", "write"),
        "edit" | "edit_file" | "patch" => ("✏", "edit"),
        "glob" | "glob_files" | "search_files" => ("🗂", "glob"),
        "grep" | "grep_files" | "search" => ("🔎", "grep"),
        "list_directory" | "ls" => ("📁", "ls"),
        "web_fetch" | "fetch" | "http" => ("🌐", "web"),
        "mcp" => ("◆", "mcp"),
        _ => ("▸", "tool"),
    }
}

/// Limites de truncacion para el output del tool: (max_lines, max_chars).
///
/// Criterios (E32):
/// - Read es verbose → 80 lineas / 6KB
/// - Bash tipicamente compacto → 60 lineas / 4KB
/// - Write/Edit rara vez tienen stdout largo → 20 lineas / 2KB
/// - Glob/Grep listan paths → 40 lineas / 3KB
/// - Generic → 30 lineas / 2KB
pub fn result_limits(tool_name: &str) -> (usize, usize) {
    match normalize_tool_name(tool_name) {
        "read_file" | "read" | "view" => (80, 6_000),
        "bash" | "shell" | "exec" => (60, 4_000),
        "write_file" | "write" | "edit" | "edit_file" => (20, 2_000),
        "glob" | "glob_files" | "search_files" | "grep" | "grep_files" | "list_directory" => {
            (40, 3_000)
        }
        _ => (30, 2_000),
    }
}

/// Extrae un preview estructurado de los argumentos segun el tool.
/// Retorna `None` si no hay preview especializado — el caller debe caer al
/// render generico de args JSON.
pub fn structured_preview(tool_name: &str, args_json: &str) -> Option<Vec<PreviewLine>> {
    let value: Value = serde_json::from_str(args_json).ok()?;
    let obj = value.as_object()?;
    match normalize_tool_name(tool_name) {
        "bash" | "shell" | "exec" => bash_preview(obj),
        "read_file" | "read" | "view" => path_preview(obj, "Archivo"),
        "write_file" | "write" | "create" => path_preview(obj, "Escribir"),
        "edit" | "edit_file" | "patch" => edit_preview(obj),
        "glob" | "glob_files" | "search_files" => pattern_preview(obj, "Patron"),
        "grep" | "grep_files" | "search" => grep_preview(obj),
        "list_directory" | "ls" => path_preview(obj, "Directorio"),
        _ => None,
    }
}

/// Linea de preview con semantica — el widget decide el estilo.
///
/// `value_alt` solo se usa cuando `kind == Diff`: contiene el `new_string`
/// mientras `value` contiene el `old_string`. Para el resto de kinds es `None`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreviewLine {
    pub label: String,
    pub value: String,
    pub value_alt: Option<String>,
    pub kind: PreviewKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreviewKind {
    /// Comando shell — renderizar en bordered box monospace.
    Command,
    /// Path de archivo — resaltar el nombre final.
    Path,
    /// Patron (glob/regex) — comillas.
    Pattern,
    /// Texto descriptivo.
    Text,
    /// Diff inline old vs new — `value` = old, `value_alt` = new.
    Diff,
}

fn bash_preview(obj: &serde_json::Map<String, Value>) -> Option<Vec<PreviewLine>> {
    let cmd = obj.get("command").or_else(|| obj.get("cmd"))?.as_str()?;
    let mut lines = vec![PreviewLine {
        label: "Comando".to_string(),
        value: cmd.to_string(),
        value_alt: None,
        kind: PreviewKind::Command,
    }];
    if let Some(desc) = obj.get("description").and_then(|v| v.as_str()) {
        lines.push(PreviewLine {
            label: "Proposito".to_string(),
            value: desc.to_string(),
            value_alt: None,
            kind: PreviewKind::Text,
        });
    }
    if let Some(timeout) = obj.get("timeout").and_then(|v| v.as_u64()) {
        lines.push(PreviewLine {
            label: "Timeout".to_string(),
            value: format!("{timeout}ms"),
            value_alt: None,
            kind: PreviewKind::Text,
        });
    }
    Some(lines)
}

fn path_preview(obj: &serde_json::Map<String, Value>, label: &str) -> Option<Vec<PreviewLine>> {
    let path = obj
        .get("path")
        .or_else(|| obj.get("file_path"))
        .or_else(|| obj.get("filepath"))?
        .as_str()?;
    Some(vec![PreviewLine {
        label: label.to_string(),
        value: path.to_string(),
        value_alt: None,
        kind: PreviewKind::Path,
    }])
}

fn edit_preview(obj: &serde_json::Map<String, Value>) -> Option<Vec<PreviewLine>> {
    let path = obj.get("path").or_else(|| obj.get("file_path")).and_then(|v| v.as_str())?;
    let mut lines = vec![PreviewLine {
        label: "Archivo".to_string(),
        value: path.to_string(),
        value_alt: None,
        kind: PreviewKind::Path,
    }];
    let old = obj.get("old_string").and_then(|v| v.as_str()).unwrap_or("");
    let new = obj.get("new_string").and_then(|v| v.as_str()).unwrap_or("");
    if !old.is_empty() || !new.is_empty() {
        lines.push(PreviewLine {
            label: "Cambios".to_string(),
            value: old.to_string(),
            value_alt: Some(new.to_string()),
            kind: PreviewKind::Diff,
        });
    }
    Some(lines)
}

fn pattern_preview(obj: &serde_json::Map<String, Value>, label: &str) -> Option<Vec<PreviewLine>> {
    let pattern = obj.get("pattern").or_else(|| obj.get("query"))?.as_str()?;
    let mut lines = vec![PreviewLine {
        label: label.to_string(),
        value: format!("\"{pattern}\""),
        value_alt: None,
        kind: PreviewKind::Pattern,
    }];
    if let Some(scope) = obj.get("path").and_then(|v| v.as_str()) {
        lines.push(PreviewLine {
            label: "Scope".to_string(),
            value: scope.to_string(),
            value_alt: None,
            kind: PreviewKind::Path,
        });
    }
    Some(lines)
}

fn grep_preview(obj: &serde_json::Map<String, Value>) -> Option<Vec<PreviewLine>> {
    let pattern = obj.get("pattern").or_else(|| obj.get("query"))?.as_str()?;
    let mut lines = vec![PreviewLine {
        label: "Regex".to_string(),
        value: format!("\"{pattern}\""),
        value_alt: None,
        kind: PreviewKind::Pattern,
    }];
    if let Some(glob) = obj.get("glob").and_then(|v| v.as_str()) {
        lines.push(PreviewLine {
            label: "Glob".to_string(),
            value: glob.to_string(),
            value_alt: None,
            kind: PreviewKind::Pattern,
        });
    }
    if let Some(path) = obj.get("path").and_then(|v| v.as_str()) {
        lines.push(PreviewLine {
            label: "Scope".to_string(),
            value: path.to_string(),
            value_alt: None,
            kind: PreviewKind::Path,
        });
    }
    Some(lines)
}

fn normalize_tool_name(name: &str) -> &str {
    // Recorta prefijos de MCP namespacing "server:tool" → "tool".
    name.rsplit(':').next().unwrap_or(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_icon_recognizes_common_tools() {
        assert_eq!(tool_icon("bash").1, "bash");
        assert_eq!(tool_icon("read_file").1, "read");
        assert_eq!(tool_icon("grep_files").1, "grep");
        assert_eq!(tool_icon("unknown_tool").1, "tool");
    }

    #[test]
    fn normalize_strips_namespace_prefix() {
        assert_eq!(normalize_tool_name("ingenieria:bash"), "bash");
        assert_eq!(normalize_tool_name("bash"), "bash");
    }

    #[test]
    fn result_limits_vary_by_tool() {
        assert!(result_limits("read_file").0 > result_limits("write_file").0);
        assert!(result_limits("bash").0 > result_limits("write_file").0);
    }

    #[test]
    fn bash_preview_extracts_command() {
        let args = r#"{"command":"ls -la","description":"listar"}"#;
        let preview = structured_preview("bash", args).unwrap();
        assert_eq!(preview[0].kind, PreviewKind::Command);
        assert_eq!(preview[0].value, "ls -la");
        assert_eq!(preview[1].value, "listar");
    }

    #[test]
    fn read_preview_extracts_path() {
        let args = r#"{"path":"src/lib.rs"}"#;
        let preview = structured_preview("read_file", args).unwrap();
        assert_eq!(preview[0].kind, PreviewKind::Path);
        assert_eq!(preview[0].value, "src/lib.rs");
    }

    #[test]
    fn edit_preview_emits_diff_line() {
        let args = r#"{"path":"a.rs","old_string":"foo","new_string":"bar"}"#;
        let preview = structured_preview("edit_file", args).unwrap();
        assert_eq!(preview.len(), 2);
        assert_eq!(preview[0].kind, PreviewKind::Path);
        assert_eq!(preview[1].kind, PreviewKind::Diff);
        assert_eq!(preview[1].value, "foo");
        assert_eq!(preview[1].value_alt.as_deref(), Some("bar"));
    }

    #[test]
    fn grep_preview_includes_scope_when_present() {
        let args = r#"{"pattern":"TODO","path":"src/"}"#;
        let preview = structured_preview("grep_files", args).unwrap();
        assert!(preview.iter().any(|l| l.kind == PreviewKind::Pattern));
        assert!(preview.iter().any(|l| l.kind == PreviewKind::Path));
    }

    #[test]
    fn structured_preview_returns_none_for_unknown() {
        assert!(structured_preview("mystery_tool", "{}").is_none());
    }
}
