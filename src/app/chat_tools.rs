use crate::state::{ChatMessage, ChatRole, ChatStatus};

use super::App;

/// Veces que un mismo (tool, args) debe repetirse seguido para considerarse doom loop.
const DOOM_LOOP_THRESHOLD: usize = 3;

/// Cuenta cuántas veces aparece (name, arguments) en los tool_calls de mensajes
/// assistant recientes. Se usa para detectar doom loops antes de ejecutar.
fn count_recent_identical_calls(messages: &[ChatMessage], name: &str, arguments: &str) -> usize {
    messages
        .iter()
        .rev()
        .filter_map(|m| if m.role == ChatRole::Assistant { Some(m.tool_calls.as_slice()) } else { None })
        .flatten()
        .filter(|tc| tc.name == name && tc.arguments == arguments)
        .count()
}

impl App {
    pub(crate) fn execute_pending_tool_calls(&mut self) {
        let tool_calls = if let Some(last) = self.state.chat.messages.last() {
            if last.role == ChatRole::Assistant {
                last.tool_calls.clone()
            } else {
                return;
            }
        } else {
            return;
        };

        // Run the enforcement pipeline per tool call.
        // Level 1: persistent rules (always_allow/deny)
        // Level 2: workspace boundary (for file tools)
        // Level 3: mode-based permission check
        // Level 4: bash validator pipeline (for shell tools)
        let registry = crate::services::tools::ToolRegistry::new();
        let enforcer = crate::services::permissions::PermissionEnforcer::new(
            crate::services::permissions::PermissionMode::WorkspaceWrite,
            std::env::current_dir().ok().map(|p| p.to_string_lossy().to_string()),
        );
        let mut auto_run = Vec::new();
        let mut auto_deny = Vec::new();
        let mut needs_approval = Vec::new();

        for tc in tool_calls {
            // Doom loop: si el mismo (tool, args) aparece >= THRESHOLD veces en el
            // historial reciente, escalar a aprobación manual para que el usuario
            // decida si continuar en lugar de girar infinitamente.
            let repeat_count =
                count_recent_identical_calls(&self.state.chat.messages, &tc.name, &tc.arguments);
            if repeat_count >= DOOM_LOOP_THRESHOLD {
                let reason = format!(
                    "Bucle detectado: `{}` se ha llamado {} veces con los mismos argumentos",
                    tc.name, repeat_count
                );
                needs_approval.push((tc, crate::services::tools::ToolPermission::Ask, reason, None));
                continue;
            }

            let tool_perm = registry
                .permission_for(&tc.name)
                .unwrap_or(crate::services::tools::ToolPermission::Ask);
            let (result, detail) = enforcer.check(&tc.name, tool_perm, &tc.arguments);
            match result {
                crate::services::permissions::EnforcementResult::Allow => {
                    auto_run.push(tc);
                }
                crate::services::permissions::EnforcementResult::Deny { reason } => {
                    auto_deny.push((tc, reason));
                }
                crate::services::permissions::EnforcementResult::PromptUser { reason } => {
                    // AgentMode::Auto: bypass approval, ejecutar directo.
                    if self.state.chat.agent_mode == crate::state::AgentMode::Auto {
                        auto_run.push(tc);
                    } else {
                        needs_approval.push((tc, tool_perm, reason, detail));
                    }
                }
            }
        }

        // Auto-deny tools blocked by the enforcer (persistent deny, workspace boundary, etc.)
        let had_auto_deny = !auto_deny.is_empty();
        for (tc, reason) in auto_deny {
            self.state.chat.messages.push(ChatMessage::tool_result(
                tc.id,
                format!("Tool {} denegado: {}", tc.name, reason),
            ));
        }
        if had_auto_deny {
            self.state.chat.snap_to_bottom();
        }

        // Queue tools that need approval (with enforcer details)
        if !needs_approval.is_empty() {
            for (tc, _perm, reason, detail) in &needs_approval {
                let label = detail
                    .as_ref()
                    .map(|d| d.risk_level.label().to_string())
                    .unwrap_or_else(|| "ask".to_string());
                let permission_label = format!("[{label}] ");
                self.state.chat.pending_approvals.push(crate::state::PendingToolApproval {
                    tool_call_id: tc.id.clone(),
                    tool_name: tc.name.clone(),
                    arguments: tc.arguments.clone(),
                    permission: label,
                    permission_label,
                    reason: Some(reason.clone()),
                    validator_reasons: detail
                        .as_ref()
                        .map(|d| d.reasons.clone())
                        .unwrap_or_default(),
                    selected: false,
                });
            }
            // Marca el inicio del timeout al entrar en la ventana de aprobación.
            if self.state.chat.approval_started_at.is_none() {
                self.state.chat.approval_started_at = Some(std::time::Instant::now());
                self.state.chat.approval_cursor = 0;
            }
        }

        // Auto-execute safe and always-allowed tools immediately
        if !auto_run.is_empty() {
            let tx = self.tx.clone();
            let pool = self.mcp_pool.clone();
            let hooks = self.hooks.clone();
            let config_snapshot = current_config_snapshot(&self.state);
            tokio::spawn(async move {
                let registry = crate::services::tools::ToolRegistry::new();
                for tc in auto_run {
                    hooks.fire(
                        crate::services::hooks::HookTrigger::PreToolUse,
                        crate::services::hooks::HookContext::for_tool(&tc.name, &tc.arguments),
                        tx.clone(),
                    );
                    let started = std::time::Instant::now();
                    let result = if tc.name == crate::services::tools::config_tool::CONFIG_TOOL_NAME
                    {
                        crate::services::tools::config_tool::handle_request(
                            &tc.arguments,
                            &config_snapshot,
                            &tx,
                        )
                        .await
                    } else if tc.name == crate::services::tools::todowrite::TODO_WRITE_NAME {
                        crate::services::tools::todowrite::handle_request(&tc.arguments, &tx).await
                    } else if let Some(r) = registry.execute(&tc.name, &tc.arguments).await {
                        r
                    } else {
                        execute_via_mcp(&pool, &tc.name, &tc.arguments).await
                    };
                    let duration_ms = started.elapsed().as_millis().min(u64::MAX as u128) as u64;
                    let success = !result.to_lowercase().starts_with("error")
                        && !result.to_lowercase().starts_with("mcp error");
                    hooks.fire(
                        crate::services::hooks::HookTrigger::PostToolUse,
                        crate::services::hooks::HookContext::for_tool_result(
                            &tc.name,
                            &tc.arguments,
                            success,
                            duration_ms,
                        ),
                        tx.clone(),
                    );
                    let _ = tx
                        .send(crate::actions::Action::ChatToolResult {
                            tool_call_id: tc.id,
                            content: result,
                        })
                        .await;
                }
            });
        }
        // If only pending approvals remain, UI will show the approval prompt
    }

