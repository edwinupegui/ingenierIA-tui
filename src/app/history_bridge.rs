//! Helpers puros de conversion entre JSONL (`services/session`) y el formato
//! legacy (`services/history::SavedConversation`) usado por las Actions.
//!
//! Extraido de `app/chat_history.rs` para respetar el limite de 400 LOC.
//! Sin estado: solo funciones de conversion y loaders combinados.

use crate::services::history::{
    self, HistoryEntry, SavedConversation, SavedMessage, SavedToolCall,
};
use crate::services::session::{self, SerializedToolCall, SessionEntry, SessionMeta, TimedEntry};
use crate::state::{ChatMessage, ChatRole};

pub(crate) fn message_to_timed_entry(msg: &ChatMessage) -> TimedEntry {
    match msg.role {
        ChatRole::User => {
            TimedEntry::now(SessionEntry::UserMessage { content: msg.content.clone() })
        }
        ChatRole::Assistant => TimedEntry::now(SessionEntry::AssistantMessage {
            content: msg.content.clone(),
            tool_calls: msg
                .tool_calls
                .iter()
                .map(|tc| SerializedToolCall {
                    id: tc.id.clone(),
                    name: tc.name.clone(),
                    arguments: tc.arguments.clone(),
                })
                .collect(),
        }),
        ChatRole::Tool => TimedEntry::now(SessionEntry::ToolResult {
            tool_call_id: msg.tool_call_id.clone().unwrap_or_default(),
            content: msg.content.clone(),
        }),
        ChatRole::System => {
            TimedEntry::now(SessionEntry::SystemMessage { content: msg.content.clone() })
        }
    }
}

/// Lista combinada: sesiones JSONL (nuevas) + sesiones legacy `.json`.
/// Prefiere JSONL si hay duplicado de id.
pub(crate) fn list_history_merged() -> Vec<HistoryEntry> {
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut out: Vec<HistoryEntry> = Vec::new();

    for meta in session::list_metas() {
        if seen.insert(meta.id.clone()) {
            out.push(meta_to_history_entry(meta));
        }
    }
    for legacy in history::list_history() {
        if seen.insert(legacy.id.clone()) {
            out.push(legacy);
        }
    }
    out.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    out
}

fn meta_to_history_entry(meta: SessionMeta) -> HistoryEntry {
    HistoryEntry {
        id: meta.id,
        title: meta.title,
        factory: meta.factory,
        model: meta.model,
        created_at: if meta.updated_at.is_empty() { meta.created_at } else { meta.updated_at },
        message_count: meta.message_count,
        turn_count: meta.turn_count,
        total_input_tokens: meta.total_input_tokens,
        total_output_tokens: meta.total_output_tokens,
        total_cost: meta.total_cost,
    }
}

pub(crate) fn load_most_recent_any() -> Option<SavedConversation> {
    let entries = list_history_merged();
    let entry = entries.first()?;
    load_by_id(&entry.id)
}

/// Carga una sesion por id intentando primero JSONL, luego legacy `.json`.
pub(crate) fn load_by_id(id: &str) -> Option<SavedConversation> {
    let entries = session::load_all_entries(id);
    if !entries.is_empty() {
        let meta = session::meta_path(id).and_then(|p| SessionMeta::load(&p));
        return Some(build_saved_from_jsonl(id, entries, meta.as_ref()));
    }
    history::load_conversation(id)
}

fn build_saved_from_jsonl(
    id: &str,
    entries: Vec<TimedEntry>,
    meta: Option<&SessionMeta>,
) -> SavedConversation {
    let mut messages: Vec<SavedMessage> = Vec::with_capacity(entries.len());
    for entry in &entries {
        append_entry_as_message(&entry.entry, &mut messages);
    }

    let title = meta
        .map(|m| m.title.clone())
        .or_else(|| {
            messages
                .iter()
                .find(|m| m.role == "user")
                .map(|m| session::title_from_content(&m.content))
        })
        .unwrap_or_else(|| "Sin titulo".to_string());

    SavedConversation {
        id: id.to_string(),
        title,
        factory: meta.map(|m| m.factory.clone()).unwrap_or_else(|| "?".into()),
        model: meta.map(|m| m.model.clone()).unwrap_or_else(|| "?".into()),
        created_at: meta.map(|m| m.created_at.clone()).unwrap_or_default(),
        messages,
        turn_count: meta.map(|m| m.turn_count).unwrap_or(0),
        mode: meta.map(|m| m.mode.clone()).unwrap_or_else(|| "normal".into()),
        total_input_tokens: meta.map(|m| m.total_input_tokens).unwrap_or(0),
        total_output_tokens: meta.map(|m| m.total_output_tokens).unwrap_or(0),
        total_cost: meta.map(|m| m.total_cost).unwrap_or(0.0),
    }
}

