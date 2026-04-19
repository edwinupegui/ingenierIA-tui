//! LSP diagnostic types (E25).
//!
//! Subset minimo del LSP spec que necesitamos para capturar diagnosticos
//! y formatearlos como contexto AI. No reimplementa todo el protocolo —
//! solo lo que se parsea de `textDocument/publishDiagnostics`.

/// Severidad de un diagnostico (match con LSP DiagnosticSeverity).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Error = 1,
    Warning = 2,
    Information = 3,
    Hint = 4,
}

impl Severity {
    pub fn from_lsp(value: u8) -> Self {
        match value {
            1 => Self::Error,
            2 => Self::Warning,
            3 => Self::Information,
            _ => Self::Hint,
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            Self::Error => "E",
            Self::Warning => "W",
            Self::Information => "I",
            Self::Hint => "H",
        }
    }
}

/// Posicion en un archivo (0-indexed, como LSP).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Position {
    pub line: u32,
    pub character: u32,
}

/// Rango en un archivo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Range {
    pub start: Position,
    pub end: Position,
}

/// Diagnostico parseado de `publishDiagnostics`.
#[derive(Debug, Clone)]
pub struct LspDiagnostic {
    /// URI del archivo (file:///...).
    pub uri: String,
    /// Path relativo al CWD (display-friendly).
    pub path: String,
    pub range: Range,
    pub severity: Severity,
    pub message: String,
    /// Source opcional (ej: "rustc", "clippy").
    pub source: Option<String>,
    /// Codigo del diagnostico (ej: "E0599", "unused_variables").
    pub code: Option<String>,
}

impl LspDiagnostic {
    /// Display-friendly location: `path:line:col`.
    pub fn location(&self) -> String {
        format!("{}:{}:{}", self.path, self.range.start.line + 1, self.range.start.character + 1)
    }
}

/// Parsea un diagnostico desde JSON (sub-valor de publishDiagnostics.params.diagnostics[]).
pub fn parse_diagnostic(uri: &str, diag: &serde_json::Value) -> Option<LspDiagnostic> {
    let message = diag.get("message")?.as_str()?.to_string();
    let severity =
        Severity::from_lsp(diag.get("severity").and_then(|v| v.as_u64()).unwrap_or(4) as u8);
    let range_val = diag.get("range")?;
    let range = parse_range(range_val)?;
    let source = diag.get("source").and_then(|v| v.as_str()).map(String::from);
    let code = diag
        .get("code")
        .and_then(|v| v.as_str().map(String::from).or_else(|| v.as_u64().map(|n| n.to_string())));

    let path = uri_to_relative_path(uri);

    Some(LspDiagnostic { uri: uri.to_string(), path, range, severity, message, source, code })
}

fn parse_range(v: &serde_json::Value) -> Option<Range> {
    let start = v.get("start")?;
    let end = v.get("end")?;
    Some(Range {
        start: Position {
            line: start.get("line")?.as_u64()? as u32,
            character: start.get("character")?.as_u64()? as u32,
        },
        end: Position {
            line: end.get("line")?.as_u64()? as u32,
            character: end.get("character")?.as_u64()? as u32,
        },
    })
}

/// Convierte un URI file:/// a path relativo al CWD (best-effort).
pub fn uri_to_relative_path(uri: &str) -> String {
    let path = uri.strip_prefix("file://").unwrap_or(uri);
    // Decode %20, etc — minimal.
    let decoded = path.replace("%20", " ");
    if let Ok(cwd) = std::env::current_dir() {
        if let Ok(rel) = std::path::Path::new(&decoded).strip_prefix(&cwd) {
            return rel.display().to_string();
        }
    }
    decoded
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn severity_from_lsp_maps_correctly() {
        assert_eq!(Severity::from_lsp(1), Severity::Error);
        assert_eq!(Severity::from_lsp(2), Severity::Warning);
        assert_eq!(Severity::from_lsp(3), Severity::Information);
        assert_eq!(Severity::from_lsp(99), Severity::Hint);
    }

    #[test]
    fn parse_diagnostic_from_json() {
        let json = serde_json::json!({
            "message": "unused variable",
            "severity": 2,
            "range": {
                "start": { "line": 10, "character": 4 },
                "end": { "line": 10, "character": 10 }
            },
            "source": "rustc",
            "code": "unused_variables"
        });
        let diag = parse_diagnostic("file:///tmp/foo.rs", &json).unwrap();
        assert_eq!(diag.severity, Severity::Warning);
        assert_eq!(diag.message, "unused variable");
        assert_eq!(diag.source.as_deref(), Some("rustc"));
        assert_eq!(diag.code.as_deref(), Some("unused_variables"));
        assert_eq!(diag.range.start.line, 10);
    }

    #[test]
    fn parse_diagnostic_with_numeric_code() {
        let json = serde_json::json!({
            "message": "not found",
            "severity": 1,
            "range": { "start": { "line": 0, "character": 0 }, "end": { "line": 0, "character": 5 } },
            "code": 599
        });
        let diag = parse_diagnostic("file:///tmp/x.rs", &json).unwrap();
        assert_eq!(diag.code.as_deref(), Some("599"));
    }

    #[test]
    fn location_is_one_indexed() {
        let diag = LspDiagnostic {
            uri: "file:///a".into(),
            path: "a".into(),
            range: Range {
                start: Position { line: 0, character: 0 },
                end: Position { line: 0, character: 5 },
            },
            severity: Severity::Error,
            message: "x".into(),
            source: None,
            code: None,
        };
        assert_eq!(diag.location(), "a:1:1");
    }

    #[test]
    fn uri_to_relative_path_strips_file_prefix() {
        let result = uri_to_relative_path("file:///tmp/hello.rs");
        assert!(result.contains("hello.rs"));
    }
}