    /// Execute all pending approved tool calls.
    pub(crate) fn approve_pending_tools(&mut self) {
        let approvals: Vec<_> = self.state.chat.pending_approvals.drain(..).collect();
        if approvals.is_empty() {
            return;
        }

        let tx = self.tx.clone();
        let pool = self.mcp_pool.clone();
        let hooks = self.hooks.clone();
        let config_snapshot = current_config_snapshot(&self.state);
        tokio::spawn(async move {
            let registry = crate::services::tools::ToolRegistry::new();
            for approval in approvals {
                hooks.fire(
                    crate::services::hooks::HookTrigger::PreToolUse,
                    crate::services::hooks::HookContext::for_tool(
                        &approval.tool_name,
                        &approval.arguments,
                    ),
                    tx.clone(),
                );
                let started = std::time::Instant::now();
                let result = if approval.tool_name
                    == crate::services::tools::config_tool::CONFIG_TOOL_NAME
                {
                    crate::services::tools::config_tool::handle_request(
                        &approval.arguments,
                        &config_snapshot,
                        &tx,
                    )
                    .await
                } else if approval.tool_name == crate::services::tools::todowrite::TODO_WRITE_NAME {
                    crate::services::tools::todowrite::handle_request(&approval.arguments, &tx)
                        .await
                } else if let Some(r) =
                    registry.execute(&approval.tool_name, &approval.arguments).await
                {
                    r
                } else {
                    execute_via_mcp(&pool, &approval.tool_name, &approval.arguments).await
                };
                let duration_ms = started.elapsed().as_millis().min(u64::MAX as u128) as u64;
                let success = !result.to_lowercase().starts_with("error")
                    && !result.to_lowercase().starts_with("mcp error");
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
            }
        });
    }

