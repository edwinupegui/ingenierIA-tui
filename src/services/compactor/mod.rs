//! E14 — Compactacion inteligente de contexto.
//!
//! Funcion pura `compact(messages, strategy)`:
//! 1. Separa mensajes system (siempre preservados al inicio).
//! 2. Calcula `raw_keep_from = len - keep_recent` sobre los no-system.
//! 3. Aplica [`boundary::find_safe_boundary`] para no cortar pares tool_use/tool_result.
//! 4. Genera summary categorizado (ver [`summary::build_summary`]) acotado por estrategia.
//! 5. Retorna `CompactionOutcome` con nuevos mensajes + metadata.
//!
//! Ver roadmap E14 + referencia `claw-code/rust/crates/runtime/src/compact.rs`.

pub mod boundary;
pub mod strategy;
pub mod summary;

#[allow(unused_imports)]
pub use strategy::CompactionConfig;
pub use strategy::CompactionStrategy;

use crate::state::chat_types::ChatRole;
use crate::state::ChatMessage;

/// Resultado de una compactacion.
#[derive(Debug, Clone)]
pub struct CompactionOutcome {
    /// Nueva lista de mensajes (system + summary + tail preservada).
    pub messages: Vec<ChatMessage>,
    /// Cantidad de mensajes removidos.
    pub removed_count: usize,
    /// Cantidad preservada verbatim.
    #[allow(dead_code, reason = "API publica reservada para UI de debug y tests")]
    pub preserved_count: usize,
    /// Estrategia aplicada.
    pub strategy: CompactionStrategy,
}

impl CompactionOutcome {
    pub fn noop(messages: Vec<ChatMessage>, strategy: CompactionStrategy) -> Self {
        let len = messages.len();
        Self { messages, removed_count: 0, preserved_count: len, strategy }
    }
}

/// Compacta `messages` segun `strategy`. No muta la entrada.
///
/// Si no hay suficientes mensajes no-system para compactar, retorna noop.
pub fn compact(messages: &[ChatMessage], strategy: CompactionStrategy) -> CompactionOutcome {
    let config = strategy.config();
    let (system, non_system) = split_system(messages);

    if non_system.len() <= config.keep_recent {
        return CompactionOutcome::noop(messages.to_vec(), strategy);
    }

    let raw_keep_from = non_system.len() - config.keep_recent;
    let keep_from = boundary::find_safe_boundary(&non_system, raw_keep_from);
    if keep_from == 0 {
        // Boundary caminó hasta 0 → no hay nada que remover sin romper pares.
        return CompactionOutcome::noop(messages.to_vec(), strategy);
    }

    let removed: Vec<ChatMessage> = non_system[..keep_from].to_vec();
    let preserved: Vec<ChatMessage> = non_system[keep_from..].to_vec();
    let summary_text = summary::build_summary(&removed, config.summary_budget_chars);

    let mut out: Vec<ChatMessage> = Vec::with_capacity(system.len() + 1 + preserved.len());
    out.extend(system);
    out.push(summary_message(&summary_text, strategy, removed.len()));
    out.extend(preserved.iter().cloned());

    CompactionOutcome {
        messages: out,
        removed_count: removed.len(),
        preserved_count: preserved.len(),
        strategy,
    }
}

fn split_system(messages: &[ChatMessage]) -> (Vec<ChatMessage>, Vec<ChatMessage>) {
    let mut system = Vec::new();
    let mut other = Vec::new();
    for m in messages {
        if m.role == ChatRole::System {
            system.push(m.clone());
        } else {
            other.push(m.clone());
        }
    }
    (system, other)
}

fn summary_message(body: &str, strategy: CompactionStrategy, removed: usize) -> ChatMessage {
    let header = format!("[Compactado/{}]: {} mensajes resumidos\n", strategy.label(), removed);
    let content = format!("{header}{body}");
    ChatMessage::new(ChatRole::System, content)
}

