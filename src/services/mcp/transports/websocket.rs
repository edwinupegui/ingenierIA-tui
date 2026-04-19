//! Transporte WebSocket: conecta a un MCP server via `ws://` o `wss://`.
//!
//! Protocolo: cada mensaje es una linea JSON-RPC 2.0 (request, response o
//! notification). El servidor y el cliente comparten un canal full-duplex.
//!
//! Para el MVP serializamos las requests (igual que `StdioTransport`):
//! un `Mutex` envuelve sink+stream, y cada `send_request` escribe y lee
//! hasta encontrar una respuesta. Esto simplifica el matching por id a
//! costa de concurrencia — suficiente para servers MCP locales.

#![allow(dead_code, reason = "E17a WebSocket transport — integracion con /connect-mcp pendiente")]

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use futures_util::stream::{SplitSink, SplitStream};
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio_tungstenite::tungstenite::protocol::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

use super::super::protocol::{JsonRpcRequest, JsonRpcResponse};
use super::super::transport::{McpTransport, TransportKind};

const WS_READ_TIMEOUT: Duration = Duration::from_secs(30);
const WS_MAX_SKIPPED_MESSAGES: usize = 16;

type WsStream = WebSocketStream<MaybeTlsStream<TcpStream>>;
type WsSink = SplitSink<WsStream, Message>;
type WsSource = SplitStream<WsStream>;

/// Transporte JSON-RPC sobre WebSocket full-duplex.
pub struct WebSocketTransport {
    sink: Arc<Mutex<WsSink>>,
    stream: Arc<Mutex<WsSource>>,
}

impl WebSocketTransport {
    /// Conecta a `url` (ws:// o wss://). Falla si el handshake no completa.
    pub async fn connect(url: &str) -> anyhow::Result<Self> {
        let (ws, _resp) = tokio_tungstenite::connect_async(url).await?;
        let (sink, stream) = ws.split();
        Ok(Self { sink: Arc::new(Mutex::new(sink)), stream: Arc::new(Mutex::new(stream)) })
    }

    /// Envia un payload JSON como frame de texto.
    async fn write_text(&self, payload: &str) -> anyhow::Result<()> {
        let mut sink = self.sink.lock().await;
        sink.send(Message::Text(payload.to_string())).await?;
        Ok(())
    }

    /// Lee el siguiente mensaje de texto, ignorando pings/pongs/binarios.
    async fn read_text(&self) -> anyhow::Result<String> {
        let mut stream = self.stream.lock().await;
        let fut = async {
            while let Some(msg) = stream.next().await {
                match msg? {
                    Message::Text(t) => return Ok(t),
                    Message::Close(_) => anyhow::bail!("WebSocket cerrado por el server"),
                    // Silenciar Ping/Pong/Binary/Frame: no son JSON-RPC.
                    _ => continue,
                }
            }
            anyhow::bail!("WebSocket stream terminado sin respuesta");
        };
        tokio::time::timeout(WS_READ_TIMEOUT, fut)
            .await
            .map_err(|_| anyhow::anyhow!("Timeout leyendo del WebSocket"))?
    }
}

#[async_trait]
impl McpTransport for WebSocketTransport {
    async fn send_request(&self, request: JsonRpcRequest) -> anyhow::Result<JsonRpcResponse> {
        let payload = serde_json::to_string(&request)?;
        self.write_text(&payload).await?;

        // Lee hasta encontrar un JSON-RPC response valido. Notifications o
        // mensajes del server se descartan (max N skips para no colgar).
        for _ in 0..WS_MAX_SKIPPED_MESSAGES {
            let raw = self.read_text().await?;
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                continue;
            }
            let Ok(resp) = serde_json::from_str::<JsonRpcResponse>(trimmed) else {
                // Linea no era respuesta (log/notification). Saltar.
                continue;
            };
            return Ok(resp);
        }
        anyhow::bail!("no JSON-RPC response tras {} mensajes", WS_MAX_SKIPPED_MESSAGES)
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
        let payload = serde_json::to_string(&body)?;
        self.write_text(&payload).await
    }

    fn kind(&self) -> TransportKind {
        TransportKind::WebSocket
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpListener;
    use tokio_tungstenite::accept_async;

    /// Servidor de prueba que acepta una conexion, espera un request JSON-RPC
    /// y responde con un result configurable. Retorna el `ws://` URL.
    async fn spawn_echo_server(
        response_result: serde_json::Value,
    ) -> (String, tokio::task::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        let handle = tokio::spawn(async move {
            if let Ok((stream, _)) = listener.accept().await {
                if let Ok(ws) = accept_async(stream).await {
                    let (mut sink, mut src) = ws.split();
                    while let Some(Ok(msg)) = src.next().await {
                        if let Message::Text(text) = msg {
                            let req: serde_json::Value =
                                serde_json::from_str(&text).expect("valid jsonrpc");
                            let id = req.get("id").cloned().unwrap_or(serde_json::json!(1));
                            let resp = serde_json::json!({
                                "jsonrpc": "2.0",
                                "id": id,
                                "result": response_result.clone(),
                            });
                            let _ = sink.send(Message::Text(resp.to_string())).await;
                            break;
                        }
                    }
                }
            }
        });
        (format!("ws://{addr}"), handle)
    }

    #[tokio::test]
    async fn kind_is_websocket() {
        let (url, server) = spawn_echo_server(serde_json::json!({"ok": true})).await;
        let t = WebSocketTransport::connect(&url).await.expect("connect");
        assert_eq!(t.kind(), TransportKind::WebSocket);
        drop(t);
        let _ = server.await;
    }

    #[tokio::test]
    async fn send_request_roundtrip() {
        let (url, server) = spawn_echo_server(serde_json::json!({"ok": 42})).await;
        let t = WebSocketTransport::connect(&url).await.expect("connect");
        let req = JsonRpcRequest::new(1, "ping", serde_json::json!({}));
        let resp = t.send_request(req).await.expect("roundtrip");
        let result = resp.result.expect("result");
        assert_eq!(result, serde_json::json!({"ok": 42}));
        drop(t);
        let _ = server.await;
    }

    #[tokio::test]
    async fn invalid_url_fails_fast() {
        let err = WebSocketTransport::connect("ws://127.0.0.1:1").await;
        assert!(err.is_err(), "conexion a puerto cerrado debe fallar");
    }
}
