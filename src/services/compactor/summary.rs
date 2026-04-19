//! Builder de summary categorizado para mensajes removidos.
//!
//! Adapta ideas de `claw-code/rust/crates/runtime/src/compact.rs::summarize_messages`:
//! - counts por rol
//! - tools usados (deduplicados)
//! - recent user requests (ultimos N)
//! - archivos referenciados (regex simple)
//! - pending work (heuristica por keywords)

use crate::state::chat_types::ChatRole;
use crate::state::ChatMessage;

/// Construye un summary markdown-compatible de los mensajes removidos.
/// Trunca al final si supera `budget_chars`.
pub fn build_summary(removed: &[ChatMessage], budget_chars: usize) -> String {
    if removed.is_empty() {
        return String::new();
    }
    let mut lines = Vec::new();
    lines.push("**Resumen de contexto compactado**".to_string());
    lines.push(counts_line(removed));

    let tools = collect_tool_names(removed);
    if !tools.is_empty() {
        lines.push(format!("- Tools usados: {}", tools.join(", ")));
    }

    let files = collect_file_refs(removed);
    if !files.is_empty() {
        lines.push(format!("- Archivos referenciados: {}", files.join(", ")));
    }

    let requests = recent_user_requests(removed, 3);
    if !requests.is_empty() {
        lines.push("- Ultimos requests del usuario:".to_string());
        for r in requests {
            lines.push(format!("  - {r}"));
        }
    }

    let pending = infer_pending_work(removed);
    if !pending.is_empty() {
        lines.push("- Trabajo pendiente/notas:".to_string());
        for p in pending {
            lines.push(format!("  - {p}"));
        }
    }

    let out = lines.join("\n");
    truncate_at_boundary(out, budget_chars)
}

fn counts_line(messages: &[ChatMessage]) -> String {
    let (mut u, mut a, mut t) = (0u32, 0u32, 0u32);
    for m in messages {
        match m.role {
            ChatRole::User => u += 1,
            ChatRole::Assistant => a += 1,
            ChatRole::Tool => t += 1,
            ChatRole::System => {}
        }
    }
    format!("- {} mensajes compactados (user={u}, assistant={a}, tool={t})", messages.len())
}

fn collect_tool_names(messages: &[ChatMessage]) -> Vec<String> {
    let mut names: Vec<String> =
        messages.iter().flat_map(|m| m.tool_calls.iter().map(|tc| tc.name.clone())).collect();
    names.sort();
    names.dedup();
    names.into_iter().take(10).collect()
}

fn recent_user_requests(messages: &[ChatMessage], limit: usize) -> Vec<String> {
    let mut requests: Vec<String> = messages
        .iter()
        .rev()
        .filter(|m| m.role == ChatRole::User)
        .map(|m| truncate_text(&m.content, 120))
        .filter(|s| !s.is_empty())
        .take(limit)
        .collect();
    requests.reverse();
    requests
}

fn infer_pending_work(messages: &[ChatMessage]) -> Vec<String> {
    const KEYWORDS: &[&str] = &["todo", "pendiente", "pending", "next", "follow up", "falta"];
    let mut out: Vec<String> = messages
        .iter()
        .rev()
        .filter_map(|m| first_line(&m.content))
        .filter(|line| {
            let low = line.to_lowercase();
            KEYWORDS.iter().any(|kw| low.contains(kw))
        })
        .take(3)
        .map(|line| truncate_text(line, 120))
        .collect();
    out.reverse();
    out
}

/// Extrae paths tipo `src/foo/bar.rs`, `Cargo.toml`, `ROADMAP.md`.
fn collect_file_refs(messages: &[ChatMessage]) -> Vec<String> {
    let mut seen: Vec<String> = Vec::new();
    for m in messages {
        for candidate in extract_paths(&m.content) {
            if !seen.iter().any(|s| s == &candidate) {
                seen.push(candidate);
            }
        }
        for tc in &m.tool_calls {
            for candidate in extract_paths(&tc.arguments) {
                if !seen.iter().any(|s| s == &candidate) {
                    seen.push(candidate);
                }
            }
        }
    }
    seen.into_iter().take(8).collect()
}

