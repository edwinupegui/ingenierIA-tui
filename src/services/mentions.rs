//! @mention resolution: parse `@kind:name` en el input del chat y resuelve
//! el contenido del documento via MCP `get_document` para inyectarlo al
//! prompt (P2.4). Sin efectos secundarios: toma el texto + pool y devuelve
//! texto augmentado + refs.
//!
//! Kinds soportados: skill, agent, workflow, adr, policy, command.
//! El kind "command" se resuelve localmente (slash_commands registry) en un
//! paso futuro — por ahora devuelve mensaje de fallback.
use std::sync::Arc;

use crate::services::mcp::McpPool;
use crate::state::DocReference;

/// Resultado de parsear el input buscando @mentions.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedMention {
    /// Tipo canónico: "skill" | "agent" | "workflow" | "adr" | "policy" | "command".
    pub kind: String,
    /// Nombre tal como lo tipeó el usuario.
    pub name: String,
    /// Ocurrencia literal en el input (para poder construir secciones
    /// agrupadas y mostrar qué se resolvió).
    pub raw: String,
}

/// Resultado de resolver todas las mentions: texto augmentado y refs.
#[derive(Debug, Clone)]
pub struct ResolvedPrompt {
    pub augmented_text: String,
    pub refs: Vec<DocReference>,
}

/// Extrae @mentions del input. Deduplica por (kind, name) manteniendo la
/// primera aparición. Vacío si no hay ninguna.
pub fn parse_mentions(input: &str) -> Vec<ParsedMention> {
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'@' {
            // Verifica que no sea un email: el char anterior debe ser start
            // o no-word, y tras el nombre debe haber `:`.
            let prev_ok = i == 0 || !bytes[i - 1].is_ascii_alphanumeric();
            if prev_ok {
                if let Some((kind, name, end)) = parse_mention_from(&input[i..]) {
                    let key = format!("{kind}:{name}");
                    if seen.insert(key.clone()) {
                        out.push(ParsedMention {
                            kind: kind.to_string(),
                            name: name.to_string(),
                            raw: format!("@{kind}:{name}"),
                        });
                    }
                    i += end;
                    continue;
                }
            }
        }
        i += 1;
    }
    out
}

/// Intenta parsear `@kind:name` desde el inicio del slice. Retorna
/// `(kind, name, bytes_consumed_incluyendo_@)` si coincide.
fn parse_mention_from(s: &str) -> Option<(&str, &str, usize)> {
    let rest = s.strip_prefix('@')?;
    let colon = rest.find(':')?;
    let kind = &rest[..colon];
    if !is_supported_kind(kind) {
        return None;
    }
    let after_colon = &rest[colon + 1..];
    let name_end = after_colon
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '_'))
        .unwrap_or(after_colon.len());
    if name_end == 0 {
        return None;
    }
    let name = &after_colon[..name_end];
    let consumed = 1 + colon + 1 + name_end; // '@' + kind + ':' + name
    Some((kind, name, consumed))
}

fn is_supported_kind(k: &str) -> bool {
    matches!(k, "skill" | "agent" | "workflow" | "adr" | "policy" | "command")
}

/// Para un set de mentions, resuelve su contenido via MCP y construye un
/// prompt augmentado con secciones `### Context from @kind:name`. Mentions
/// que fallan se marcan inline como `(no se pudo cargar: ...)` sin abortar
/// el flujo.
pub async fn resolve_prompt(
    pool: &Arc<McpPool>,
    factory: &str,
    user_input: &str,
    mentions: &[ParsedMention],
) -> ResolvedPrompt {
    if mentions.is_empty() {
        return ResolvedPrompt { augmented_text: user_input.to_string(), refs: Vec::new() };
    }
    let mut sections = Vec::new();
    let mut refs = Vec::new();
    for m in mentions {
        let (content, resolved_ok) = fetch_mention(pool, factory, m).await;
        sections.push(format_section(m, &content, resolved_ok));
        if resolved_ok {
            refs.push(DocReference {
                uri: format!("ingenieria://{}/{}/{}", m.kind, factory, m.name),
                kind: m.kind.clone(),
                name: m.name.clone(),
                bytes: content.len(),
            });
        }
    }
    let augmented = format!(
        "{}\n\n---\n\n### Mensaje del usuario\n\n{}",
        sections.join("\n\n---\n\n"),
        user_input
    );
    ResolvedPrompt { augmented_text: augmented, refs }
}

/// Llama al tool MCP `get_document`. Retorna (contenido, ok).
async fn fetch_mention(pool: &Arc<McpPool>, factory: &str, m: &ParsedMention) -> (String, bool) {
    if m.kind == "command" {
        return (format!("(command `{}` se ejecuta localmente, no se inyecta)", m.name), false);
    }
    let args = serde_json::json!({
        "type": m.kind,
        "factory": factory,
        "name": m.name,
    });
    match pool.call_tool("get_document", args).await {
        Ok(text) if !text.trim().is_empty() => (text, true),
        Ok(_) => ("(documento vacío)".to_string(), false),
        Err(e) => (format!("(error cargando: {e})"), false),
    }
}

fn format_section(m: &ParsedMention, content: &str, ok: bool) -> String {
    let status = if ok { "" } else { " [FAIL]" };
    format!("### Context from @{}:{}{}\n\n{}", m.kind, m.name, status, content.trim())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_single_skill_mention() {
        let out = parse_mentions("usa @skill:auth-flow para esto");
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].kind, "skill");
        assert_eq!(out[0].name, "auth-flow");
    }

    #[test]
    fn parses_multiple_mentions_dedup() {
        let out = parse_mentions("@skill:a @agent:b revisa @skill:a de nuevo con @policy:security");
        assert_eq!(out.len(), 3);
        assert_eq!(out[0].kind, "skill");
        assert_eq!(out[1].kind, "agent");
        assert_eq!(out[2].kind, "policy");
    }

    #[test]
    fn ignores_email_like_at() {
        let out = parse_mentions("contacto foo@bar.com no es mention");
        assert_eq!(out.len(), 0);
    }

    #[test]
    fn ignores_unsupported_kind() {
        let out = parse_mentions("@foo:bar no es valido @skill:x si");
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].name, "x");
    }

    #[test]
    fn stops_name_at_non_word_char() {
        let out = parse_mentions("@skill:auth-flow.");
        assert_eq!(out[0].name, "auth-flow");
    }

    #[test]
    fn at_without_colon_ignored() {
        let out = parse_mentions("@skill solo");
        assert_eq!(out.len(), 0);
    }

    #[test]
    fn format_section_preserves_content() {
        let m = ParsedMention { kind: "skill".into(), name: "x".into(), raw: "@skill:x".into() };
        let s = format_section(&m, "  contenido  \n", true);
        assert!(s.contains("### Context from @skill:x"));
        assert!(s.contains("contenido"));
        assert!(!s.contains("[FAIL]"));
    }

    #[test]
    fn format_section_marks_failed() {
        let m = ParsedMention { kind: "skill".into(), name: "x".into(), raw: "@skill:x".into() };
        let s = format_section(&m, "", false);
        assert!(s.contains("[FAIL]"));
    }
}
