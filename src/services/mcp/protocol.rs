//! Tipos JSON-RPC 2.0 compartidos entre transportes.
//!
//! Referencia: https://www.jsonrpc.org/specification y MCP spec.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: &'static str,
    pub id: u64,
    pub method: String,
    pub params: serde_json::Value,
}

impl JsonRpcRequest {
    pub fn new(id: u64, method: impl Into<String>, params: serde_json::Value) -> Self {
        Self { jsonrpc: "2.0", id, method: method.into(), params }
    }
}

#[derive(Debug, Deserialize)]
pub struct JsonRpcResponse {
    #[expect(
        dead_code,
        reason = "field populated by JSON-RPC deserialization, required by protocol"
    )]
    pub jsonrpc: String,
    #[expect(
        dead_code,
        reason = "field populated by JSON-RPC deserialization, required by protocol"
    )]
    pub id: Option<u64>,
    pub result: Option<serde_json::Value>,
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Deserialize)]
pub struct JsonRpcError {
    #[expect(
        dead_code,
        reason = "field populated by JSON-RPC deserialization, kept for diagnostics"
    )]
    pub code: i64,
    pub message: String,
}

// ── Tool call result ────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ToolResult {
    pub content: Vec<ContentBlock>,
    #[serde(default)]
    #[expect(dead_code, reason = "field populated by MCP tool response deserialization")]
    pub is_error: bool,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum ContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(other)]
    Other,
}

impl ToolResult {
    /// Concatena todos los bloques de texto, ignorando otros tipos.
    pub fn text(self) -> String {
        self.content
            .into_iter()
            .filter_map(|block| match block {
                ContentBlock::Text { text } => Some(text),
                ContentBlock::Other => None,
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}
