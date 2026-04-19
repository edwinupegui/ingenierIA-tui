//! `McpClient`: cliente JSON-RPC sobre cualquier `McpTransport`.
//!
//! El cliente se encarga del lifecycle MCP (initialize + notifications/initialized)
//! y delega el envio de mensajes al transporte inyectado. Esto permite SSE,
//! stdio, o cualquier otro transporte que implemente el trait.
//!
//! Para backward-compat, `McpClient::connect(base_url)` sigue existiendo y
//! crea un `SseTransport` internamente.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use serde::Deserialize;

use super::protocol::{JsonRpcRequest, ToolResult};
use super::transport::{McpTransport, TransportKind};
use super::transports::SseTransport;

// ── MCP tool discovery ─────────────────────────────────────────────────────

/// Metadata for an MCP tool discovered via `tools/list`.
#[cfg(feature = "mcp")]
#[derive(Debug, Clone, Deserialize)]
pub struct McpToolInfo {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(rename = "inputSchema", default)]
    pub input_schema: serde_json::Value,
}

// ── MCP Client ──────────────────────────────────────────────────────────────

/// Cliente JSON-RPC para MCP. Trabaja sobre cualquier `McpTransport`.
pub struct McpClient {
    transport: Box<dyn McpTransport>,
    next_id: Arc<AtomicU64>,
}

impl McpClient {
    /// Constructor low-level con transporte inyectado. Ejecuta el handshake
    /// `initialize` + `notifications/initialized`.
    pub async fn from_transport(transport: Box<dyn McpTransport>) -> anyhow::Result<Self> {
        let client = Self { transport, next_id: Arc::new(AtomicU64::new(1)) };
        client.initialize().await?;
        client
            .transport
            .send_notification("notifications/initialized", serde_json::json!({}))
            .await?;
        Ok(client)
    }

    /// Backward-compat: crea un cliente con transporte SSE al URL dado.
    pub async fn connect(base_url: &str) -> anyhow::Result<Self> {
        let sse = SseTransport::connect(base_url).await?;
        Self::from_transport(Box::new(sse)).await
    }

    /// Tipo de transporte actual (para `/mcp-status`).
    #[allow(dead_code, reason = "diagnostico consumido cuando McpClient sea singleton persistente")]
    pub fn transport_kind(&self) -> TransportKind {
        self.transport.kind()
    }

    async fn initialize(&self) -> anyhow::Result<serde_json::Value> {
        self.send_request(
            "initialize",
            serde_json::json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {
                    "name": "ingenieria-tui",
                    "version": env!("CARGO_PKG_VERSION")
                }
            }),
        )
        .await
    }

    /// Envia un request JSON-RPC y devuelve el `result`. Traduce errores
    /// JSON-RPC a `anyhow::Error`.
    async fn send_request(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> anyhow::Result<serde_json::Value> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let request = JsonRpcRequest::new(id, method, params);
        let rpc_resp = self.transport.send_request(request).await?;
        if let Some(err) = rpc_resp.error {
            anyhow::bail!("MCP error: {}", err.message);
        }
        rpc_resp.result.ok_or_else(|| anyhow::anyhow!("No result in MCP response"))
    }

    /// Discover available MCP tools via `tools/list`.
    #[cfg(feature = "mcp")]
    pub async fn list_tools(&self) -> anyhow::Result<Vec<McpToolInfo>> {
        let result = self.send_request("tools/list", serde_json::json!({})).await?;
        let tools: Vec<McpToolInfo> = serde_json::from_value(
            result.get("tools").cloned().unwrap_or(serde_json::Value::Array(vec![])),
        )?;
        Ok(tools)
    }

    /// Call an MCP tool by name with arguments. Returns the text content,
    /// truncado a `DEFAULT_MAX_BYTES` si es oversized.
    pub async fn call_tool(
        &self,
        name: &str,
        arguments: serde_json::Value,
    ) -> anyhow::Result<String> {
        self.call_tool_with_schema(name, arguments, None).await
    }

    /// Variante de `call_tool` que valida contra el `inputSchema` si se
    /// provee. Si la validacion falla, devuelve el error sin llamar al server.
    pub async fn call_tool_with_schema(
        &self,
        name: &str,
        arguments: serde_json::Value,
        schema: Option<&serde_json::Value>,
    ) -> anyhow::Result<String> {
        if let Some(schema) = schema {
            if let Err(reason) = super::validation::validate_tool_input(&arguments, schema) {
                anyhow::bail!("invalid tool input for '{name}': {reason}");
            }
        }

        let result = self
            .send_request(
                "tools/call",
                serde_json::json!({
                    "name": name,
                    "arguments": arguments,
                }),
            )
            .await?;

        let tool_result: ToolResult = serde_json::from_value(result)?;
        let text = tool_result.text();
        Ok(super::truncation::truncate_if_oversized(&text, super::truncation::DEFAULT_MAX_BYTES))
    }
}
