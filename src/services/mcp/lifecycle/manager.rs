//! `McpLifecycleManager`: coordina multiples servers MCP con degraded-mode.
//!
//! Diseño:
//! - `start_all` intenta conectar cada server en paralelo (tokio::spawn).
//! - Cada server vive en un `ServerHandle` tras `Arc<RwLock<Inner>>` para
//!   snapshot thread-safe desde UI.
//! - Fallos no detienen el resto (partial failure → degraded-mode).
//! - Retry con exponential backoff ([`super::retry`]) hasta un cap de
//!   [`MAX_ATTEMPTS`] fallos consecutivos; entonces el server queda Failed
//!   permanente hasta reload de config.
//! - Routing de tools: `call_tool("server/tool", args)` si hay prefijo,
//!   si no busca la primera coincidencia por nombre simple entre los ready.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use super::super::client::McpClient;
use super::super::transport::{McpTransport, TransportKind};
use super::super::transports::{SseTransport, StdioTransport, WebSocketTransport};
use super::config::{ServerConfig, ServerKind};
use super::retry::next_delay;
use super::state::{LifecycleSnapshot, ServerSnapshot, ServerState};

/// Tope de reintentos consecutivos antes de dejar Failed permanente.
const MAX_ATTEMPTS: u32 = 6;

/// Handle interno por server.
struct ServerHandle {
    config: ServerConfig,
    state: ServerState,
    client: Option<Arc<McpClient>>,
    tools: Vec<String>,
    attempts: u32,
}

impl ServerHandle {
    fn kind(&self) -> TransportKind {
        match self.config.kind {
            ServerKind::Sse { .. } => TransportKind::Sse,
            ServerKind::Stdio { .. } => TransportKind::Stdio,
            ServerKind::WebSocket { .. } => TransportKind::WebSocket,
        }
    }
}

struct Inner {
    handles: HashMap<String, ServerHandle>,
}

/// Manager publico, clonable (Arc interno).
#[derive(Clone)]
pub struct McpLifecycleManager {
    inner: Arc<RwLock<Inner>>,
}

impl McpLifecycleManager {
    /// Crea un manager vacio.
    pub fn new() -> Self {
        Self { inner: Arc::new(RwLock::new(Inner { handles: HashMap::new() })) }
    }

    /// Registra configs y dispara las conexiones en paralelo. Los servers
    /// `enabled:false` quedan en `Disabled` sin tocar red.
    pub async fn start_all(&self, configs: Vec<ServerConfig>) {
        let names: Vec<String> = {
            let mut inner = self.write_inner();
            for cfg in configs {
                let state =
                    if cfg.enabled { ServerState::Connecting } else { ServerState::Disabled };
                inner.handles.insert(
                    cfg.name.clone(),
                    ServerHandle {
                        config: cfg.clone(),
                        state,
                        client: None,
                        tools: Vec::new(),
                        attempts: 0,
                    },
                );
            }
            inner
                .handles
                .values()
                .filter(|h| h.config.enabled)
                .map(|h| h.config.name.clone())
                .collect()
        };
        for name in names {
            let mgr = self.clone();
            tokio::spawn(async move {
                mgr.connect_loop(name).await;
            });
        }
    }

    /// Loop de conexion + retry con backoff. Termina al conectar, al superar
    /// `MAX_ATTEMPTS`, o si el handle desaparece.
    async fn connect_loop(&self, name: String) {
        loop {
            let cfg = {
                let inner = self.read_inner();
                let Some(h) = inner.handles.get(&name) else { return };
                h.config.clone()
            };

            match connect_server(&cfg).await {
                Ok((client, tools)) => {
                    let mut inner = self.write_inner();
                    if let Some(h) = inner.handles.get_mut(&name) {
                        h.state = ServerState::Ready { tools_count: tools.len() };
                        h.client = Some(Arc::new(client));
                        h.tools = tools;
                        h.attempts = 0;
                    }
                    return;
                }
                Err(e) => {
                    let attempts = {
                        let mut inner = self.write_inner();
                        let Some(h) = inner.handles.get_mut(&name) else { return };
                        h.attempts = h.attempts.saturating_add(1);
                        h.state =
                            ServerState::Failed { reason: e.to_string(), attempts: h.attempts };
                        h.client = None;
                        h.tools.clear();
                        h.attempts
                    };
                    if attempts >= MAX_ATTEMPTS {
                        return;
                    }
                    tokio::time::sleep(next_delay(attempts)).await;
                    // Marca Connecting antes del reintento para UI.
                    let mut inner = self.write_inner();
                    if let Some(h) = inner.handles.get_mut(&name) {
                        if matches!(h.state, ServerState::Failed { .. }) {
                            h.state = ServerState::Connecting;
                        }
                    }
                }
            }
        }
    }

