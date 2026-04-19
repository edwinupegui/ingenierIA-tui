//! Legacy history loader (formato JSON completo).
//!
//! **Deprecado** para escritura desde E11. El sistema vivo es
//! `services/session/` (JSONL append-only). Este modulo sobrevive unicamente
//! para LEER sesiones antiguas guardadas como `.json` en
//! `~/.config/ingenieria-tui/history/` y merge-arlas con el listado nuevo.
//!
//! Las nuevas sesiones NO se escriben aqui.

use serde::{Deserialize, Serialize};

/// Maximum number of history entries to keep when listing legacy sessions.
const MAX_HISTORY_ENTRIES: usize = 50;

/// Metadata for a saved legacy conversation (lightweight, for listing).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub id: String,
    pub title: String,
    pub factory: String,
    pub model: String,
    pub created_at: String,
    pub message_count: usize,
    #[serde(default)]
    pub turn_count: usize,
    #[serde(default)]
    pub total_input_tokens: u32,
    #[serde(default)]
    pub total_output_tokens: u32,
    #[serde(default)]
    pub total_cost: f64,
}

/// A full saved legacy conversation (JSON format).
#[derive(Debug, Serialize, Deserialize)]
pub struct SavedConversation {
    pub id: String,
    pub title: String,
    pub factory: String,
    pub model: String,
    pub created_at: String,
    pub messages: Vec<SavedMessage>,
    #[serde(default)]
    pub turn_count: usize,
    #[serde(default = "default_mode")]
    pub mode: String,
    #[serde(default)]
    pub total_input_tokens: u32,
    #[serde(default)]
    pub total_output_tokens: u32,
    #[serde(default)]
    pub total_cost: f64,
}

fn default_mode() -> String {
    "normal".to_string()
}

/// Serializable chat message (legacy format).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedMessage {
    pub role: String,
    pub content: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<SavedToolCall>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

/// Serializable tool call (legacy format).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

/// Lista sesiones legacy `.json` (solo lectura, newest first).
pub fn list_history() -> Vec<HistoryEntry> {
    let Some(dir) = history_dir() else {
        return Vec::new();
    };

    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };

    let mut result: Vec<HistoryEntry> = entries
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "json"))
        .filter_map(|e| {
            let data = std::fs::read_to_string(e.path()).ok()?;
            let conv: SavedConversation = serde_json::from_str(&data).ok()?;
            Some(HistoryEntry {
                id: conv.id,
                title: conv.title,
                factory: conv.factory,
                model: conv.model,
                created_at: conv.created_at,
                message_count: conv.messages.len(),
                turn_count: conv.turn_count,
                total_input_tokens: conv.total_input_tokens,
                total_output_tokens: conv.total_output_tokens,
                total_cost: conv.total_cost,
            })
        })
        .collect();

    result.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    result.truncate(MAX_HISTORY_ENTRIES);
    result
}

/// Carga una conversacion legacy por ID.
pub fn load_conversation(id: &str) -> Option<SavedConversation> {
    let path = history_dir()?.join(format!("{id}.json"));
    let data = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&data).ok()
}

fn history_dir() -> Option<std::path::PathBuf> {
    dirs::config_dir().map(|d| d.join("ingenieria-tui").join("history"))
}
