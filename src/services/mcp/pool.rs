//! `McpPool`: cliente MCP persistente para el server primario de ingenierIA.
//!
//! Diseño:
//! - Mantiene un `Arc<McpClient>` cacheado tras `RwLock<Option<_>>` para que
//!   múltiples tool calls reusen la misma conexión SSE.
//! - Primer `call_tool` abre la conexión (lazy). Errores de conexión NO
//!   envenenan el cache: siguientes calls reintentan.
//! - Si `call_tool` falla con error de transporte (desconexión mid-call), el
//!   cache se invalida y la próxima llamada reconecta automáticamente.
//! - Este pool es SOLO para el server principal (`base_url` del wizard).
//!   Los servers extras viven en `McpLifecycleManager`.
//!
//! Por qué existe separado del `McpLifecycleManager`: el primary server no
//! vive en `mcp-servers.json`; se configura via wizard y debe estar listo
//! desde el primer tool call del chat. El lifecycle manager tiene retry
//! backoff independiente que no queremos bloqueando al usuario.

use std::sync::Arc;

use tokio::sync::RwLock;

use super::client::McpClient;

/// Pool con una sola conexión cacheada al MCP server de ingenierIA.
pub struct McpPool {
    inner: RwLock<Option<Arc<McpClient>>>,
    base_url: String,
}

impl McpPool {
    pub fn new(base_url: impl Into<String>) -> Arc<Self> {
        Arc::new(Self { inner: RwLock::new(None), base_url: base_url.into() })
    }

    #[allow(dead_code, reason = "consumido por diagnostics futuros y /mcp-status extensions")]
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Devuelve un cliente listo. Si no hay cache, abre la conexión y la
    /// guarda. Errores de conexión dejan el cache vacío (no envenenado).
    pub async fn get_client(&self) -> anyhow::Result<Arc<McpClient>> {
        if let Some(c) = self.read_cached().await {
            return Ok(c);
        }
        let client = Arc::new(McpClient::connect(&self.base_url).await?);
        let mut guard = self.inner.write().await;
        if guard.is_none() {
            *guard = Some(client.clone());
        }
        Ok(guard.clone().unwrap_or(client))
    }

    async fn read_cached(&self) -> Option<Arc<McpClient>> {
        self.inner.read().await.clone()
    }

    /// Invalida el cache. La próxima `get_client()` abrirá una conexión nueva.
    pub async fn invalidate(&self) {
        self.inner.write().await.take();
    }

    /// Llama un tool MCP reusando el cliente cacheado. Si el call falla, el
    /// cache se invalida para que el próximo intento reconecte.
    pub async fn call_tool(
        &self,
        name: &str,
        arguments: serde_json::Value,
    ) -> anyhow::Result<String> {
        let client = self.get_client().await?;
        match client.call_tool(name, arguments).await {
            Ok(out) => Ok(out),
            Err(e) => {
                self.invalidate().await;
                Err(e)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn pool_has_no_client_initially() {
        let pool = McpPool::new("http://invalid.test");
        assert!(pool.read_cached().await.is_none());
    }

    #[tokio::test]
    async fn pool_invalidate_is_idempotent() {
        let pool = McpPool::new("http://invalid.test");
        pool.invalidate().await;
        pool.invalidate().await;
        assert!(pool.read_cached().await.is_none());
    }

    #[tokio::test]
    async fn get_client_fails_without_server_and_leaves_cache_empty() {
        // Puerto inutilizable: el connect debe fallar.
        let pool = McpPool::new("http://127.0.0.1:1");
        let res = pool.get_client().await;
        assert!(res.is_err());
        assert!(pool.read_cached().await.is_none(), "cache no debe envenenarse");
    }

    /// Integración: requiere `sc-mcp-ingenieria` corriendo en localhost:3001.
    /// Valida que 3 tool calls consecutivos reusen la misma conexión SSE (el
    /// caché retiene el Arc<McpClient>). Skip default; ejecutar con
    /// `cargo test --features full -- --ignored pool_reuses_connection`.
    #[tokio::test]
    #[ignore = "requiere sc-mcp-ingenieria en localhost:3001"]
    async fn pool_reuses_connection_across_calls() {
        let pool = McpPool::new("http://localhost:3001");
        // Primer call abre conexión.
        let r1 = pool
            .call_tool("list_documents", serde_json::json!({"factory": "net"}))
            .await
            .expect("first call should succeed");
        assert!(!r1.is_empty());
        let client_after_first = pool.read_cached().await.expect("client cached");
        // Segundo call debe reusar exactamente el mismo Arc<McpClient>.
        let _r2 = pool
            .call_tool("list_documents", serde_json::json!({"factory": "net"}))
            .await
            .expect("second call should succeed");
        let client_after_second = pool.read_cached().await.expect("client still cached");
        assert!(
            Arc::ptr_eq(&client_after_first, &client_after_second),
            "pool debe reusar la conexión SSE, no reconectar"
        );
        // Tercer call con otro tool: mismo cliente.
        let _r3 = pool
            .call_tool(
                "get_factory_context",
                serde_json::json!({"factory": "net", "include": ["config"]}),
            )
            .await
            .expect("third call should succeed");
        let client_after_third = pool.read_cached().await.expect("client still cached");
        assert!(Arc::ptr_eq(&client_after_first, &client_after_third));
    }
}