fn append_entry_as_message(entry: &SessionEntry, out: &mut Vec<SavedMessage>) {
    match entry {
        SessionEntry::UserMessage { content } => {
            out.push(SavedMessage {
                role: "user".into(),
                content: content.clone(),
                tool_calls: Vec::new(),
                tool_call_id: None,
            });
        }
        SessionEntry::AssistantMessage { content, tool_calls } => {
            out.push(SavedMessage {
                role: "assistant".into(),
                content: content.clone(),
                tool_calls: tool_calls
                    .iter()
                    .map(|tc| SavedToolCall {
                        id: tc.id.clone(),
                        name: tc.name.clone(),
                        arguments: tc.arguments.clone(),
                    })
                    .collect(),
                tool_call_id: None,
            });
        }
        SessionEntry::ToolResult { tool_call_id, content } => {
            out.push(SavedMessage {
                role: "tool".into(),
                content: content.clone(),
                tool_calls: Vec::new(),
                tool_call_id: Some(tool_call_id.clone()),
            });
        }
        SessionEntry::SystemMessage { content } => {
            out.push(SavedMessage {
                role: "system".into(),
                content: content.clone(),
                tool_calls: Vec::new(),
                tool_call_id: None,
            });
        }
        SessionEntry::Fork { .. } | SessionEntry::MetaSnapshot { .. } => {
            // No son mensajes del chat; se ignoran para la reconstruccion.
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::session::SerializedToolCall;
    use crate::state::{ChatMessage, ChatRole, ToolCall, ToolCallStatus};

    #[test]
    fn user_message_converts_to_user_entry() {
        let msg = ChatMessage::new(ChatRole::User, "hola".into());
        let entry = message_to_timed_entry(&msg);
        assert!(matches!(entry.entry, SessionEntry::UserMessage { .. }));
    }

    #[test]
    fn assistant_with_tool_calls_preserves_them() {
        let mut msg = ChatMessage::new(ChatRole::Assistant, "hi".into());
        msg.tool_calls.push(ToolCall {
            id: "c1".into(),
            name: "search".into(),
            arguments: "{}".into(),
            status: ToolCallStatus::Success,
            duration_ms: None,
        });
        let entry = message_to_timed_entry(&msg);
        match entry.entry {
            SessionEntry::AssistantMessage { tool_calls, .. } => {
                assert_eq!(
                    tool_calls,
                    vec![SerializedToolCall {
                        id: "c1".into(),
                        name: "search".into(),
                        arguments: "{}".into(),
                    }]
                );
            }
            other => panic!("expected assistant_message, got {other:?}"),
        }
    }

    #[test]
    fn tool_message_without_id_uses_empty_string() {
        let msg = ChatMessage::new(ChatRole::Tool, "result".into());
        let entry = message_to_timed_entry(&msg);
        match entry.entry {
            SessionEntry::ToolResult { tool_call_id, content } => {
                assert_eq!(tool_call_id, "");
                assert_eq!(content, "result");
            }
            other => panic!("expected tool_result, got {other:?}"),
        }
    }

    #[test]
    fn build_saved_ignores_fork_and_snapshot_entries() {
        let entries = vec![
            TimedEntry::with_timestamp(
                "t".into(),
                SessionEntry::UserMessage { content: "u1".into() },
            ),
            TimedEntry::with_timestamp(
                "t".into(),
                SessionEntry::Fork { parent_session_id: "parent".into(), label: "test".into() },
            ),
            TimedEntry::with_timestamp(
                "t".into(),
                SessionEntry::MetaSnapshot {
                    turn_count: 1,
                    message_count: 1,
                    total_input_tokens: 10,
                    total_output_tokens: 5,
                    total_cost: 0.01,
                    mode: "normal".into(),
                },
            ),
        ];
        let conv = build_saved_from_jsonl("id1", entries, None);
        // Fork y MetaSnapshot no cuentan como mensajes.
        assert_eq!(conv.messages.len(), 1);
        assert_eq!(conv.messages[0].role, "user");
    }
}