fn extract_paths(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    for word in
        s.split(|c: char| c.is_whitespace() || matches!(c, '"' | '\'' | '`' | ',' | '(' | ')'))
    {
        let trimmed = word.trim_matches(|c: char| matches!(c, '.' | ',' | ':' | ';' | '!' | '?'));
        if looks_like_path(trimmed) {
            out.push(trimmed.to_string());
        }
    }
    out
}

fn looks_like_path(s: &str) -> bool {
    if s.is_empty() || s.len() > 160 {
        return false;
    }
    let has_ext = s
        .rsplit_once('.')
        .map(|(_, ext)| {
            (1..=6).contains(&ext.len()) && ext.chars().all(|c| c.is_ascii_alphanumeric())
        })
        .unwrap_or(false);
    let has_sep = s.contains('/') || s.contains('\\');
    let ok_chars = s.chars().all(|c| c.is_ascii_alphanumeric() || "/_.-\\".contains(c));
    ok_chars && (has_ext || has_sep) && !s.starts_with("//") && !s.starts_with("http")
}

fn first_line(s: &str) -> Option<&str> {
    s.lines().find(|l| !l.trim().is_empty()).map(|l| l.trim())
}

fn truncate_text(s: &str, max: usize) -> String {
    let collapsed: String = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.chars().count() <= max {
        return collapsed;
    }
    let mut out: String = collapsed.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}

fn truncate_at_boundary(mut s: String, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        return s;
    }
    let mut end = 0;
    for (i, _) in s.char_indices().take(max_chars.saturating_sub(1)) {
        end = i;
    }
    // include the last char boundary
    while !s.is_char_boundary(end) && end > 0 {
        end -= 1;
    }
    s.truncate(end);
    s.push('…');
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::chat_types::{ToolCall, ToolCallStatus};

    fn user(s: &str) -> ChatMessage {
        ChatMessage::new(ChatRole::User, s.into())
    }
    fn assistant_with_tool(name: &str, args: &str) -> ChatMessage {
        let mut m = ChatMessage::new(ChatRole::Assistant, String::new());
        m.tool_calls.push(ToolCall {
            id: "id1".into(),
            name: name.into(),
            arguments: args.into(),
            status: ToolCallStatus::Success,
            duration_ms: Some(1),
        });
        m
    }

    #[test]
    fn empty_input_yields_empty() {
        assert_eq!(build_summary(&[], 1000), "");
    }

    #[test]
    fn includes_counts_and_tools() {
        let msgs = vec![user("hola"), assistant_with_tool("Read", "src/main.rs")];
        let out = build_summary(&msgs, 1000);
        assert!(out.contains("2 mensajes"));
        assert!(out.contains("Tools usados: Read"));
        assert!(out.contains("src/main.rs"));
    }

    #[test]
    fn extracts_recent_user_requests() {
        let msgs = vec![user("compila el proyecto"), user("ahora corre tests"), user("commit")];
        let out = build_summary(&msgs, 1000);
        assert!(out.contains("compila"));
        assert!(out.contains("tests"));
        assert!(out.contains("commit"));
    }

    #[test]
    fn detects_pending_work() {
        let msgs =
            vec![user("TODO: agregar test para boundary"), user("falta documentar el roadmap")];
        let out = build_summary(&msgs, 1000);
        assert!(out.contains("Trabajo pendiente"));
    }

    #[test]
    fn truncates_to_budget() {
        let huge = "x".repeat(5000);
        let msgs = vec![user(&huge)];
        let out = build_summary(&msgs, 200);
        assert!(out.chars().count() <= 200);
        assert!(out.ends_with('…'));
    }

    #[test]
    fn path_detection_ignores_urls_and_short_words() {
        assert!(looks_like_path("src/foo.rs"));
        assert!(looks_like_path("Cargo.toml"));
        assert!(!looks_like_path("https://x.com"));
        assert!(!looks_like_path("hola"));
    }
}
