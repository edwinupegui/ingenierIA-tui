//! Redactor de secretos para audit log.
//!
//! Detecta y reemplaza patrones tipicos de secretos (API keys, tokens,
//! passwords, bearer) antes de escribir al log. Pensado defensivamente: es
//! mejor redactar de mas que filtrar un secreto real.

/// Reemplaza secretos detectados por `[REDACTED]`. Opera linea por linea.
///
/// Patrones cubiertos:
/// - `sk-...`, `sk_live_...`, `sk_test_...` (Stripe/OpenAI/Anthropic)
/// - `ghp_...`, `ghs_...`, `gho_...`, `github_pat_...` (GitHub)
/// - `Bearer <token>` en headers
/// - `api_key = "..."`, `token: "..."`, `password: "..."` en configs
/// - AWS: `AKIA[A-Z0-9]{16}`
/// - Anthropic: `sk-ant-...`
pub fn redact_secrets(input: &str) -> String {
    let mut out = input.to_string();

    // sk-... tokens (Anthropic, OpenAI, Stripe) — 20+ chars
    out = redact_prefix_token(&out, "sk-ant-");
    out = redact_prefix_token(&out, "sk_live_");
    out = redact_prefix_token(&out, "sk_test_");
    out = redact_prefix_token(&out, "sk-");

    // GitHub tokens
    out = redact_prefix_token(&out, "github_pat_");
    out = redact_prefix_token(&out, "ghp_");
    out = redact_prefix_token(&out, "ghs_");
    out = redact_prefix_token(&out, "gho_");
    out = redact_prefix_token(&out, "ghu_");
    out = redact_prefix_token(&out, "ghr_");

    // AWS Access Key ID
    out = redact_aws_akia(&out);

    // Bearer tokens
    out = redact_bearer(&out);

    // key-value assignments
    out = redact_kv_secret(&out, "api_key");
    out = redact_kv_secret(&out, "apikey");
    out = redact_kv_secret(&out, "token");
    out = redact_kv_secret(&out, "password");
    out = redact_kv_secret(&out, "secret");

    out
}

