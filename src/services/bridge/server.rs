//! HTTP server del IDE Bridge (E27).
//!
//! Levanta un servidor axum en un puerto configurable (default 19542) que
//! expone la API JSON para integracion con IDEs. El server corre como un
//! tokio task y se apaga via shutdown signal.
//!
//! Endpoints:
//!   GET  /api/status          — health del bridge
//!   POST /api/context         — IDE envia contexto (archivo abierto, etc)
//!   POST /api/tool_approval   — IDE responde a un permiso pendiente
//!
//! El server recibe un `Sender<Action>` para emitir hacia el reducer,
//! y un snapshot de estado via `BridgeSnapshot` para responder GETs.

use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use tokio::sync::{mpsc::Sender, watch};

use super::protocol::{
    AckResponse, BridgeStatus, ContextUpdate, PendingApprovalItem, ToolApproval,
};
use crate::actions::Action;

/// Puerto por defecto del bridge. Elegido para evitar colisiones comunes.
pub const DEFAULT_PORT: u16 = 19542;

/// Snapshot minimo de estado que el server necesita para responder GETs.
/// Publicado periodicamente via `watch::Sender` desde el reducer.
#[derive(Debug, Clone, Default)]
pub struct BridgeSnapshot {
    pub app_screen: String,
    pub chat_status: String,
    pub diagnostics_count: usize,
    pub monitors_active: usize,
    pub agents_active: usize,
    pub pending_approvals: Vec<PendingApprovalItem>,
}

/// Estado compartido entre los handlers de axum.
#[derive(Clone)]
struct AppContext {
    tx: Sender<Action>,
    state_rx: watch::Receiver<BridgeSnapshot>,
}

/// Spawnea el bridge server. Retorna el `watch::Sender` para publicar
/// snapshots de estado y un `JoinHandle` para poder cancel.
pub fn spawn_bridge_server(
    port: u16,
    tx: Sender<Action>,
) -> (watch::Sender<BridgeSnapshot>, tokio::task::JoinHandle<()>) {
    let (state_tx, state_rx) = watch::channel(BridgeSnapshot::default());
    let ctx = AppContext { tx, state_rx };

    let handle = tokio::spawn(async move {
        let app = Router::new()
            .route("/api/status", get(handle_status))
            .route("/api/context", post(handle_context))
            .route("/api/tool_approval", post(handle_tool_approval))
            .route("/api/pending_approvals", get(handle_pending_approvals))
            .with_state(ctx);

        let addr = format!("127.0.0.1:{port}");
        let listener = match tokio::net::TcpListener::bind(&addr).await {
            Ok(l) => l,
            Err(e) => {
                tracing::error!(addr, err = %e, "IDE Bridge bind fallo");
                return;
            }
        };
        tracing::info!(addr, "IDE Bridge server escuchando");
        let _ = axum::serve(listener, app).await;
    });

    (state_tx, handle)
}

async fn handle_status(State(ctx): State<AppContext>) -> Json<BridgeStatus> {
    let snap = ctx.state_rx.borrow().clone();
    Json(BridgeStatus {
        version: env!("CARGO_PKG_VERSION"),
        app_screen: snap.app_screen,
        chat_status: snap.chat_status,
        diagnostics_count: snap.diagnostics_count,
        monitors_active: snap.monitors_active,
        agents_active: snap.agents_active,
        pending_approvals: snap.pending_approvals,
    })
}

async fn handle_pending_approvals(State(ctx): State<AppContext>) -> Json<Vec<PendingApprovalItem>> {
    let snap = ctx.state_rx.borrow().clone();
    Json(snap.pending_approvals)
}

async fn handle_context(
    State(ctx): State<AppContext>,
    Json(update): Json<ContextUpdate>,
) -> (StatusCode, Json<AckResponse>) {
    let _ = ctx
        .tx
        .send(Action::BridgeContextUpdate {
            kind: update.kind,
            path: update.path,
            content: update.content,
        })
        .await;
    (StatusCode::OK, Json(AckResponse::ok()))
}

async fn handle_tool_approval(
    State(ctx): State<AppContext>,
    Json(approval): Json<ToolApproval>,
) -> (StatusCode, Json<AckResponse>) {
    let _ = ctx
        .tx
        .send(Action::BridgeToolApproval {
            tool_call_id: approval.tool_call_id,
            approved: approval.approved,
        })
        .await;
    (StatusCode::OK, Json(AckResponse::ok()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_port_is_reasonable() {
        // DEFAULT_PORT es una constante validada en compilacion.
        assert_eq!(DEFAULT_PORT, 19542);
    }

    #[test]
    fn bridge_snapshot_default_has_empty_strings() {
        let snap = BridgeSnapshot::default();
        assert!(snap.app_screen.is_empty());
        assert_eq!(snap.monitors_active, 0);
    }
}
