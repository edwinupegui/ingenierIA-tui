//! Formateo de diagnosticos LSP para contexto AI (E25).
//!
//! Convierte la lista acumulada de diagnosticos en una seccion markdown
//! que se inyecta en el system prompt, dentro del budget de contexto.

use super::types::{LspDiagnostic, Severity};

/// Tope de diagnosticos incluidos en contexto AI — evita saturar el prompt.
const MAX_DIAGNOSTICS_IN_CONTEXT: usize = 30;

/// Formatea diagnosticos como seccion markdown para el system prompt.
/// Filtra solo errores y warnings (info+hint son ruido). Trunca a
/// [`MAX_DIAGNOSTICS_IN_CONTEXT`].
pub fn format_diagnostics_context(diagnostics: &[LspDiagnostic]) -> Option<String> {
    let relevant: Vec<&LspDiagnostic> = diagnostics
        .iter()
        .filter(|d| matches!(d.severity, Severity::Error | Severity::Warning))
        .take(MAX_DIAGNOSTICS_IN_CONTEXT)
        .collect();

    if relevant.is_empty() {
        return None;
    }

    let errors = relevant.iter().filter(|d| d.severity == Severity::Error).count();
    let warnings = relevant.iter().filter(|d| d.severity == Severity::Warning).count();
    let total = diagnostics
        .iter()
        .filter(|d| matches!(d.severity, Severity::Error | Severity::Warning))
        .count();

    let mut out = String::from("## Diagnosticos del proyecto (LSP)\n\n");
    out.push_str(&format!("{errors} error(es), {warnings} warning(s)"));
    if total > relevant.len() {
        out.push_str(&format!(" (mostrando {} de {total})", relevant.len()));
    }
    out.push_str("\n\n");

    for diag in &relevant {
        let loc = diag.location();
        let icon = diag.severity.icon();
        let source_tag = diag.source.as_deref().map(|s| format!("[{s}] ")).unwrap_or_default();
        let code_tag = diag.code.as_deref().map(|c| format!("({c}) ")).unwrap_or_default();
        out.push_str(&format!(
            "- `{loc}` [{icon}] {source_tag}{code_tag}{}\n",
            diag.message.lines().next().unwrap_or("")
        ));
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::lsp::types::{Position, Range};

    fn diag(sev: Severity, msg: &str) -> LspDiagnostic {
        LspDiagnostic {
            uri: "file:///x".into(),
            path: "x.rs".into(),
            range: Range {
                start: Position { line: 0, character: 0 },
                end: Position { line: 0, character: 5 },
            },
            severity: sev,
            message: msg.into(),
            source: Some("rustc".into()),
            code: Some("E0599".into()),
        }
    }

    #[test]
    fn empty_diagnostics_returns_none() {
        assert!(format_diagnostics_context(&[]).is_none());
    }

    #[test]
    fn only_hints_returns_none() {
        let d = vec![diag(Severity::Hint, "hint stuff")];
        assert!(format_diagnostics_context(&d).is_none());
    }

    #[test]
    fn errors_and_warnings_are_included() {
        let ds = vec![
            diag(Severity::Error, "not found"),
            diag(Severity::Warning, "unused"),
            diag(Severity::Hint, "rename"),
        ];
        let out = format_diagnostics_context(&ds).unwrap();
        assert!(out.contains("1 error(es)"));
        assert!(out.contains("1 warning(s)"));
        assert!(out.contains("not found"));
        assert!(out.contains("unused"));
        assert!(!out.contains("rename"));
    }

    #[test]
    fn source_and_code_tags_rendered() {
        let ds = vec![diag(Severity::Error, "boom")];
        let out = format_diagnostics_context(&ds).unwrap();
        assert!(out.contains("[rustc]"));
        assert!(out.contains("(E0599)"));
    }

    #[test]
    fn truncation_reported_in_header() {
        let ds: Vec<LspDiagnostic> =
            (0..40).map(|i| diag(Severity::Error, &format!("err {i}"))).collect();
        let out = format_diagnostics_context(&ds).unwrap();
        assert!(out.contains("mostrando 30 de 40"));
    }
}
