//! Parser/serializer de frontmatter markdown para memorias.
//!
//! Formato minimo (3 campos fijos, sin YAML completo):
//!
//! ```markdown
//! ---
//! name: My Role
//! description: senior rust engineer
//! type: user
//! ---
//!
//! Body text here.
//! ```
//!
//! Decisiones:
//! - No usar `serde_yaml` (dependencia pesada) — parser line-based.
//! - Valores se recortan (trim); `name`/`description` no se permite vacio.
//! - `type` desconocido → error (mejor fallar temprano que corromper store).

use super::types::{MemoryFrontmatter, MemoryType};

const DELIM: &str = "---";

/// Parsea un archivo `.md` con frontmatter. Retorna `(frontmatter, body)`.
pub fn parse_memory(raw: &str) -> Result<(MemoryFrontmatter, String), String> {
    let mut lines = raw.lines();
    let first = lines.next().ok_or("archivo vacio")?;
    if first.trim() != DELIM {
        return Err("falta frontmatter inicial '---'".into());
    }

    let mut name = None;
    let mut description = None;
    let mut memory_type = None;
    let mut found_end = false;
    let mut body_lines = Vec::new();

    for line in lines.by_ref() {
        if line.trim() == DELIM {
            found_end = true;
            break;
        }
        if let Some((key, value)) = split_kv(line) {
            match key {
                "name" => name = Some(value),
                "description" => description = Some(value),
                "type" => {
                    memory_type = Some(
                        MemoryType::from_label(&value)
                            .ok_or_else(|| format!("tipo de memoria invalido: '{value}'"))?,
                    );
                }
                _ => {} // claves desconocidas se ignoran (forward-compat)
            }
        }
    }

    if !found_end {
        return Err("frontmatter sin cierre '---'".into());
    }

    // El resto del archivo es body (preservar saltos intermedios).
    for line in lines {
        body_lines.push(line);
    }
    let body = body_lines.join("\n").trim().to_string();

    let frontmatter = MemoryFrontmatter {
        name: require_non_empty("name", name)?,
        description: require_non_empty("description", description)?,
        memory_type: memory_type.ok_or("falta campo 'type' en frontmatter")?,
    };
    Ok((frontmatter, body))
}

/// Serializa a markdown con frontmatter en el formato canonico.
pub fn serialize_memory(fm: &MemoryFrontmatter, body: &str) -> String {
    let mut out = String::with_capacity(body.len() + 128);
    out.push_str("---\n");
    out.push_str(&format!("name: {}\n", fm.name));
    out.push_str(&format!("description: {}\n", fm.description));
    out.push_str(&format!("type: {}\n", fm.memory_type.label()));
    out.push_str("---\n\n");
    out.push_str(body.trim());
    out.push('\n');
    out
}

/// `"key: value"` → `("key", "value")`. Retorna `None` si no hay `:`.
fn split_kv(line: &str) -> Option<(&str, String)> {
    let colon = line.find(':')?;
    let key = line[..colon].trim();
    let value = line[colon + 1..].trim().to_string();
    if key.is_empty() {
        return None;
    }
    Some((key, value))
}

fn require_non_empty(field: &str, value: Option<String>) -> Result<String, String> {
    match value {
        Some(s) if !s.is_empty() => Ok(s),
        _ => Err(format!("campo '{field}' ausente o vacio")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_happy_path() {
        let raw =
            "---\nname: Role\ndescription: senior\ntype: user\n---\n\nBody line 1\nBody line 2\n";
        let (fm, body) = parse_memory(raw).unwrap();
        assert_eq!(fm.name, "Role");
        assert_eq!(fm.description, "senior");
        assert_eq!(fm.memory_type, MemoryType::User);
        assert_eq!(body, "Body line 1\nBody line 2");
    }

    #[test]
    fn parse_missing_opening_delim_errors() {
        let raw = "name: x\ntype: user\n";
        assert!(parse_memory(raw).is_err());
    }

    #[test]
    fn parse_unclosed_frontmatter_errors() {
        let raw = "---\nname: x\ntype: user\n";
        assert!(parse_memory(raw).is_err());
    }

    #[test]
    fn parse_invalid_type_errors() {
        let raw = "---\nname: x\ndescription: d\ntype: wrong\n---\nbody\n";
        let err = parse_memory(raw).unwrap_err();
        assert!(err.contains("invalido"));
    }

    #[test]
    fn parse_missing_field_errors() {
        let raw = "---\nname: x\ntype: user\n---\nbody\n";
        assert!(parse_memory(raw).is_err());
    }

    #[test]
    fn serialize_roundtrip() {
        let fm = MemoryFrontmatter {
            name: "Role".into(),
            description: "desc".into(),
            memory_type: MemoryType::Feedback,
        };
        let out = serialize_memory(&fm, "body line\n");
        let (fm2, body) = parse_memory(&out).unwrap();
        assert_eq!(fm, fm2);
        assert_eq!(body, "body line");
    }

    #[test]
    fn parse_ignores_unknown_keys() {
        let raw = "---\nname: x\ndescription: d\ntype: user\nfoo: bar\n---\nbody\n";
        let (fm, _) = parse_memory(raw).unwrap();
        assert_eq!(fm.name, "x");
    }
}
