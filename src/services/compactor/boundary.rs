//! Boundary detection para evitar cortar pares tool_use / tool_result.
//!
//! Si el primer mensaje preservado es un `Tool` message (tool_result) pero el
//! `Assistant` que lo origino (tool_calls) fue marcado para remover, el
//! provider puede rechazar la request (OpenAI: "tool message must follow
//! assistant with tool_calls"; Anthropic: tool_result orphan).
//!
//! Esta funcion camina hacia atras hasta encontrar un punto seguro.

use crate::state::chat_types::ChatRole;
use crate::state::ChatMessage;

/// Retorna un indice de corte seguro `k` tal que `messages[k..]` preserva
/// pares tool_use/tool_result completos.
///
/// Si `raw_keep_from` cae dentro de una cadena de tool_results, se camina hacia
/// atras hasta incluir el `Assistant` con los `tool_calls` correspondientes.
pub fn find_safe_boundary(messages: &[ChatMessage], raw_keep_from: usize) -> usize {
    let len = messages.len();
    if raw_keep_from >= len {
        return len;
    }
    let mut k = raw_keep_from;

    // Walk back mientras el primer preservado sea un tool_result.
    while k > 0 && matches!(messages[k].role, ChatRole::Tool) {
        k -= 1;
    }

    // Si ahora apuntamos a un Assistant con tool_calls, lo incluimos — ya estamos bien.
    // Si apuntamos a User/System/Assistant-sin-tool_calls, tambien es seguro.
    k
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::chat_types::ToolCall;

    fn msg(role: ChatRole) -> ChatMessage {
        ChatMessage::new(role, "x".into())
    }

    fn assistant_with_tools(ids: &[&str]) -> ChatMessage {
        let mut m = ChatMessage::new(ChatRole::Assistant, String::new());
        m.tool_calls = ids
            .iter()
            .map(|id| ToolCall {
                id: (*id).into(),
                name: "n".into(),
                arguments: "{}".into(),
                status: crate::state::chat_types::ToolCallStatus::Success,
                duration_ms: Some(1),
            })
            .collect();
        m
    }

    fn tool_result(id: &str) -> ChatMessage {
        ChatMessage::tool_result(id.into(), "ok".into())
    }

    #[test]
    fn boundary_unchanged_when_first_preserved_is_user() {
        let msgs = vec![
            msg(ChatRole::User),
            msg(ChatRole::Assistant),
            msg(ChatRole::User),
            msg(ChatRole::Assistant),
        ];
        assert_eq!(find_safe_boundary(&msgs, 2), 2);
    }

    #[test]
    fn boundary_walks_back_across_tool_results() {
        // 0 User, 1 Assistant(tool_calls), 2 Tool, 3 Tool, 4 Assistant, 5 User
        let msgs = vec![
            msg(ChatRole::User),
            assistant_with_tools(&["a", "b"]),
            tool_result("a"),
            tool_result("b"),
            msg(ChatRole::Assistant),
            msg(ChatRole::User),
        ];
        // raw_keep_from=3 (el segundo tool_result) → debe volver a 1 (assistant con tool_calls)
        assert_eq!(find_safe_boundary(&msgs, 3), 1);
        // raw_keep_from=2 (primer tool_result) → tambien vuelve a 1
        assert_eq!(find_safe_boundary(&msgs, 2), 1);
        // raw_keep_from=4 (assistant sano) → no se mueve
        assert_eq!(find_safe_boundary(&msgs, 4), 4);
    }

    #[test]
    fn boundary_at_zero_is_preserved() {
        let msgs = vec![tool_result("a")];
        assert_eq!(find_safe_boundary(&msgs, 0), 0);
    }

    #[test]
    fn boundary_beyond_len_returns_len() {
        let msgs = vec![msg(ChatRole::User)];
        assert_eq!(find_safe_boundary(&msgs, 42), 1);
    }
}
