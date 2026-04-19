//! Session JSONL entry model.
//!
//! Cada linea del `.jsonl` es un `TimedEntry` serializado. El formato es
//! estable y pensado para ser compartido/exportado. Referencia CLAW
//! `rust/crates/runtime/src/session.rs` (1517 LOC) — version simplificada
//! adaptada al dominio del TUI.

use serde::{Deserialize, Serialize};

/// Representa una entrada unica persistida en el JSONL.
///
/// Serializacion:
/// ```json
/// {"timestamp":"2026-04-13T...","entry_type":"user_message","data":{"content":"..."}}
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimedEntry {
    pub timestamp: String,
    #[serde(flatten)]
    pub entry: SessionEntry,
}

/// Tipos de entrada soportados. Tag `entry_type` + `data` payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "entry_type", content = "data", rename_all = "snake_case")]
pub enum SessionEntry {
    /// Mensaje del usuario.
    UserMessage { content: String },
    /// Mensaje del asistente con tool_calls opcionales.
    AssistantMessage {
        content: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        tool_calls: Vec<SerializedToolCall>,
    },
    /// Resultado de un tool invocado por el asistente.
    ToolResult { tool_call_id: String, content: String },
    /// Mensaje de sistema (instrucciones inyectadas, modo planning, etc.).
    SystemMessage { content: String },
    /// Marcador de fork: esta sesion nacio de otra.
    Fork { parent_session_id: String, label: String },
    /// Snapshot de metadata — se escribe periodicamente para recuperar stats
    /// sin tener que releer el `.meta.json` sidecar.
    MetaSnapshot {
        turn_count: usize,
        message_count: usize,
        total_input_tokens: u32,
        total_output_tokens: u32,
        total_cost: f64,
        mode: String,
    },
}

/// Version serializable de `ToolCall` (sin runtime status ni duration).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SerializedToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

impl TimedEntry {
    /// Construye un `TimedEntry` con timestamp ISO-8601 actual.
    pub fn now(entry: SessionEntry) -> Self {
        Self { timestamp: ingenieria_domain::time::now_iso(), entry }
    }

    /// Construye un `TimedEntry` con timestamp explicito (usado en tests y
    /// para preservar orden cronologico al re-persistir entries recuperados).
    #[cfg_attr(not(test), allow(dead_code, reason = "helper usado en tests y utilidades futuras"))]
    pub fn with_timestamp(timestamp: String, entry: SessionEntry) -> Self {
        Self { timestamp, entry }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_message_round_trip() {
        let e = TimedEntry::with_timestamp(
            "2026-04-13T10:00:00Z".into(),
            SessionEntry::UserMessage { content: "hola".into() },
        );
        let line = serde_json::to_string(&e).unwrap();
        assert!(line.contains("\"entry_type\":\"user_message\""));
        assert!(line.contains("\"content\":\"hola\""));
        let back: TimedEntry = serde_json::from_str(&line).unwrap();
        match back.entry {
            SessionEntry::UserMessage { content } => assert_eq!(content, "hola"),
            other => panic!("expected user_message, got {other:?}"),
        }
    }

    #[test]
    fn assistant_message_with_tool_calls() {
        let e = TimedEntry::with_timestamp(
            "2026-04-13T10:00:00Z".into(),
            SessionEntry::AssistantMessage {
                content: "".into(),
                tool_calls: vec![SerializedToolCall {
                    id: "call_1".into(),
                    name: "search".into(),
                    arguments: "{\"q\":\"x\"}".into(),
                }],
            },
        );
        let line = serde_json::to_string(&e).unwrap();
        let back: TimedEntry = serde_json::from_str(&line).unwrap();
        match back.entry {
            SessionEntry::AssistantMessage { tool_calls, .. } => {
                assert_eq!(tool_calls.len(), 1);
                assert_eq!(tool_calls[0].name, "search");
            }
            other => panic!("expected assistant_message, got {other:?}"),
        }
    }

    #[test]
    fn tool_result_round_trip() {
        let e = TimedEntry::with_timestamp(
            "t".into(),
            SessionEntry::ToolResult { tool_call_id: "call_1".into(), content: "ok".into() },
        );
        let line = serde_json::to_string(&e).unwrap();
        let back: TimedEntry = serde_json::from_str(&line).unwrap();
        match back.entry {
            SessionEntry::ToolResult { tool_call_id, content } => {
                assert_eq!(tool_call_id, "call_1");
                assert_eq!(content, "ok");
            }
            other => panic!("expected tool_result, got {other:?}"),
        }
    }

    #[test]
    fn fork_entry_round_trip() {
        let e = TimedEntry::with_timestamp(
            "t".into(),
            SessionEntry::Fork { parent_session_id: "abc123".into(), label: "probar opus".into() },
        );
        let line = serde_json::to_string(&e).unwrap();
        assert!(line.contains("\"entry_type\":\"fork\""));
        let back: TimedEntry = serde_json::from_str(&line).unwrap();
        match back.entry {
            SessionEntry::Fork { parent_session_id, label } => {
                assert_eq!(parent_session_id, "abc123");
                assert_eq!(label, "probar opus");
            }
            other => panic!("expected fork, got {other:?}"),
        }
    }
}
