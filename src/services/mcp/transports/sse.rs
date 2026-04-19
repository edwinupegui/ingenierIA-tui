//! Transporte SSE: conexión al endpoint `/claude/sse`, obtiene endpoint POST
//! via primer SSE event, luego envía requests por HTTP POST y recibe las
//! respuestas JSON-RPC por el mismo SSE stream.
//!
//! Protocolo real del MCP SDK (Server-Sent Events transport):
//! 1. Cliente hace GET al `/claude/sse` → server responde con stream de eventos
//!    y envía `event: endpoint\ndata: /path?sessionId=X\n\n` como primer mensaje.
//! 2. Cliente POST al endpoint con el JSON-RPC request → server responde
//!    inmediatamente `202 Accepted` (body vacío).
//! 3. Server eventualmente envía la respuesta JSON-RPC por el SSE stream:
//!    `event: message\ndata: {"jsonrpc":"2.0","id":N,"result":...}\n\n`.
//! 4. Cliente correlaciona por `id`.
//!
//! Esta implementación mantiene la conexión SSE viva en una task background
//! que parsea eventos y enruta cada respuesta por `id` a un `oneshot::Sender`
//! registrado al hacer POST.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use futures_util::StreamExt;
use tokio::sync::{oneshot, Mutex};

use super::super::protocol::{JsonRpcRequest, JsonRpcResponse};
use super::super::transport::{McpTransport, TransportKind};

const MCP_HTTP_TIMEOUT: Duration = Duration::from_secs(30);
const MCP_SSE_INIT_DEADLINE: Duration = Duration::from_secs(10);
const MCP_SSE_CHUNK_TIMEOUT: Duration = Duration::from_secs(5);
const MCP_RESPONSE_TIMEOUT: Duration = Duration::from_secs(60);

type PendingMap = Arc<Mutex<HashMap<u64, oneshot::Sender<JsonRpcResponse>>>>;

pub struct SseTransport {
    post_url: String,
    http: reqwest::Client,
    pending: PendingMap,
    /// Tarea que drena el SSE stream + enruta respuestas. Se cancela en drop.
    _reader_task: tokio::task::JoinHandle<()>,
}

impl Drop for SseTransport {
    fn drop(&mut self) {
        self._reader_task.abort();
    }
}

impl SseTransport {
    pub async fn connect(base_url: &str) -> anyhow::Result<Self> {
        let base_url = base_url.trim_end_matches('/').to_string();
        let http = reqwest::Client::builder().timeout(MCP_HTTP_TIMEOUT).build()?;
        let pending: PendingMap = Arc::new(Mutex::new(HashMap::new()));
        let (endpoint_path, reader_task) = establish_sse(&http, &base_url, pending.clone()).await?;
        let post_url = join_base_and_endpoint(&base_url, &endpoint_path);
        Ok(Self { post_url, http, pending, _reader_task: reader_task })
    }
}

#[async_trait]
impl McpTransport for SseTransport {
    async fn send_request(&self, request: JsonRpcRequest) -> anyhow::Result<JsonRpcResponse> {
        let id = request.id;
        let (tx, rx) = oneshot::channel();
        self.pending.lock().await.insert(id, tx);

        let post_result = self.http.post(&self.post_url).json(&request).send().await;
        if let Err(e) = post_result.as_ref() {
            self.pending.lock().await.remove(&id);
            return Err(anyhow::anyhow!("POST error: {e}"));
        }
        let resp = post_result?;
        if !resp.status().is_success() {
            self.pending.lock().await.remove(&id);
            anyhow::bail!("HTTP {} posting MCP request", resp.status());
        }

        match tokio::time::timeout(MCP_RESPONSE_TIMEOUT, rx).await {
            Ok(Ok(rpc_resp)) => Ok(rpc_resp),
            Ok(Err(_)) => anyhow::bail!("MCP reader task dropped while awaiting response"),
            Err(_) => {
                self.pending.lock().await.remove(&id);
                anyhow::bail!("Timeout waiting for MCP response id={id}")
            }
        }
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
        self.http.post(&self.post_url).json(&body).send().await?.error_for_status()?;
        Ok(())
    }

    fn kind(&self) -> TransportKind {
        TransportKind::Sse
    }
}

fn join_base_and_endpoint(base_url: &str, endpoint_path: &str) -> String {
    if endpoint_path.starts_with("http://") || endpoint_path.starts_with("https://") {
        return endpoint_path.to_string();
    }
    let trimmed = base_url.trim_end_matches('/');
    if endpoint_path.starts_with('/') {
        format!("{trimmed}{endpoint_path}")
    } else {
        format!("{trimmed}/{endpoint_path}")
    }
}

async fn establish_sse(
    http: &reqwest::Client,
    base_url: &str,
    pending: PendingMap,
) -> anyhow::Result<(String, tokio::task::JoinHandle<()>)> {
    let sse_url = format!("{base_url}/claude/sse");
    let resp = http
        .get(&sse_url)
        .header("Accept", "text/event-stream")
        .send()
        .await?
        .error_for_status()?;

    let mut stream = resp.bytes_stream();
    let mut buffer = String::new();
    let deadline = tokio::time::Instant::now() + MCP_SSE_INIT_DEADLINE;

    loop {
        if tokio::time::Instant::now() > deadline {
            anyhow::bail!("Timeout waiting for MCP SSE endpoint event");
        }
        let chunk = tokio::time::timeout(MCP_SSE_CHUNK_TIMEOUT, stream.next()).await;
        match chunk {
            Ok(Some(Ok(bytes))) => {
                buffer.push_str(&String::from_utf8_lossy(&bytes));
            }
            _ => anyhow::bail!("SSE stream ended before endpoint established"),
        }
        if let Some(endpoint) = extract_endpoint_path(&buffer) {
            let endpoint_len = find_end_of_endpoint_event(&buffer);
            let leftover = buffer.split_off(endpoint_len);
            // Spawn reader task that parses `event: message` chunks and
            // routes by JSON-RPC id.
            let reader = tokio::spawn(async move {
                reader_loop(stream, leftover, pending).await;
            });
            return Ok((endpoint, reader));
        }
    }
}