    /// Deny all pending tool calls (return error result to AI).
    pub(crate) fn deny_pending_tools(&mut self) {
        let approvals: Vec<_> = self.state.chat.pending_approvals.drain(..).collect();
        let had_any = !approvals.is_empty();
        for approval in approvals {
            self.state.chat.messages.push(ChatMessage::tool_result(
                approval.tool_call_id,
                format!("Tool {} denegado por el usuario", approval.tool_name),
            ));
        }
        self.state.chat.approval_cursor = 0;
        self.state.chat.approval_started_at = None;
        if had_any {
            self.state.chat.snap_to_bottom();
        }
        self.maybe_continue_after_tools();
    }

    /// Aprueba el subset elegido: si hay items con `selected=true` aprueba
    /// esos; si ninguno está seleccionado, aprueba el item del cursor.
    /// Los no aprobados quedan en `pending_approvals` para siguiente input.
    pub(crate) fn approve_chosen_tools(&mut self) {
        let chosen_ids = self.chosen_pending_ids();
        if chosen_ids.is_empty() {
            return;
        }
        let drained: Vec<_> = self
            .state
            .chat
            .pending_approvals
            .extract_if(.., |a| chosen_ids.contains(&a.tool_call_id))
            .collect();
        self.state.chat.approval_cursor = 0;
        if self.state.chat.pending_approvals.is_empty() {
            self.state.chat.approval_started_at = None;
        }
        self.spawn_approved_subset(drained);
    }

    /// Deniega el subset elegido (selected o cursor).
    pub(crate) fn deny_chosen_tools(&mut self) {
        let chosen_ids = self.chosen_pending_ids();
        if chosen_ids.is_empty() {
            return;
        }
        let drained: Vec<_> = self
            .state
            .chat
            .pending_approvals
            .extract_if(.., |a| chosen_ids.contains(&a.tool_call_id))
            .collect();
        let had_any = !drained.is_empty();
        for approval in drained {
            self.state.chat.messages.push(ChatMessage::tool_result(
                approval.tool_call_id,
                format!("Tool {} denegado por el usuario", approval.tool_name),
            ));
        }
        self.state.chat.approval_cursor = 0;
        if self.state.chat.pending_approvals.is_empty() {
            self.state.chat.approval_started_at = None;
        }
        if had_any {
            self.state.chat.snap_to_bottom();
        }
        self.maybe_continue_after_tools();
    }

    /// Mueve el cursor del modal de aprobaciones con wrap-around.
    pub(crate) fn approval_cursor_move(&mut self, delta: i32) {
        let len = self.state.chat.pending_approvals.len();
        if len == 0 {
            return;
        }
        let current = self.state.chat.approval_cursor as i32;
        let next = (current + delta).rem_euclid(len as i32) as usize;
        self.state.chat.approval_cursor = next;
    }

    /// Toggle de selección del item bajo el cursor.
    pub(crate) fn approval_toggle_selection(&mut self) {
        let idx = self.state.chat.approval_cursor;
        if let Some(item) = self.state.chat.pending_approvals.get_mut(idx) {
            item.selected = !item.selected;
        }
    }

