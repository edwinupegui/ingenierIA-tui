//! Trait `McpTransport` — abstrae el medio de transporte JSON-RPC.
//!
//! Implementaciones:
//! - `transports::sse` — SSE + HTTP POST (usado por el servidor ingenierIA)
//! - `transports::stdio` — subprocess con JSON-RPC via stdin/stdout
//!
//! Los clientes llaman `send_request`/`send_notification` sin conocer el
//! medio. El lifecycle (connect/disconnect) es responsabilidad de cada impl.

use async_trait::async_trait;

use super::protocol::{JsonRpcRequest, JsonRpcResponse};

/// Kind de transporte — usado en diagnostics (`/mcp-status`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(
    dead_code,
    reason = "variantes construidas desde impls de transport (SSE activo, stdio en espera)"
)]
pub enum TransportKind {
    Sse,
    Stdio,
    WebSocket,
}

impl TransportKind {
    #[allow(
        dead_code,
        reason = "API diagnostica consumida por /mcp-status cuando haya cliente persistente"
    )]
    pub fn label(&self) -> &'static str {
        match self {
            Self::Sse => "SSE",
            Self::Stdio => "stdio",
            Self::WebSocket => "WebSocket",
        }
    }
}

/// Trait para enviar JSON-RPC requests/notifications sobre un transporte.
#[async_trait]
pub trait McpTransport: Send + Sync {
    /// Envia un request y espera la respuesta. `request.id` debe ser unico.
    async fn send_request(&self, request: JsonRpcRequest) -> anyhow::Result<JsonRpcResponse>;

    /// Envia una notification (sin respuesta esperada).
    async fn send_notification(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> anyhow::Result<()>;

    /// Tipo de transporte para logs y diagnostics.
    #[allow(dead_code, reason = "consumido cuando McpClient se convierta en singleton persistente")]
    fn kind(&self) -> TransportKind;
}