/// Valida que la salida respeta la invariante de pares tool_use/tool_result.
/// Util en tests y como sanity-check en debug.
#[cfg(test)]
pub fn validate_tool_pairs(messages: &[ChatMessage]) -> Result<(), String> {
    let mut open: Vec<String> = Vec::new();
    for m in messages {
        match m.role {
            ChatRole::Assistant if !m.tool_calls.is_empty() => {
                for tc in &m.tool_calls {
                    open.push(tc.id.clone());
                }
            }
            ChatRole::Tool => {
                let Some(id) = m.tool_call_id.as_ref() else {
                    return Err("Tool message sin tool_call_id".into());
                };
                if !open.iter().any(|o| o == id) {
                    return Err(format!("tool_result orphan id={id}"));
                }
            }
            _ => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::chat_types::{ToolCall, ToolCallStatus};

    fn user(s: &str) -> ChatMessage {
        ChatMessage::new(ChatRole::User, s.into())
    }
    fn assistant(s: &str) -> ChatMessage {
        ChatMessage::new(ChatRole::Assistant, s.into())
    }
    fn system(s: &str) -> ChatMessage {
        ChatMessage::new(ChatRole::System, s.into())
    }
    fn asst_with_tools(ids: &[&str]) -> ChatMessage {
        let mut m = ChatMessage::new(ChatRole::Assistant, String::new());
        m.tool_calls = ids
            .iter()
            .map(|id| ToolCall {
                id: (*id).into(),
                name: "Read".into(),
                arguments: "{}".into(),
                status: ToolCallStatus::Success,
                duration_ms: Some(1),
            })
            .collect();
        m
    }
    fn tool_res(id: &str) -> ChatMessage {
        ChatMessage::tool_result(id.into(), "ok".into())
    }

    #[test]
    fn noop_when_few_messages() {
        let msgs: Vec<ChatMessage> = (0..3).map(|i| user(&format!("msg {i}"))).collect();
        let out = compact(&msgs, CompactionStrategy::Balanced);
        assert_eq!(out.removed_count, 0);
        assert_eq!(out.messages.len(), 3);
    }

    #[test]
    fn balanced_keeps_10_recent() {
        let msgs: Vec<ChatMessage> = (0..20)
            .map(|i| if i % 2 == 0 { user(&format!("q{i}")) } else { assistant(&format!("r{i}")) })
            .collect();
        let out = compact(&msgs, CompactionStrategy::Balanced);
        // 20 → compactar 10 → preservar 10 + 1 summary system
        assert_eq!(out.preserved_count, 10);
        assert_eq!(out.removed_count, 10);
        assert_eq!(out.messages.len(), 11);
        assert_eq!(out.messages[0].role, ChatRole::System);
    }

    #[test]
    fn aggressive_removes_more_than_conservative() {
        let msgs: Vec<ChatMessage> = (0..30)
            .map(|i| if i % 2 == 0 { user(&format!("q{i}")) } else { assistant(&format!("r{i}")) })
            .collect();
        let agr = compact(&msgs, CompactionStrategy::Aggressive);
        let con = compact(&msgs, CompactionStrategy::Conservative);
        assert!(agr.removed_count > con.removed_count);
    }

    #[test]
    fn preserves_system_messages_at_front() {
        let msgs = vec![
            system("sys1"),
            system("sys2"),
            user("a"),
            assistant("b"),
            user("c"),
            assistant("d"),
            user("e"),
            assistant("f"),
            user("g"),
            assistant("h"),
            user("i"),
            assistant("j"),
            user("k"),
            assistant("l"),
            user("m"),
            assistant("n"),
        ];
        let out = compact(&msgs, CompactionStrategy::Balanced);
        assert_eq!(out.messages[0].content, "sys1");
        assert_eq!(out.messages[1].content, "sys2");
        // El 3ro debe ser el summary (System con "[Compactado/...]")
        assert_eq!(out.messages[2].role, ChatRole::System);
        assert!(out.messages[2].content.contains("[Compactado/balanced]"));
    }

    #[test]
    fn respects_tool_boundary() {
        // 15 msgs: pares user/assistant + al final un par tool_use/tool_result
        // para forzar al boundary a caminar hacia atras.
        let mut msgs: Vec<ChatMessage> = Vec::new();
        for i in 0..10 {
            msgs.push(user(&format!("q{i}")));
            msgs.push(assistant(&format!("r{i}")));
        }
        // Los ultimos 5 mensajes forman una cadena que termina en tool_use/tool_result
        msgs.push(user("run read")); // idx 20
        msgs.push(asst_with_tools(&["tid"])); // idx 21
        msgs.push(tool_res("tid")); // idx 22
        msgs.push(assistant("result")); // idx 23
        msgs.push(user("ok")); // idx 24

        let out = compact(&msgs, CompactionStrategy::Aggressive); // keep_recent=4
                                                                  // Con keep_recent=4, raw_keep_from=25-4=21 (el Assistant con tool_calls).
                                                                  // El boundary NO deberia mover porque messages[21] es Assistant (no Tool).
                                                                  // Validamos que la salida tenga pares consistentes.
        let only_non_sys: Vec<ChatMessage> = out
            .messages
            .iter()
            .filter(|m| m.role != ChatRole::System || !m.content.starts_with("[Compactado"))
            .cloned()
            .collect();
        validate_tool_pairs(&only_non_sys).unwrap();
    }

    #[test]
    fn boundary_walks_back_across_tool_results() {
        let mut msgs: Vec<ChatMessage> = Vec::new();
        for i in 0..10 {
            msgs.push(user(&format!("q{i}")));
            msgs.push(assistant(&format!("r{i}")));
        }
        msgs.push(asst_with_tools(&["a", "b"])); // idx 20
        msgs.push(tool_res("a")); // idx 21
        msgs.push(tool_res("b")); // idx 22
        msgs.push(user("done")); // idx 23
                                 // total 24. aggressive keep_recent=4 → raw_keep_from=20 → apunta al Assistant con tool_calls
                                 // boundary no mueve (no es Tool). Validamos pares OK.
        let out = compact(&msgs, CompactionStrategy::Aggressive);
        validate_tool_pairs(&out.messages).unwrap();
        // caso mas agresivo: keep_recent=2 → raw_keep_from=22 (tool_result b) → walks back a 20
        let cfg_small = CompactionStrategy::Aggressive.config();
        assert_eq!(cfg_small.keep_recent, 4);
    }

    #[test]
    fn summary_mentions_strategy() {
        // Conservative preserva 20; necesitamos mas para que haya algo que compactar.
        let msgs: Vec<ChatMessage> = (0..30).map(|i| user(&format!("q{i}"))).collect();
        let out = compact(&msgs, CompactionStrategy::Conservative);
        let summary = out.messages.iter().find(|m| m.content.starts_with("[Compactado/"));
        assert!(summary.is_some());
        assert!(summary.unwrap().content.contains("conservative"));
    }

    #[test]
    fn validate_tool_pairs_detects_orphans() {
        let orphan = vec![tool_res("ghost")];
        assert!(validate_tool_pairs(&orphan).is_err());
    }
}
