//! Parser defensivo para extraer `StructuredOutput` de texto del assistant.
//!
//! Estrategia:
//! 1. Buscar bloques fenced ```json ... ``` (case-insensitive en la etiqueta).
//! 2. Si no hay bloques, intentar parsear el texto completo como JSON.
//! 3. Como ultimo recurso, recortar desde el primer `{` hasta el ultimo `}`.
//!
//! Para cada candidato se intenta deserializar a `StructuredOutput`. El primer
//! candidato valido gana. Si todos fallan, retorna `None` (fallback a texto
//! libre — el mensaje original del assistant sigue siendo el payload visible).

use super::StructuredOutput;

/// Lee un texto del assistant y devuelve la primera variante tipada detectada.
/// Retorna `None` si no se encontro un bloque valido (fallback silencioso).
pub fn detect_structured_output(text: &str) -> Option<StructuredOutput> {
    for candidate in candidates(text) {
        if let Ok(parsed) = serde_json::from_str::<StructuredOutput>(candidate.trim()) {
            return Some(parsed);
        }
    }
    None
}

/// Devuelve slices candidatos a contener JSON, en orden de preferencia:
/// bloques fenced primero, texto completo despues, substring `{..}` al final.
fn candidates(text: &str) -> Vec<&str> {
    let mut out = extract_fenced_blocks(text);
    let trimmed = text.trim();
    if !trimmed.is_empty() && !out.iter().any(|c| c.trim() == trimmed) {
        out.push(trimmed);
    }
    if let Some(sub) = brace_substring(text) {
        if !out.contains(&sub) {
            out.push(sub);
        }
    }
    out
}

/// Extrae el contenido de todos los bloques ```json ... ``` del texto.
/// El delimitador de apertura acepta lang "json" o sin lang; se descartan
/// bloques cuya lang no sea vacio ni "json" (ej: ```rust).
fn extract_fenced_blocks(text: &str) -> Vec<&str> {
    let mut out: Vec<&str> = Vec::new();
    let mut rest = text;
    while let Some(open_idx) = rest.find("```") {
        let after_open = &rest[open_idx + 3..];
        let Some(nl) = after_open.find('\n') else { break };
        let lang_raw = after_open[..nl].trim();
        let lang_ok = lang_raw.is_empty() || lang_raw.eq_ignore_ascii_case("json");
        let body = &after_open[nl + 1..];
        let Some(close_rel) = body.find("```") else { break };
        if lang_ok {
            out.push(&body[..close_rel]);
        }
        // avanzar cursor sobre el cierre
        let consumed = (open_idx + 3) + nl + 1 + close_rel + 3;
        if consumed >= rest.len() {
            break;
        }
        rest = &rest[consumed..];
    }
    out
}

/// Devuelve el substring desde el primer `{` hasta el ultimo `}` incluidos,
/// o `None` si no hay un par balanceado minimo.
fn brace_substring(text: &str) -> Option<&str> {
    let first = text.find('{')?;
    let last = text.rfind('}')?;
    if last <= first {
        return None;
    }
    Some(&text[first..=last])
}

#[cfg(test)]
mod tests {
    use super::super::{
        CodeAction, CodeActionKind, ComplianceResult, Severity, StructuredOutput, WorkflowPlan,
    };
    use super::*;

    #[test]
    fn returns_none_on_plain_text() {
        assert!(detect_structured_output("solo texto sin JSON").is_none());
    }

    #[test]
    fn parses_fenced_json_block_with_compliance() {
        let text = r#"Aqui esta el resultado:

```json
{
  "kind": "compliance_result",
  "factory": "net",
  "passed": true,
  "violations": [],
  "summary": "sin observaciones"
}
```

Listo."#;
        let out = detect_structured_output(text).expect("parsed");
        match out {
            StructuredOutput::ComplianceResult(ComplianceResult { factory, passed, .. }) => {
                assert_eq!(factory, "net");
                assert!(passed);
            }
            other => panic!("esperaba compliance_result, vino {other:?}"),
        }
    }

    #[test]
    fn parses_fenced_json_block_with_workflow_plan() {
        let text = r#"```json
{
  "kind": "workflow_plan",
  "title": "Deploy net",
  "factory": "net",
  "steps": [
    { "order": 1, "description": "build", "tool": "dotnet", "done": false },
    { "order": 2, "description": "test", "done": false }
  ]
}
```"#;
        let out = detect_structured_output(text).expect("parsed");
        let StructuredOutput::WorkflowPlan(WorkflowPlan { title, steps, .. }) = out else {
            panic!("variant");
        };
        assert_eq!(title, "Deploy net");
        assert_eq!(steps.len(), 2);
        assert_eq!(steps[0].tool.as_deref(), Some("dotnet"));
    }

    #[test]
    fn parses_bare_json_text() {
        let text = r#"{"kind":"code_action","action":"create","target":"src/x.rs","description":"add module"}"#;
        let out = detect_structured_output(text).expect("parsed");
        let StructuredOutput::CodeAction(CodeAction { action, target, .. }) = out else {
            panic!("variant");
        };
        assert!(matches!(action, CodeActionKind::Create));
        assert_eq!(target, "src/x.rs");
    }

    #[test]
    fn extracts_json_surrounded_by_prose() {
        let text = r#"Propongo esta accion:
{"kind":"code_action","action":"delete","target":"legacy.rs","description":"unused"}
Avisame si procedo."#;
        let out = detect_structured_output(text).expect("parsed");
        assert!(matches!(out, StructuredOutput::CodeAction(_)));
    }

    #[test]
    fn ignores_non_json_fenced_blocks() {
        let text = r#"```rust
let x = 1;
```

```json
{"kind":"code_action","action":"rename","target":"a.rs","description":"b"}
```"#;
        let out = detect_structured_output(text).expect("parsed");
        assert!(matches!(out, StructuredOutput::CodeAction(_)));
    }

    #[test]
    fn returns_none_when_kind_missing() {
        let text = r#"```json
{"factory": "net", "passed": true}
```"#;
        assert!(detect_structured_output(text).is_none());
    }

    #[test]
    fn returns_none_when_kind_unknown() {
        let text = r#"```json
{"kind":"unknown_variant","x":1}
```"#;
        assert!(detect_structured_output(text).is_none());
    }

    #[test]
    fn first_valid_block_wins_over_later_ones() {
        let text = r#"```json
{"kind":"code_action","action":"create","target":"first.rs","description":"d"}
```

y tambien

```json
{"kind":"code_action","action":"delete","target":"second.rs","description":"d"}
```"#;
        let out = detect_structured_output(text).expect("parsed");
        let StructuredOutput::CodeAction(CodeAction { target, .. }) = out else {
            panic!("variant");
        };
        assert_eq!(target, "first.rs");
    }

    #[test]
    fn severity_info_is_accepted_in_violation() {
        let text = r#"```json
{
  "kind": "compliance_result",
  "factory": "ang",
  "passed": false,
  "violations": [
    {"rule":"R1","severity":"info","message":"m"}
  ]
}
```"#;
        let out = detect_structured_output(text).expect("parsed");
        let StructuredOutput::ComplianceResult(ComplianceResult { violations, .. }) = out else {
            panic!("variant");
        };
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].severity, Severity::Info);
    }

    #[test]
    fn brace_substring_finds_embedded_object() {
        assert_eq!(brace_substring("pre {x} post"), Some("{x}"));
        assert_eq!(brace_substring("abc"), None);
        assert_eq!(brace_substring("}reverse{"), None);
    }
}
