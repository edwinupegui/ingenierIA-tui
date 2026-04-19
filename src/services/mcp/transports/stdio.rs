//! Transporte stdio: spawnea un proceso y habla JSON-RPC via stdin/stdout.
//!
//! Uso tipico: MCP servers locales como `@modelcontextprotocol/server-filesystem`
//! o servers custom en subprocess. Cada linea es un objeto JSON-RPC 2.0.

#![allow(dead_code, reason = "E17 stdio transport — integracion con /connect-mcp pendiente")]

use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::Mutex;

use super::super::protocol::{JsonRpcRequest, JsonRpcResponse};
use super::super::transport::{McpTransport, TransportKind};

/// Transporte via subprocess JSON-RPC.
pub struct StdioTransport {
    /// Handle al proceso hijo. Mantenido vivo mientras exista el transport.
    #[allow(dead_code, reason = "keep-alive handle del subprocess")]
    child: Arc<Mutex<Child>>,
    /// Stdin del subprocess (para escribir requests).
    stdin: Arc<Mutex<ChildStdin>>,
    /// Reader bufferado del stdout (para leer respuestas linea por linea).
    stdout: Arc<Mutex<BufReader<ChildStdout>>>,
    /// Contador interno de ids.
    next_id: Arc<AtomicU64>,
}

impl StdioTransport {
    /// Lanza el comando y establece comunicacion JSON-RPC.
    pub async fn spawn(command: &str, args: &[&str]) -> anyhow::Result<Self> {
        let mut cmd = Command::new(command);
        cmd.args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true);

        let mut child = cmd.spawn()?;
        let stdin = child.stdin.take().ok_or_else(|| anyhow::anyhow!("no stdin"))?;
        let stdout = child.stdout.take().ok_or_else(|| anyhow::anyhow!("no stdout"))?;

        Ok(Self {
            child: Arc::new(Mutex::new(child)),
            stdin: Arc::new(Mutex::new(stdin)),
            stdout: Arc::new(Mutex::new(BufReader::new(stdout))),
            next_id: Arc::new(AtomicU64::new(1)),
        })
    }

    /// Escribe una linea JSON al stdin y flushes.
    async fn write_line(&self, line: &str) -> anyhow::Result<()> {
        let mut stdin = self.stdin.lock().await;
        stdin.write_all(line.as_bytes()).await?;
        stdin.write_all(b"\n").await?;
        stdin.flush().await?;
        Ok(())
    }

    /// Lee una linea de stdout. Bloquea hasta recibir newline o EOF.
    async fn read_line(&self) -> anyhow::Result<String> {
        let mut stdout = self.stdout.lock().await;
        let mut buf = String::new();
        let n = stdout.read_line(&mut buf).await?;
        if n == 0 {
            anyhow::bail!("stdio subprocess closed stdout");
        }
        Ok(buf)
    }

    /// Genera el siguiente id JSON-RPC.
    pub fn next_id(&self) -> u64 {
        self.next_id.fetch_add(1, Ordering::Relaxed)
    }
}

#[async_trait]
impl McpTransport for StdioTransport {
    async fn send_request(&self, request: JsonRpcRequest) -> anyhow::Result<JsonRpcResponse> {
        let line = serde_json::to_string(&request)?;
        self.write_line(&line).await?;

        // Leer lineas hasta encontrar una que sea respuesta al id.
        for _ in 0..10 {
            let raw = self.read_line().await?;
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                continue;
            }
            let Ok(resp) = serde_json::from_str::<JsonRpcResponse>(trimmed) else {
                // Linea no era JSON-RPC valido (posible log del server). Saltar.
                continue;
            };
            return Ok(resp);
        }
        anyhow::bail!("no JSON-RPC response received after 10 lines")
    }

    async fn send_notification(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> anyhow::Result<()> {
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        });
        let line = serde_json::to_string(&body)?;
        self.write_line(&line).await?;
        Ok(())
    }

    fn kind(&self) -> TransportKind {
        TransportKind::Stdio
    }
}
