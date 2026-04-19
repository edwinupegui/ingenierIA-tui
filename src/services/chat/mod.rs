// ── Re-exports from ingenieria-api crate (E28 Phase 3) ───────────────────────
pub use ingenieria_api::metrics;
pub use ingenieria_api::model_fallback;
pub use ingenieria_api::pricing;
pub use ingenieria_api::prompt_cache;
pub use ingenieria_api::retry;
pub use ingenieria_api::stall_detector;
pub use ingenieria_api::{ChatEvent, ChatStream, ModelInfo, ToolDefinition};

// ── Local modules (not yet extracted) ──────────────────────────────────────
pub mod claude_provider;
pub mod claude_sse;
#[cfg(feature = "copilot")]
pub mod copilot_provider;
pub mod mock_provider;
pub mod stream_monitor;
pub mod stream_parser;
pub mod synthetic_results;

use crate::state::ChatMessage;

/// Build a ToolDefinition from an MCP tool discovered via `tools/list`.
#[cfg(feature = "mcp")]
pub fn tool_def_from_mcp(info: &crate::services::mcp::McpToolInfo) -> ToolDefinition {
    ToolDefinition {
        json: serde_json::json!({
            "type": "function",
            "function": {
                "name": info.name,
                "description": info.description.as_deref().unwrap_or(""),
                "parameters": info.input_schema,
            }
        }),
    }
}

// ── Provider trait ──────────────────────────────────────────────────────────

/// Trait for chat completion providers (Copilot, Claude, etc.).
#[async_trait::async_trait]
pub trait ChatProvider: Send + Sync {
    /// Stream a chat completion. Returns a stream of ChatEvents.
    async fn stream_chat(
        &self,
        model: &str,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
    ) -> anyhow::Result<ChatStream>;

    /// List available models for this provider.
    #[allow(dead_code)]
    async fn list_models(&self) -> anyhow::Result<Vec<ModelInfo>>;

    /// Human-readable provider name (e.g. "GitHub Copilot").
    #[allow(dead_code)]
    fn provider_name(&self) -> &str;
}
