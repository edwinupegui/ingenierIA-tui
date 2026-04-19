//! Handlers del IDE Bridge (E27).
//!
//! Slash commands:
//!   /bridge-status  — info del bridge HTTP server
//!
//! Tambien maneja Actions emitidas por el server axum:
//!   BridgeContextUpdate, BridgeToolApproval.

use crate::state::{ChatMessage, ChatRole};

use super::App;

impl App {
    /// Inicia el bridge HTTP si el feature `ide` esta habilitado.
    /// Llamado una vez desde startup. Safe cuando no hay runtime (tests sync).
    #[cfg(feature = "ide")]
    pub(crate) fn try_start_bridge(&mut self) {
        if self.bridge_state_tx.is_some() {
            return;
        }
        // Guard: en tests sync no hay runtime; skip silencioso.
        if tokio::runtime::Handle::try_current().is_err() {
            return;
        }
        let port = crate::services::bridge::DEFAULT_PORT;
        let (state_tx, _handle) =
            crate::services::bridge::spawn_bridge_server(port, self.tx.clone());
        self.bridge_state_tx = Some(state_tx);
        self.notify(format!("IDE Bridge: http://127.0.0.1:{port}"));
    }

    #[cfg(not(feature = "ide"))]
    pub(crate) fn try_start_bridge(&mut self) {}

    /// Publica un snapshot del estado actual al bridge server.
    /// Llamado desde el tick para que el GET /api/status sea fresco.
    pub(crate) fn publish_bridge_snapshot(&self) {
        #[cfg(feature = "ide")]
        if let Some(tx) = &self.bridge_state_tx {
            let pending = self
                .state
                .chat
                .pending_approvals
                .iter()
                .map(|p| crate::services::bridge::protocol::PendingApprovalItem {
                    tool_call_id: p.tool_call_id.clone(),
                    tool_name: p.tool_name.clone(),
                    arguments: p.arguments.clone(),
                    permission: p.permission.clone(),
                    reason: p.reason.clone(),
                })
                .collect();
            let snap = crate::services::bridge::BridgeSnapshot {
                app_screen: format!("{:?}", self.state.screen),
                chat_status: format!("{:?}", self.state.chat.status),
                diagnostics_count: self.state.lsp.diagnostics.len(),
                monitors_active: self.state.monitors.active_count(),
                agents_active: self.state.agents.active_count(),
                pending_approvals: pending,
            };
            let _ = tx.send(snap);
        }
    }

    #[cfg(feature = "ide")]
    pub(crate) fn handle_bridge_context_update(
        &mut self,
        kind: String,
        path: Option<String>,
        content: Option<String>,
    ) {
        let label = path.as_deref().unwrap_or(&kind);
        let body = content.unwrap_or_default();
        if !body.is_empty() {
            let msg_text = format!("[IDE context: {label}]\n\n{body}");
            self.state.chat.messages.push(ChatMessage::new(ChatRole::User, msg_text));
            self.state.chat.scroll_offset = u16::MAX;
        }
        self.notify(format!("IDE: contexto '{kind}' recibido"));
    }

    #[cfg(feature = "ide")]
    pub(crate) fn handle_bridge_tool_approval(&mut self, tool_call_id: String, approved: bool) {
        let action_label = if approved { "aprobado" } else { "denegado" };
        tracing::info!(tool_call_id, approved, "bridge tool approval");

        // Find the matching pending approval by tool_call_id.
        let idx =
            self.state.chat.pending_approvals.iter().position(|p| p.tool_call_id == tool_call_id);

        let Some(idx) = idx else {
            self.notify(format!("IDE: tool {tool_call_id} no pendiente (ya resuelto)"));
            return;
        };

        let approval = self.state.chat.pending_approvals.remove(idx);
        self.notify(format!("IDE: {} {action_label}", approval.tool_name));

        if approved {
            // Execute the approved tool (same flow as approve_pending_tools).
            let tx = self.tx.clone();
            let pool = self.mcp_pool.clone();
            let hooks = self.hooks.clone();
            tokio::spawn(async move {
                hooks.fire(
                    crate::services::hooks::HookTrigger::PreToolUse,
                    crate::services::hooks::HookContext::for_tool(
                        &approval.tool_name,
                        &approval.arguments,
                    ),
                    tx.clone(),
                );
                let started = std::time::Instant::now();
                let registry = crate::services::tools::ToolRegistry::new();
                let result = if let Some(r) =
                    registry.execute(&approval.tool_name, &approval.arguments).await
                {
                    r
                } else {
                    super::chat_tools::execute_via_mcp(
                        &pool,
                        &approval.tool_name,
                        &approval.arguments,
                    )
                    .await
                };
                let duration_ms = started.elapsed().as_millis().min(u64::MAX as u128) as u64;
                let success = !result.to_lowercase().starts_with("error");
                hooks.fire(
                    crate::services::hooks::HookTrigger::PostToolUse,
                    crate::services::hooks::HookContext::for_tool_result(
                        &approval.tool_name,
                        &approval.arguments,
                        success,
                        duration_ms,
                    ),
                    tx.clone(),
                );
                let _ = tx
                    .send(crate::actions::Action::ChatToolResult {
                        tool_call_id: approval.tool_call_id,
                        content: result,
                    })
                    .await;
            });
        } else {
            self.state.chat.messages.push(ChatMessage::tool_result(
                approval.tool_call_id,
                format!("Tool {} denegado por IDE", approval.tool_name),
            ));
            self.maybe_continue_after_tools();
        }
    }
}