fn extract_endpoint_path(buffer: &str) -> Option<String> {
    for line in buffer.lines() {
        let Some(data) = line.strip_prefix("data: ") else {
            continue;
        };
        let data = data.trim();
        if data.contains("sessionId=") {
            return Some(data.to_string());
        }
    }
    None
}

/// Devuelve el offset (en bytes) inmediatamente posterior al bloque del
/// primer evento `endpoint`, para que el reader_loop arranque leyendo desde
/// los bytes restantes (si llegaron combinados en el mismo chunk).
fn find_end_of_endpoint_event(buffer: &str) -> usize {
    buffer.find("\n\n").map(|p| p + 2).unwrap_or_else(|| buffer.len())
}

/// Parsea eventos SSE del stream y enruta respuestas por id.
async fn reader_loop<S>(mut stream: S, leftover: String, pending: PendingMap)
where
    S: futures_util::Stream<Item = reqwest::Result<bytes::Bytes>> + Unpin,
{
    let mut buffer = leftover;
    loop {
        match stream.next().await {
            Some(Ok(bytes)) => {
                buffer.push_str(&String::from_utf8_lossy(&bytes));
                drain_events(&mut buffer, &pending).await;
            }
            Some(Err(e)) => {
                tracing::debug!(error = %e, "SSE reader error, closing");
                return;
            }
            None => return,
        }
    }
}

async fn drain_events(buffer: &mut String, pending: &PendingMap) {
    // SSE events separados por doble newline.
    while let Some(pos) = buffer.find("\n\n") {
        let (raw_event, rest) = buffer.split_at(pos + 2);
        let event_block = raw_event.to_string();
        *buffer = rest.to_string();

        if let Some(payload) = payload_of_message_event(&event_block) {
            route_message(&payload, pending).await;
        }
    }
}

/// Extrae el payload `data: {...}` de un evento `event: message`.
fn payload_of_message_event(block: &str) -> Option<String> {
    let mut is_message = false;
    let mut data_lines: Vec<&str> = Vec::new();
    for line in block.lines() {
        if let Some(t) = line.strip_prefix("event: ") {
            if t.trim() == "message" {
                is_message = true;
            }
        } else if let Some(d) = line.strip_prefix("data: ") {
            data_lines.push(d);
        }
    }
    // Aceptamos también eventos sin `event:` explícito cuyo data sea JSON-RPC.
    if data_lines.is_empty() {
        return None;
    }
    let payload = data_lines.join("\n");
    if is_message || payload.trim_start().starts_with('{') {
        Some(payload)
    } else {
        None
    }
}

async fn route_message(payload: &str, pending: &PendingMap) {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(payload) else {
        tracing::debug!(%payload, "SSE payload no es JSON, ignorado");
        return;
    };
    let Some(id) = value.get("id").and_then(|v| v.as_u64()) else {
        // Es una notificación del server; ignorar por ahora.
        return;
    };
    let resp: JsonRpcResponse = match serde_json::from_value(value) {
        Ok(r) => r,
        Err(e) => {
            tracing::debug!(error = %e, "no se pudo parsear JsonRpcResponse");
            return;
        }
    };
    if let Some(tx) = pending.lock().await.remove(&id) {
        let _ = tx.send(resp);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_claude_prefixed_endpoint() {
        let buf = "event: endpoint\ndata: /claude/messages?sessionId=abc123\n\n";
        assert_eq!(
            extract_endpoint_path(buf),
            Some("/claude/messages?sessionId=abc123".to_string())
        );
    }

    #[test]
    fn extracts_unprefixed_endpoint() {
        let buf = "event: endpoint\ndata: /messages?sessionId=xyz\n\n";
        assert_eq!(extract_endpoint_path(buf), Some("/messages?sessionId=xyz".to_string()));
    }

    #[test]
    fn extract_session_missing_returns_none() {
        assert_eq!(extract_endpoint_path("event: ping\n\n"), None);
    }

    #[test]
    fn join_preserves_path() {
        assert_eq!(
            join_base_and_endpoint("http://host:3001", "/claude/messages?sessionId=x"),
            "http://host:3001/claude/messages?sessionId=x"
        );
    }

    #[test]
    fn join_respects_absolute_endpoint() {
        assert_eq!(
            join_base_and_endpoint("http://host:3001", "http://other:9000/msg?sessionId=x"),
            "http://other:9000/msg?sessionId=x"
        );
    }

    #[test]
    fn join_trims_trailing_slash() {
        assert_eq!(
            join_base_and_endpoint("http://host:3001/", "/messages?sessionId=x"),
            "http://host:3001/messages?sessionId=x"
        );
    }

    #[test]
    fn payload_of_message_parses_json() {
        let block = "event: message\ndata: {\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{}}\n\n";
        let p = payload_of_message_event(block).unwrap();
        assert!(p.starts_with('{'));
    }

    #[test]
    fn payload_of_endpoint_event_rejected() {
        let block = "event: endpoint\ndata: /claude/messages?sessionId=x\n\n";
        assert!(payload_of_message_event(block).is_none());
    }
}