    /// Snapshot read-only para UI. Lock sincrono con secciones cortas (solo
    /// clona metadata, sin red ni IO).
    pub fn snapshot(&self) -> LifecycleSnapshot {
        let inner = self.read_inner();
        let mut servers: Vec<ServerSnapshot> = inner
            .handles
            .values()
            .map(|h| ServerSnapshot {
                name: h.config.name.clone(),
                kind: h.kind(),
                state: h.state.clone(),
            })
            .collect();
        servers.sort_by(|a, b| a.name.cmp(&b.name));
        LifecycleSnapshot { servers }
    }

    fn read_inner(&self) -> std::sync::RwLockReadGuard<'_, Inner> {
        // Envenenamiento no puede ocurrir en practica: ningun panic sostiene el lock.
        self.inner.read().unwrap_or_else(|e| e.into_inner())
    }

    fn write_inner(&self) -> std::sync::RwLockWriteGuard<'_, Inner> {
        self.inner.write().unwrap_or_else(|e| e.into_inner())
    }

    /// Invoca un tool. Si `qualified_name` contiene `/`, prefijo = server.
    /// Si no, se busca en el primer server Ready que exponga ese tool.
    /// Retorna `Err` si no hay server disponible con el tool.
    #[allow(
        dead_code,
        reason = "API consumida cuando se integre el manager en chat_tools (fuera de scope E17b)"
    )]
    pub async fn call_tool(
        &self,
        qualified_name: &str,
        arguments: serde_json::Value,
    ) -> anyhow::Result<String> {
        let (server, tool) = match qualified_name.split_once('/') {
            Some((s, t)) => (Some(s.to_string()), t.to_string()),
            None => (None, qualified_name.to_string()),
        };
        let client = self.resolve_client(server.as_deref(), &tool)?;
        client.call_tool(&tool, arguments).await
    }

    fn resolve_client(&self, server: Option<&str>, tool: &str) -> anyhow::Result<Arc<McpClient>> {
        let inner = self.read_inner();
        if let Some(name) = server {
            let h = inner
                .handles
                .get(name)
                .ok_or_else(|| anyhow::anyhow!("MCP server '{name}' no configurado"))?;
            if !h.state.is_ready() {
                anyhow::bail!("MCP server '{name}' no ready ({})", h.state.label());
            }
            if !h.tools.iter().any(|t| t == tool) {
                anyhow::bail!("MCP server '{name}' no expone tool '{tool}'");
            }
            return h
                .client
                .clone()
                .ok_or_else(|| anyhow::anyhow!("MCP server '{name}' sin client"));
        }
        // Lookup: primer Ready con el tool.
        for h in inner.handles.values() {
            if h.state.is_ready() && h.tools.iter().any(|t| t == tool) {
                if let Some(c) = h.client.clone() {
                    return Ok(c);
                }
            }
        }
        anyhow::bail!("ningun server MCP ready expone tool '{tool}'")
    }

    // ── API de testing: inyeccion directa de estado ──────────────────────
    #[cfg(test)]
    pub(super) fn insert_test_handle(&self, name: &str, state: ServerState, tools: Vec<String>) {
        let mut inner = self.write_inner();
        inner.handles.insert(
            name.to_string(),
            ServerHandle {
                config: ServerConfig {
                    name: name.to_string(),
                    enabled: true,
                    kind: ServerKind::Sse { url: "http://test".into() },
                },
                state,
                client: None,
                tools,
                attempts: 0,
            },
        );
    }
}

