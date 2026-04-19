//! Core chat types shared across the workspace.
//!
//! These types carry no dependencies on ratatui or services. The main
//! binary extends `ChatMessage` with UI-specific fields (`cached_lines`,
//! `structured`) that live in `state/chat_types.rs`.

use serde::{Deserialize, Serialize};

/// Role of a chat message in the conversation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChatRole {
    System,
    User,
    Assistant,
    Tool,
}

/// Status of a tool call execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolCallStatus {
    Pending,
    Success,
    Error,
}

/// A tool invocation attached to an assistant message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
    pub status: ToolCallStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
}

/// Operating mode for the chat session.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChatMode {
    #[default]
    Normal,
    Planning,
    PlanReview,
}

impl ChatMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            ChatMode::Normal => "normal",
            ChatMode::Planning => "planning",
            ChatMode::PlanReview => "plan_review",
        }
    }

    pub fn from_str_lossy(s: &str) -> Self {
        match s {
            "planning" => ChatMode::Planning,
            "plan_review" => ChatMode::PlanReview,
            _ => ChatMode::Normal,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_role_serializes() {
        let json = serde_json::to_string(&ChatRole::Assistant).unwrap();
        assert_eq!(json, "\"assistant\"");
    }

    #[test]
    fn tool_call_status_roundtrip() {
        let s = ToolCallStatus::Success;
        let json = serde_json::to_string(&s).unwrap();
        let back: ToolCallStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(s, back);
    }

    #[test]
    fn chat_mode_from_str_lossy_defaults() {
        assert_eq!(ChatMode::from_str_lossy("planning"), ChatMode::Planning);
        assert_eq!(ChatMode::from_str_lossy("unknown"), ChatMode::Normal);
    }
}
