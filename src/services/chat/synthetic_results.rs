//! Parches para mensajes con `tool_use` sin su `tool_result` correspondiente.
//!
//! Si una sesion previa se interrumpio a medio turno (crash, Ctrl+C, timeout)
//! al cargar el historial nos podemos encontrar con tool_use "huerfanos": el
//! assistant llamo al tool pero nunca se guardo el resultado. Sin parchear,
//! Anthropic devuelve 400 porque espera un tool_result por cada tool_use.
//!
//! Referencia: claude-code `SYNTHETIC_TOOL_RESULT_PLACEHOLDER`.

use crate::state::{ChatMessage, ChatRole, ToolCall};

/// Placeholder usado cuando el resultado real no esta disponible.
pub const SYNTHETIC_RESULT_CONTENT: &str =
    "[Resultado no disponible: sesion interrumpida antes de capturar la salida de este tool]";

/// Recorre los mensajes y, para cada `tool_use` del assistant sin un
/// `tool_result` posterior, inserta un mensaje `Tool` sintetico.
///
/// Los resultados sinteticos se insertan al final del bloque de tool_results
/// que sigue al assistant (despues de los resultados reales existentes), para
/// que Anthropic los procese como parte del mismo turno.
pub fn patch_orphaned_tool_uses(messages: &mut Vec<ChatMessage>) -> usize {
    let mut patched = 0;
    let mut i = 0;
    while i < messages.len() {
        let ChatRole::Assistant = messages[i].role else {
            i += 1;
            continue;
        };
        let orphans = find_orphans(messages, i);
        if !orphans.is_empty() {
            let mut insert_at = i + 1;
            while insert_at < messages.len() && messages[insert_at].role == ChatRole::Tool {
                insert_at += 1;
            }
            for (offset, tc) in orphans.into_iter().enumerate() {
                messages.insert(insert_at + offset, synthetic_for(&tc));
                patched += 1;
            }
        }
        i += 1;
    }
    patched
}

/// Devuelve los tool_calls del mensaje `idx` que no tienen resultado
/// correspondiente despues (hasta el proximo mensaje User/System).
fn find_orphans(messages: &[ChatMessage], idx: usize) -> Vec<ToolCall> {
    let tool_calls = &messages[idx].tool_calls;
    if tool_calls.is_empty() {
        return Vec::new();
    }
    let mut seen_ids: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for msg in messages.iter().skip(idx + 1) {
        match msg.role {
            ChatRole::Tool => {
                if let Some(id) = msg.tool_call_id.as_deref() {
                    seen_ids.insert(id);
                }
            }
            // Un nuevo turno del user cierra la ventana de busqueda.
            ChatRole::User | ChatRole::System => break,
            ChatRole::Assistant => {}
        }
    }
    tool_calls.iter().filter(|tc| !seen_ids.contains(tc.id.as_str())).cloned().collect()
}

fn synthetic_for(tc: &ToolCall) -> ChatMessage {
    ChatMessage::tool_result(tc.id.clone(), SYNTHETIC_RESULT_CONTENT.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{ChatMessage, ChatRole, ToolCall, ToolCallStatus};

    fn asst_with_tool(id: &str, name: &str) -> ChatMessage {
        let mut m = ChatMessage::new(ChatRole::Assistant, String::new());
        m.tool_calls.push(ToolCall {
            id: id.into(),
            name: name.into(),
            arguments: "{}".into(),
            status: ToolCallStatus::Pending,
            duration_ms: None,
        });
        m
    }

    #[test]
    fn no_tool_calls_no_patch() {
        let mut msgs = vec![
            ChatMessage::new(ChatRole::User, "hi".into()),
            ChatMessage::new(ChatRole::Assistant, "hello".into()),
        ];
        assert_eq!(patch_orphaned_tool_uses(&mut msgs), 0);
        assert_eq!(msgs.len(), 2);
    }

    #[test]
    fn matched_tool_result_no_patch() {
        let mut msgs = vec![
            ChatMessage::new(ChatRole::User, "run".into()),
            asst_with_tool("t1", "bash"),
            ChatMessage::tool_result("t1".into(), "ok".into()),
        ];
        assert_eq!(patch_orphaned_tool_uses(&mut msgs), 0);
        assert_eq!(msgs.len(), 3);
    }

    #[test]
    fn orphan_gets_synthetic_result() {
        let mut msgs =
            vec![ChatMessage::new(ChatRole::User, "run".into()), asst_with_tool("t1", "bash")];
        let patched = patch_orphaned_tool_uses(&mut msgs);
        assert_eq!(patched, 1);
        assert_eq!(msgs.len(), 3);
        assert_eq!(msgs[2].role, ChatRole::Tool);
        assert_eq!(msgs[2].tool_call_id.as_deref(), Some("t1"));
        assert_eq!(msgs[2].content, SYNTHETIC_RESULT_CONTENT);
    }

    #[test]
    fn multiple_orphans_in_same_message() {
        let mut asst = asst_with_tool("t1", "bash");
        asst.tool_calls.push(ToolCall {
            id: "t2".into(),
            name: "read".into(),
            arguments: "{}".into(),
            status: ToolCallStatus::Pending,
            duration_ms: None,
        });
        let mut msgs = vec![ChatMessage::new(ChatRole::User, "x".into()), asst];
        let patched = patch_orphaned_tool_uses(&mut msgs);
        assert_eq!(patched, 2);
        assert_eq!(msgs[2].tool_call_id.as_deref(), Some("t1"));
        assert_eq!(msgs[3].tool_call_id.as_deref(), Some("t2"));
    }

    #[test]
    fn partial_match_only_patches_missing() {
        let mut asst = asst_with_tool("t1", "bash");
        asst.tool_calls.push(ToolCall {
            id: "t2".into(),
            name: "read".into(),
            arguments: "{}".into(),
            status: ToolCallStatus::Pending,
            duration_ms: None,
        });
        let mut msgs = vec![
            ChatMessage::new(ChatRole::User, "x".into()),
            asst,
            ChatMessage::tool_result("t1".into(), "ok".into()),
        ];
        let patched = patch_orphaned_tool_uses(&mut msgs);
        assert_eq!(patched, 1);
        assert_eq!(msgs.len(), 4);
        // t2 es el huerfano; debe ser el mensaje 3.
        assert_eq!(msgs[3].tool_call_id.as_deref(), Some("t2"));
    }

    #[test]
    fn new_user_turn_bounds_search() {
        // Si un siguiente User aparece antes del resultado, el tool queda huerfano
        // aun si mas adelante hay un "t1" (que pertenece a otro turno).
        let mut msgs = vec![
            ChatMessage::new(ChatRole::User, "first".into()),
            asst_with_tool("t1", "bash"),
            ChatMessage::new(ChatRole::User, "second".into()),
            ChatMessage::tool_result("t1".into(), "stray".into()),
        ];
        let patched = patch_orphaned_tool_uses(&mut msgs);
        assert_eq!(patched, 1);
    }
}
