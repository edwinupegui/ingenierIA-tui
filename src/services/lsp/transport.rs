//! JSON-RPC framing sobre stdio (E25).
//!
//! Implementa el protocolo base LSP: Content-Length header + JSON body,
//! bidireccional sobre stdin/stdout de un child process.
//!
//! Solo soporta el subconjunto necesario para el client (no server):
//! - `send_request` — envia request y retorna response (con correlacion por id)
//! - `send_notification` — fire-and-forget
//! - `read_message` — lee el proximo mensaje (response o notification)

use anyhow::{anyhow, Result};
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{ChildStdin, ChildStdout};

/// Writer: serializa un JSON-RPC message a stdout del child.
pub struct LspWriter {
    stdin: ChildStdin,
}

impl LspWriter {
    pub fn new(stdin: ChildStdin) -> Self {
        Self { stdin }
    }

    /// Envia un request JSON-RPC (con id).
    pub async fn send_request(&mut self, id: u64, method: &str, params: Value) -> Result<()> {
        let msg = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });
        self.write_message(&msg).await
    }

    /// Envia una notificacion JSON-RPC (sin id).
    pub async fn send_notification(&mut self, method: &str, params: Value) -> Result<()> {
        let msg = serde_json::json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        });
        self.write_message(&msg).await
    }

    async fn write_message(&mut self, msg: &Value) -> Result<()> {
        let body = serde_json::to_string(msg)?;
        let header = format!("Content-Length: {}\r\n\r\n", body.len());
        self.stdin.write_all(header.as_bytes()).await?;
        self.stdin.write_all(body.as_bytes()).await?;
        self.stdin.flush().await?;
        Ok(())
    }
}

/// Reader: parsea mensajes JSON-RPC de stdout del child.
pub struct LspReader {
    reader: BufReader<ChildStdout>,
}

/// Tipo de mensaje recibido.
#[derive(Debug)]
#[allow(dead_code, reason = "id+result leidos por tests y por handshake en client.rs")]
pub enum LspMessage {
    /// Respuesta a un request previo (contiene id + result|error).
    Response { id: u64, result: Option<Value>, error: Option<Value> },
    /// Notificacion del server (sin id).
    Notification { method: String, params: Value },
}

impl LspReader {
    pub fn new(stdout: ChildStdout) -> Self {
        Self { reader: BufReader::new(stdout) }
    }

    /// Lee el proximo mensaje completo. Bloquea hasta recibir uno.
    pub async fn read_message(&mut self) -> Result<LspMessage> {
        let content_length = self.read_headers().await?;
        let mut buf = vec![0u8; content_length];
        self.reader.read_exact(&mut buf).await?;
        let json: Value = serde_json::from_slice(&buf)?;
        parse_message(json)
    }

    /// Parsea headers HTTP-like hasta encontrar Content-Length.
    async fn read_headers(&mut self) -> Result<usize> {
        let mut content_length: Option<usize> = None;
        let mut line = String::new();
        loop {
            line.clear();
            let n = self.reader.read_line(&mut line).await?;
            if n == 0 {
                return Err(anyhow!("EOF leyendo headers LSP"));
            }
            let trimmed = line.trim();
            if trimmed.is_empty() {
                break;
            }
            if let Some(val) = trimmed.strip_prefix("Content-Length:") {
                content_length = val.trim().parse().ok();
            }
        }
        content_length.ok_or_else(|| anyhow!("Content-Length no encontrado"))
    }
}

fn parse_message(json: Value) -> Result<LspMessage> {
    if let Some(id) = json.get("id").and_then(|v| v.as_u64()) {
        // Response (tiene id pero no method) o request del server (tiene method+id).
        if json.get("method").is_some() {
            // Server request — tratamos como notification para MVP.
            let method = json["method"].as_str().unwrap_or("").to_string();
            let params = json.get("params").cloned().unwrap_or(Value::Null);
            return Ok(LspMessage::Notification { method, params });
        }
        Ok(LspMessage::Response {
            id,
            result: json.get("result").cloned(),
            error: json.get("error").cloned(),
        })
    } else if let Some(method) = json.get("method").and_then(|v| v.as_str()) {
        let params = json.get("params").cloned().unwrap_or(Value::Null);
        Ok(LspMessage::Notification { method: method.to_string(), params })
    } else {
        Err(anyhow!("mensaje JSON-RPC invalido: sin id ni method"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_response_message() {
        let json = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": { "capabilities": {} }
        });
        let msg = parse_message(json).unwrap();
        match msg {
            LspMessage::Response { id, result, error } => {
                assert_eq!(id, 1);
                assert!(result.is_some());
                assert!(error.is_none());
            }
            _ => panic!("expected Response"),
        }
    }

    #[test]
    fn parse_notification_message() {
        let json = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/publishDiagnostics",
            "params": { "uri": "file:///x", "diagnostics": [] }
        });
        let msg = parse_message(json).unwrap();
        match msg {
            LspMessage::Notification { method, params } => {
                assert_eq!(method, "textDocument/publishDiagnostics");
                assert!(params.get("uri").is_some());
            }
            _ => panic!("expected Notification"),
        }
    }

    #[test]
    fn parse_error_response() {
        let json = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "error": { "code": -32600, "message": "invalid request" }
        });
        let msg = parse_message(json).unwrap();
        match msg {
            LspMessage::Response { id, error, .. } => {
                assert_eq!(id, 2);
                assert!(error.is_some());
            }
            _ => panic!("expected Response"),
        }
    }

    #[test]
    fn parse_server_request_as_notification() {
        // Some servers send requests with id + method (e.g. workspace/configuration).
        let json = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 5,
            "method": "workspace/configuration",
            "params": {}
        });
        let msg = parse_message(json).unwrap();
        assert!(matches!(msg, LspMessage::Notification { .. }));
    }

    #[test]
    fn parse_invalid_rejects() {
        let json = serde_json::json!({ "jsonrpc": "2.0" });
        assert!(parse_message(json).is_err());
    }
}