    /// IDs de los tool_calls elegidos: los seleccionados, o el del cursor si
    /// ninguno tiene `selected`. Vacío si no hay pending.
    fn chosen_pending_ids(&self) -> std::collections::HashSet<String> {
        let approvals = &self.state.chat.pending_approvals;
        if approvals.is_empty() {
            return std::collections::HashSet::new();
        }
        let selected: std::collections::HashSet<String> =
            approvals.iter().filter(|a| a.selected).map(|a| a.tool_call_id.clone()).collect();
        if !selected.is_empty() {
            return selected;
        }
        let idx = self.state.chat.approval_cursor.min(approvals.len().saturating_sub(1));
        approvals
            .get(idx)
            .map(|a| std::iter::once(a.tool_call_id.clone()).collect())
            .unwrap_or_default()
    }

    /// Ejecuta un subset aprobado (extraído de `pending_approvals`). Mismo
    /// flujo de `approve_pending_tools` pero sobre una lista dada.
    fn spawn_approved_subset(&mut self, approvals: Vec<crate::state::PendingToolApproval>) {
        if approvals.is_empty() {
            return;
        }
        let tx = self.tx.clone();
        let pool = self.mcp_pool.clone();
        let hooks = self.hooks.clone();
        let config_snapshot = current_config_snapshot(&self.state);
        tokio::spawn(async move {
            let registry = crate::services::tools::ToolRegistry::new();
            for approval in approvals {
                hooks.fire(
                    crate::services::hooks::HookTrigger::PreToolUse,
                    crate::services::hooks::HookContext::for_tool(
                        &approval.tool_name,
                        &approval.arguments,
                    ),
                    tx.clone(),
                );
                let started = std::time::Instant::now();
                let result = if approval.tool_name
                    == crate::services::tools::config_tool::CONFIG_TOOL_NAME
                {
                    crate::services::tools::config_tool::handle_request(
                        &approval.arguments,
                        &config_snapshot,
                        &tx,
                    )
                    .await
                } else if approval.tool_name == crate::services::tools::todowrite::TODO_WRITE_NAME {
                    crate::services::tools::todowrite::handle_request(&approval.arguments, &tx)
                        .await
                } else if let Some(r) =
                    registry.execute(&approval.tool_name, &approval.arguments).await
                {
                    r
                } else {
                    execute_via_mcp(&pool, &approval.tool_name, &approval.arguments).await
                };
                let duration_ms = started.elapsed().as_millis().min(u64::MAX as u128) as u64;
                let success = !result.to_lowercase().starts_with("error")
                    && !result.to_lowercase().starts_with("mcp error");
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
            }
        });
    }

    /// Always-allow a tool: approve pending calls for it and persist the rule.
    pub(crate) fn handle_always_allow_tool(&mut self, tool_name: String) {
        let mut rules = crate::services::permissions::PermissionRules::load();
        rules.add_allow(&tool_name);
        match rules.save() {
            Ok(()) => self.notify(format!("✓ {tool_name} → siempre permitir")),
            Err(e) => self.notify(format!("⚠ {tool_name} allow en memoria, no persistido: {e}")),
        }
        self.approve_pending_tools();
    }

    /// Always-deny a tool: deny pending calls for it and persist the rule.
    pub(crate) fn handle_always_deny_tool(&mut self, tool_name: String) {
        let mut rules = crate::services::permissions::PermissionRules::load();
        rules.add_deny(&tool_name);
        match rules.save() {
            Ok(()) => self.notify(format!("✗ {tool_name} → siempre denegar")),
            Err(e) => self.notify(format!("⚠ {tool_name} deny en memoria, no persistido: {e}")),
        }
        self.deny_pending_tools();
    }

    /// Approve the current plan: exit planning mode, plan stays as context.
    /// E12: si hay un `pending_plan` parseado, deriva la TodoList desde sus pasos.
    pub(crate) fn handle_plan_approve(&mut self) {
        self.state.chat.mode = crate::state::ChatMode::Normal;
        self.derive_todos_from_pending_plan();
        self.notify("✓ Plan aprobado — ejecuta los pasos manualmente".to_string());
        self.auto_save_history();
    }

