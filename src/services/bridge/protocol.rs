//! IDE Bridge protocol types (E27).
//!
//! Define los mensajes JSON que intercambian la TUI (server) y el IDE
//! (client, ej: VS Code extension). Todos los endpoints reciben y
//! retornan JSON.

use serde::{Deserialize, Serialize};

/// POST /api/context — el IDE envia contexto adicional (archivo abierto, etc).
#[derive(Debug, Deserialize)]
pub struct ContextUpdate {
    /// Tipo de contexto: "active_file", "selection", "terminal_output".
    pub kind: String,
    /// Path absoluto o relativo del archivo (si aplica).
    pub path: Option<String>,
    /// Contenido textual del contexto.
    pub content: Option<String>,
}

/// POST /api/tool_approval — respuesta del IDE a un permiso pendiente.
#[derive(Debug, Deserialize)]
pub struct ToolApproval {
    /// ID del tool_call que se aprueba/deniega.
    pub tool_call_id: String,
    /// true = approve, false = deny.
    pub approved: bool,
}

/// POST /api/file_open — la TUI pide al IDE que abra un archivo.
#[allow(dead_code, reason = "consumido por extension VS Code en Sprint 13")]
#[derive(Debug, Serialize)]
pub struct FileOpenRequest {
    pub path: String,
    pub line: Option<u32>,
    pub column: Option<u32>,
}

/// GET /api/pending_approvals — lista de tools pendientes de aprobacion.
#[derive(Debug, Clone, Serialize)]
pub struct PendingApprovalItem {
    pub tool_call_id: String,
    pub tool_name: String,
    pub arguments: String,
    pub permission: String,
    pub reason: Option<String>,
}

/// GET /api/status — health del bridge.
#[derive(Debug, Serialize)]
pub struct BridgeStatus {
    pub version: &'static str,
    pub app_screen: String,
    pub chat_status: String,
    pub diagnostics_count: usize,
    pub monitors_active: usize,
    pub agents_active: usize,
    pub pending_approvals: Vec<PendingApprovalItem>,
}

/// Respuesta generica para endpoints que solo confirman recepcion.
#[derive(Debug, Serialize)]
pub struct AckResponse {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl AckResponse {
    pub fn ok() -> Self {
        Self { ok: true, message: None }
    }

    #[allow(dead_code, reason = "consumido por endpoints futuros (file_open)")]
    pub fn with_message(msg: impl Into<String>) -> Self {
        Self { ok: true, message: Some(msg.into()) }
    }

    #[allow(dead_code, reason = "consumido por validacion de endpoints futuros")]
    pub fn error(msg: impl Into<String>) -> Self {
        Self { ok: false, message: Some(msg.into()) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ack_ok_serializes() {
        let json = serde_json::to_string(&AckResponse::ok()).unwrap();
        assert!(json.contains("\"ok\":true"));
        assert!(!json.contains("message"));
    }

    #[test]
    fn ack_error_serializes() {
        let json = serde_json::to_string(&AckResponse::error("bad")).unwrap();
        assert!(json.contains("\"ok\":false"));
        assert!(json.contains("bad"));
    }

    #[test]
    fn context_update_deserializes() {
        let json = r#"{"kind":"active_file","path":"/tmp/x.rs","content":"fn main() {}"}"#;
        let ctx: ContextUpdate = serde_json::from_str(json).unwrap();
        assert_eq!(ctx.kind, "active_file");
        assert_eq!(ctx.path.as_deref(), Some("/tmp/x.rs"));
    }

    #[test]
    fn tool_approval_deserializes() {
        let json = r#"{"tool_call_id":"tc1","approved":true}"#;
        let approval: ToolApproval = serde_json::from_str(json).unwrap();
        assert_eq!(approval.tool_call_id, "tc1");
        assert!(approval.approved);
    }

    #[test]
    fn bridge_status_serializes() {
        let status = BridgeStatus {
            version: env!("CARGO_PKG_VERSION"),
            app_screen: "Chat".into(),
            chat_status: "Ready".into(),
            diagnostics_count: 5,
            monitors_active: 1,
            agents_active: 0,
            pending_approvals: vec![],
        };
        let json = serde_json::to_string(&status).unwrap();
        let expected = format!("\"version\":\"{}\"", env!("CARGO_PKG_VERSION"));
        assert!(json.contains(&expected));
    }
}