impl Default for McpLifecycleManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Conecta segun el kind y devuelve (client, lista de tools).
async fn connect_server(cfg: &ServerConfig) -> anyhow::Result<(McpClient, Vec<String>)> {
    let transport: Box<dyn McpTransport> = match &cfg.kind {
        ServerKind::Sse { url } => Box::new(SseTransport::connect(url).await?),
        ServerKind::Stdio { command, args } => {
            let argv: Vec<&str> = args.iter().map(String::as_str).collect();
            Box::new(StdioTransport::spawn(command, &argv).await?)
        }
        ServerKind::WebSocket { url } => Box::new(WebSocketTransport::connect(url).await?),
    };
    let client = McpClient::from_transport(transport).await?;
    #[cfg(feature = "mcp")]
    let tools = client.list_tools().await.unwrap_or_default().into_iter().map(|t| t.name).collect();
    #[cfg(not(feature = "mcp"))]
    let tools: Vec<String> = Vec::new();
    Ok((client, tools))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_manager_has_empty_snapshot() {
        let mgr = McpLifecycleManager::new();
        let snap = mgr.snapshot();
        assert!(snap.servers.is_empty());
        assert_eq!(snap.ready_ratio(), (0, 0));
    }

    #[test]
    fn snapshot_reports_degraded_mode() {
        let mgr = McpLifecycleManager::new();
        mgr.insert_test_handle(
            "a",
            ServerState::Ready { tools_count: 3 },
            vec!["t1".into(), "t2".into(), "t3".into()],
        );
        mgr.insert_test_handle(
            "b",
            ServerState::Failed { reason: "boom".into(), attempts: 2 },
            vec![],
        );
        let snap = mgr.snapshot();
        assert_eq!(snap.servers.len(), 2);
        assert_eq!(snap.ready_ratio(), (1, 2));
        assert_eq!(snap.total_tools(), 3);
        assert!(snap.is_degraded());
    }

    #[test]
    fn resolve_unknown_server_errors() {
        let mgr = McpLifecycleManager::new();
        let res = mgr.resolve_client(Some("nope"), "x");
        assert!(res.is_err());
        let err = res.err().expect("expected error").to_string();
        assert!(err.contains("no configurado"));
    }

    #[test]
    fn resolve_not_ready_errors() {
        let mgr = McpLifecycleManager::new();
        mgr.insert_test_handle("a", ServerState::Connecting, vec![]);
        let res = mgr.resolve_client(Some("a"), "x");
        let err = res.err().expect("expected error").to_string();
        assert!(err.contains("no ready"));
    }

    #[test]
    fn resolve_tool_not_found_errors() {
        let mgr = McpLifecycleManager::new();
        mgr.insert_test_handle("a", ServerState::Ready { tools_count: 1 }, vec!["only".into()]);
        let res = mgr.resolve_client(Some("a"), "other");
        let err = res.err().expect("expected error").to_string();
        assert!(err.contains("no expone tool"));
    }

    #[test]
    fn unqualified_lookup_without_match_errors() {
        let mgr = McpLifecycleManager::new();
        mgr.insert_test_handle("a", ServerState::Ready { tools_count: 0 }, vec![]);
        let res = mgr.resolve_client(None, "ghost");
        let err = res.err().expect("expected error").to_string();
        assert!(err.contains("ningun server"));
    }

    #[tokio::test]
    async fn start_all_honors_disabled_flag() {
        let mgr = McpLifecycleManager::new();
        mgr.start_all(vec![ServerConfig {
            name: "off".into(),
            enabled: false,
            kind: ServerKind::Sse { url: "http://x".into() },
        }])
        .await;
        let snap = mgr.snapshot();
        assert_eq!(snap.servers.len(), 1);
        assert!(matches!(snap.servers[0].state, ServerState::Disabled));
        assert_eq!(snap.ready_ratio(), (0, 0)); // disabled excluido
    }
}