    /// Edit the current plan: pre-fill input, AI will regenerate.
    pub(crate) fn handle_plan_edit(&mut self) {
        self.state.chat.mode = crate::state::ChatMode::Planning;
        self.state.chat.input = "Modifica el plan: ".to_string();
        self.notify("Edita tu instrucción y presiona Enter".to_string());
    }

    /// Reject the current plan: remove last assistant + compliance messages.
    pub(crate) fn handle_plan_reject(&mut self) {
        // Remove trailing compliance result and assistant plan message
        while let Some(last) = self.state.chat.messages.last() {
            if last.role == ChatRole::Assistant || last.role == ChatRole::System {
                self.state.chat.messages.pop();
            } else {
                break;
            }
        }
        self.state.chat.mode = crate::state::ChatMode::Planning;
        self.state.chat.scroll_offset = u16::MAX;
        self.notify("Plan descartado — escribe un nuevo prompt".to_string());
    }

    pub(crate) fn maybe_continue_after_tools(&mut self) {
        let pending_ids: Vec<String> = self
            .state
            .chat
            .messages
            .iter()
            .rev()
            .find(|m| m.role == ChatRole::Assistant && !m.tool_calls.is_empty())
            .map(|m| m.tool_calls.iter().map(|tc| tc.id.clone()).collect())
            .unwrap_or_default();

        if pending_ids.is_empty() {
            self.state.chat.status = ChatStatus::Ready;
            return;
        }

        let all_resolved = pending_ids.iter().all(|id| {
            self.state
                .chat
                .messages
                .iter()
                .any(|m| m.role == ChatRole::Tool && m.tool_call_id.as_deref() == Some(id))
        });

        if all_resolved {
            self.state.chat.status = ChatStatus::Streaming;
            self.state.chat.snap_to_bottom();
            self.spawn_chat_completion();
        }
    }
}

/// Snapshot de campos de AppState que el ConfigTool puede consultar (E20).
/// Se toma en el thread principal ANTES del `tokio::spawn` para no tener
/// que compartir AppState con la task async.
fn current_config_snapshot(
    state: &crate::state::AppState,
) -> crate::services::tools::config_tool::ConfigSnapshot {
    crate::services::tools::config_tool::ConfigSnapshot {
        model: state.model.clone(),
        factory: state.factory.api_key().unwrap_or("all").to_string(),
        permission_mode: state.chat.agent_mode.label().to_string(),
        theme: state.active_theme.label().to_string(),
    }
}

/// Ejecuta un tool call via el pool MCP persistente (fallback para tools
/// no built-in). Reusa una única conexión SSE al ingenierIA server primario
/// a través de `mcp_pool` en lugar de abrir una nueva conexión por call.
///
/// P2.5: reintenta automáticamente errores transitorios (timeout, connection
/// failed, 502/503/504) según `ToolRetryPolicy::default()` — hasta 2 intentos
/// con backoff exponencial de 500ms base. Errores terminales (no retryables)
/// se propagan inmediatamente.
pub(crate) async fn execute_via_mcp(
    pool: &std::sync::Arc<crate::services::mcp::McpPool>,
    name: &str,
    arguments: &str,
) -> String {
    use ingenieria_tools::retry::{RetryDecision, ToolRetryPolicy};
    let args: serde_json::Value =
        serde_json::from_str(arguments).unwrap_or(serde_json::Value::Object(Default::default()));
    let policy = ToolRetryPolicy::default();
    let mut attempts: u32 = 0;
    loop {
        let result = match pool.call_tool(name, args.clone()).await {
            Ok(out) => out,
            Err(e) => format!("MCP error: {e}"),
        };
        match policy.decide(&result, attempts) {
            RetryDecision::Ok | RetryDecision::GiveUp => return result,
            RetryDecision::Retry { delay, attempt } => {
                tracing::debug!(
                    tool = name,
                    attempt,
                    delay_ms = delay.as_millis() as u64,
                    "retrying transient MCP error"
                );
                tokio::time::sleep(delay).await;
                attempts = attempt;
            }
        }
    }
}