/// Reemplaza tokens con prefijo + suficientes chars alfanumericos (>=20).
fn redact_prefix_token(input: &str, prefix: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(pos) = rest.find(prefix) {
        out.push_str(&rest[..pos]);
        let tail = &rest[pos..];
        let token_end = tail
            .char_indices()
            .skip_while(|(i, _)| *i < prefix.len())
            .take_while(|(_, c)| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
            .last()
            .map(|(i, c)| i + c.len_utf8())
            .unwrap_or(prefix.len());
        if token_end >= prefix.len() + 20 {
            out.push_str("[REDACTED]");
        } else {
            out.push_str(&tail[..token_end]);
        }
        rest = &tail[token_end..];
    }
    out.push_str(rest);
    out
}

/// `AKIA[A-Z0-9]{16}` — AWS Access Key ID.
fn redact_aws_akia(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if i + 20 <= bytes.len() && &bytes[i..i + 4] == b"AKIA" {
            let is_valid =
                bytes[i + 4..i + 20].iter().all(|b| b.is_ascii_uppercase() || b.is_ascii_digit());
            let is_right = i + 20 == bytes.len() || !bytes[i + 20].is_ascii_alphanumeric();
            let is_left = i == 0 || !bytes[i - 1].is_ascii_alphanumeric();
            if is_valid && is_right && is_left {
                out.push_str("[REDACTED]");
                i += 20;
                continue;
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

/// Redacta `Bearer <token>` en headers.
fn redact_bearer(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(pos) = rest.to_lowercase().find("bearer ") {
        // Posicion en `rest` real
        out.push_str(&rest[..pos + "bearer ".len()]);
        let tail = &rest[pos + "bearer ".len()..];
        let token_end = tail
            .char_indices()
            .take_while(|(_, c)| c.is_ascii_alphanumeric() || *c == '_' || *c == '-' || *c == '.')
            .last()
            .map(|(i, c)| i + c.len_utf8())
            .unwrap_or(0);
        if token_end > 0 {
            out.push_str("[REDACTED]");
            rest = &tail[token_end..];
        } else {
            rest = tail;
        }
    }
    out.push_str(rest);
    out
}

/// Redacta `<key> = "..."` o `<key>: "..."` o `<key>=...` sin quotes.
fn redact_kv_secret(input: &str, key: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    let lower_key = key.to_lowercase();
    loop {
        let lower_rest = rest.to_lowercase();
        let Some(pos) = lower_rest.find(&lower_key) else {
            break;
        };
        // Verificar que sea boundary
        let is_left_boundary = pos == 0
            || !rest.as_bytes()[pos - 1].is_ascii_alphanumeric()
                && rest.as_bytes()[pos - 1] != b'_';
        if !is_left_boundary {
            out.push_str(&rest[..pos + key.len()]);
            rest = &rest[pos + key.len()..];
            continue;
        }
        let after_key_pos = pos + key.len();
        if after_key_pos >= rest.len() {
            break;
        }
        let after = &rest[after_key_pos..];
        let mut chars = after.char_indices();
        // Saltar espacios/comillas + `:` o `=`. Ignora `"` y `'` que
        // pueden aparecer cuando la clave esta quoted en JSON: `"api_key": ...`.
        let sep_pos = loop {
            let Some((i, c)) = chars.next() else {
                break None;
            };
            if c == ':' || c == '=' {
                break Some(i);
            }
            if !c.is_whitespace() && c != '"' && c != '\'' {
                break None;
            }
        };
        let Some(sep_pos) = sep_pos else {
            out.push_str(&rest[..after_key_pos]);
            rest = after;
            continue;
        };
        out.push_str(&rest[..after_key_pos + sep_pos + 1]);
        let value_area = &after[sep_pos + 1..];
        let value_end = redact_value_area(&mut out, value_area);
        rest = &value_area[value_end..];
    }
    out.push_str(rest);
    out
}

/// Lee el valor despues del `=` o `:` y escribe `[REDACTED]` al output.
/// Retorna el byte offset consumido en `area`.
fn redact_value_area(out: &mut String, area: &str) -> usize {
    let trimmed_start = area.find(|c: char| !c.is_whitespace()).unwrap_or(area.len());
    out.push_str(&area[..trimmed_start]);
    let remainder = &area[trimmed_start..];
    let (value_len, quoted) = if let Some(rest) = remainder.strip_prefix('"') {
        let closing = rest.find('"').map(|i| i + 2).unwrap_or(remainder.len());
        (closing, true)
    } else if let Some(rest) = remainder.strip_prefix('\'') {
        let closing = rest.find('\'').map(|i| i + 2).unwrap_or(remainder.len());
        (closing, true)
    } else {
        let end = remainder
            .find(|c: char| c.is_whitespace() || c == ',' || c == ';')
            .unwrap_or(remainder.len());
        (end, false)
    };
    // Solo redactar si hay algo que parezca secreto (>= 8 chars o con quotes)
    if quoted || value_len >= 8 {
        if quoted {
            out.push(remainder.as_bytes()[0] as char);
            out.push_str("[REDACTED]");
            out.push(remainder.as_bytes()[0] as char);
        } else {
            out.push_str("[REDACTED]");
        }
    } else {
        out.push_str(&remainder[..value_len]);
    }
    trimmed_start + value_len
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_anthropic_key() {
        let input = "using key sk-ant-api03-abc123def456ghi789jkl012mno345 for request";
        let out = redact_secrets(input);
        assert!(out.contains("[REDACTED]"));
        assert!(!out.contains("sk-ant-api03"));
    }

    #[test]
    fn redacts_short_sk_only_when_long_enough() {
        // Menos de 20 chars tras prefijo → no redactar (falso positivo)
        let input = "variable sk-foo";
        let out = redact_secrets(input);
        assert!(out.contains("sk-foo"));
    }

    #[test]
    fn redacts_github_pat() {
        let input = "token ghp_abc123def456ghi789jkl012 here";
        let out = redact_secrets(input);
        assert!(out.contains("[REDACTED]"));
    }

    #[test]
    fn redacts_bearer_in_authorization_header() {
        let input = "Authorization: Bearer eyJhbGc.payload.signature";
        let out = redact_secrets(input);
        assert!(out.contains("[REDACTED]"));
        assert!(!out.contains("eyJhbGc"));
    }

    #[test]
    fn redacts_api_key_in_json() {
        let input = r#"{"api_key": "abc123super-secret-value"}"#;
        let out = redact_secrets(input);
        assert!(out.contains("[REDACTED]"));
        assert!(!out.contains("abc123super"));
    }

    #[test]
    fn redacts_aws_akia() {
        let input = "aws key AKIAIOSFODNN7EXAMPLE configured";
        let out = redact_secrets(input);
        assert!(out.contains("[REDACTED]"));
        assert!(!out.contains("AKIAIOSFODNN7"));
    }

    #[test]
    fn preserves_non_secret_content() {
        let input = "Hello, world! This is a normal log line.";
        let out = redact_secrets(input);
        assert_eq!(out, input);
    }

    #[test]
    fn redacts_password_kv() {
        let input = "password = \"hunter2secret\"";
        let out = redact_secrets(input);
        assert!(out.contains("[REDACTED]"));
        assert!(!out.contains("hunter2"));
    }
}
