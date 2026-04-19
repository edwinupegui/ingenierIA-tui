mod agents_handler;
#[cfg(feature = "autoskill")]
mod autoskill_handler;
mod bridge_handler;
mod chat;
mod chat_codeblocks;
mod chat_history;
mod chat_tools;
mod commands;
mod config_tool_handler;
mod cron_handler;
mod elicitation_handler;
mod handler_actions;
mod handler_events;
mod history_bridge;
mod history_search_handler;
mod hooks_handler;
pub(crate) mod input_dx_handler;
mod keys;
mod keys_chat;
mod keys_splash;
mod keys_wizard;
mod lsp_handler;
mod memory_commands;
mod monitor_handler;
mod plugin_handler;
mod quit_handler;
mod slash_commands;
mod spawners;
mod spawners_chat;
mod team_handler;
mod todos_handler;
mod transcript_handler;
mod wizard;

use std::sync::Arc;
use tokio::sync::mpsc::Sender;
use tokio::task::AbortHandle;

use crate::{
    actions::Action,
    config::Config,
    domain::{document::DocumentSummary, search::SearchResultItem},
    services::IngenieriaClient,
    state::{
        AppScreen, AppState, ChatStatus, InitStep, UrlValidation, WizardModelPhase, WizardStep,
    },
};

const MAX_EVENTS: usize = 100;

pub struct App {
    pub state: AppState,
    pub(crate) client: Arc<IngenieriaClient>,
    pub(crate) tx: Sender<Action>,
    pub(crate) search_abort: Option<AbortHandle>,
    /// AbortHandle del turn de chat en curso. `Some` mientras hay streaming o
    /// ejecucion de tools; `None` una vez que el turn cierra (Done/Failure).
    /// Permite a `Esc` durante Streaming abortar el request HTTP al provider.
    pub(crate) chat_abort: Option<AbortHandle>,
    pub(crate) wizard_from_config: bool,
    pub(crate) init_return_screen: AppScreen,
    pub(crate) hooks: crate::services::hooks::HookRunner,
    pub(crate) mcp_manager: crate::services::mcp::lifecycle::McpLifecycleManager,
    /// Pool persistente para el MCP server primario de ingenierIA (base_url del
    /// client). Reemplaza las conexiones ad-hoc de `execute_via_mcp` con un
    /// cliente cacheado. P1.2 del plan de chat agéntico.
    pub(crate) mcp_pool: std::sync::Arc<crate::services::mcp::McpPool>,
    /// E21: si true, resolve_provider usa MockChatProvider en lugar de
    /// Claude/Copilot. Activado via `--mock` CLI.
    pub(crate) mock_provider: bool,
    /// E27: channel para publicar snapshots de estado al bridge HTTP server.
    #[cfg(feature = "ide")]
    pub(crate) bridge_state_tx:
        Option<tokio::sync::watch::Sender<crate::services::bridge::BridgeSnapshot>>,
}

impl App {
    pub fn new(
        client: Arc<IngenieriaClient>,
        tx: Sender<Action>,
        config: Config,
        show_wizard: bool,
        wizard_from_config: bool,
        mock_provider: bool,
    ) -> Self {
        let mut state = AppState::new_with_provider(
            &config.developer,
            &config.model,
            &config.provider,
            config.default_factory.as_deref(),
            config.theme.as_deref(),
        );
        if show_wizard {
            state.screen = AppScreen::Wizard;
            state.wizard.server_url_cursor = config.server_url.len();
            state.wizard.server_url_input = config.server_url;
        }

        // Check for recent session to offer auto-resume
        if !show_wizard {
            if let Some(entry) = crate::services::history::list_history().first() {
                state.toasts.push(
                    format!("Sesion reciente: {}. Usa /resume", entry.title),
                    crate::state::ToastLevel::Info,
                    0,
                );
            }
        }

        let (hooks, hook_warnings) = crate::services::hooks::init_runner();
        for w in hook_warnings {
            state.toasts.push(format!("⚠ hooks.json: {w}"), crate::state::ToastLevel::Warning, 0);
        }
        if !hooks.is_empty() {
            state.toasts.push(
                format!("{} hook(s) cargados", hooks.len()),
                crate::state::ToastLevel::Info,
                0,
            );
        }

        // E17b: MCP lifecycle manager para servers extra definidos en mcp-servers.json.
        let mcp_manager = crate::services::mcp::lifecycle::McpLifecycleManager::new();
        let (mcp_configs, mcp_warnings) = crate::services::mcp::lifecycle::load_servers();
        for w in mcp_warnings {
            state.toasts.push(
                format!("⚠ mcp-servers.json: {w}"),
                crate::state::ToastLevel::Warning,
                0,
            );
        }
        if !mcp_configs.is_empty() {
            state.toasts.push(
                format!("{} MCP server(s) configurados", mcp_configs.len()),
                crate::state::ToastLevel::Info,
                0,
            );
            let mgr = mcp_manager.clone();
            tokio::spawn(async move {
                mgr.start_all(mcp_configs).await;
            });
        }

        if mock_provider {
            state.toasts.push(
                "⚙ Modo --mock activo: usando MockChatProvider".to_string(),
                crate::state::ToastLevel::Info,
                0,
            );
        }

        let mcp_pool = crate::services::mcp::McpPool::new(client.base_url());
        let mut app = Self {
            state,
            client,
            tx,
            search_abort: None,
            chat_abort: None,
            wizard_from_config,
            init_return_screen: AppScreen::Dashboard,
            hooks,
            mcp_manager,
            mcp_pool,
            mock_provider,
            #[cfg(feature = "ide")]
            bridge_state_tx: None,
        };
        // E28: auto-discover plugins from ~/.config/ingenieria-tui/plugins/
        if let Some(plugins_dir) = crate::services::plugins::default_plugins_dir() {
            let loaded =
                crate::services::plugins::load_from_dir(&mut app.state.plugins, &plugins_dir);
            if loaded > 0 {
                app.dispatch_plugin_init_effects();
            }
        }
        // E27: auto-start IDE bridge.
        app.try_start_bridge();
        app
    }

    /// Reducer principal. Retorna `true` si la app debe terminar.
    pub fn handle(&mut self, action: Action) -> bool {
        // Plugin pre-action: extrae tag y bloquea si algun plugin lo pide.
        let tag = if self.state.plugins.is_empty() {
            None
        } else {
            let t = action.tag();
            if let ingenieria_domain::plugin::PluginResponse::Block(reason) =
                self.state.plugins.on_pre_action(&t)
            {
                self.notify(format!("⊘ Plugin bloqueó {t}: {reason}"));
                return false;
            }
            Some(t)
        };

        match action {
            // ── Keyboard ──────────────────────────────────────────────
            Action::KeyEsc => return self.on_esc(),
            Action::KeyCtrlC => return self.on_ctrl_c(),
            Action::Quit => return true,
            Action::KeyShiftEnter => self.on_shift_enter(),
            Action::Paste(text) => self.on_paste(text),
            Action::KeyEnter => self.on_enter(),
            Action::KeyTab => self.on_tab(),
            Action::KeyBackTab => self.on_backtab(),
            Action::KeyChar(c) => self.on_char(c),
            Action::KeyBackspace => self.on_backspace(),
            Action::KeyUp => self.on_up(),
            Action::KeyDown => self.on_down(),
            Action::KeyLeft => self.on_left(),
            Action::KeyRight => self.on_right(),
            Action::KeyHome => self.on_home(),
            Action::KeyEnd => self.on_end(),
            Action::KeyDelete => self.on_delete(),

            // ── Scroll ────────────────────────────────────────────────
            Action::PreviewScrollUp => self.handle_scroll(-5),
            Action::PreviewScrollDown => self.handle_scroll(5),
            Action::ChatNavPrev => self.handle_chat_nav(-1),
            Action::ChatNavNext => self.handle_chat_nav(1),
            Action::MouseScrollUp(col) => self.handle_mouse_scroll(col, -3),
            Action::MouseScrollDown(col) => self.handle_mouse_scroll(col, 3),

            // ── Input edit ────────────────────────────────────────────
            Action::InputUndo => self.handle_input_undo(),
            Action::InputRedo => self.handle_input_redo(),

            // ── Focus ─────────────────────────────────────────────────
            Action::FocusGained => self.handle_focus_gained(),
            Action::FocusLost => {}

            // ── Tick ──────────────────────────────────────────────────
            Action::Tick => self.handle_tick(),

            // ── Server / Health ───────────────────────────────────────
            Action::HealthUpdated(h) => self.handle_health_updated(h),
            Action::HealthFetchFailed(msg) => self.handle_health_fetch_failed(msg),
            Action::ServerEvent(event) => self.handle_server_event(event),
            Action::SseDisconnected => tracing::debug!("SSE disconnected"),

            // ── Documents ─────────────────────────────────────────────
            Action::DocumentsLoaded(docs) => self.handle_documents_loaded(docs),
            Action::DocumentsLoadFailed(msg) => self.handle_documents_load_failed(msg),
            Action::DocumentLoaded(doc) => self.handle_document_loaded(doc),
            Action::DocumentLoadFailed(msg) => {
                self.state.dashboard.preview.loading = false;
                self.notify(format!("✗ Error: {msg}"));
            }
            Action::ChatDocLoaded(doc) => self.handle_chat_doc_loaded(doc),
            Action::ChatDocLoadFailed(msg) => {
                self.notify(format!("✗ Error cargando documento: {msg}"));
            }

            // ── Sync ──────────────────────────────────────────────────
            Action::SyncResult { updated_uris, server_last_update } => {
                self.handle_sync_result(updated_uris, server_last_update);
            }
            Action::SyncFailed(msg) => {
                self.state.dashboard.sync.loading = false;
                tracing::debug!(error = %msg, "Sync check failed");
            }

            // ── Workflow / Compliance ─────────────────────────────────
            Action::WorkflowLoaded { workflow_name, content } => {
                self.handle_workflow_loaded(workflow_name, content);
            }
            Action::WorkflowFailed(msg) => self.notify(format!("✗ Workflow: {msg}")),
            Action::ComplianceResult(report) => self.handle_compliance_result(report),
            Action::ComplianceFailed(msg) => self.notify(format!("✗ Compliance: {msg}")),

            // ── Wizard / Init ─────────────────────────────────────────
            Action::WizardUrlValid => {
                self.state.wizard.url_validation = UrlValidation::Valid;
                self.state.wizard.step = WizardStep::Name;
            }
            Action::WizardUrlInvalid(msg) => {
                self.state.wizard.url_validation = UrlValidation::Invalid(msg);
            }
            Action::InitDetected(pt, dir) => self.handle_init_detected(pt, dir),
            Action::InitComplete(results) => {
                self.state.init.results = results;
                self.state.init.error = None;
                self.state.init.step = InitStep::Done;
            }
            Action::InitFailed(msg) => {
                self.state.init.error = Some(msg);
                self.state.init.step = InitStep::Done;
            }

            // ── Copilot OAuth ─────────────────────────────────────────
            Action::CopilotDeviceCode { user_code, verification_uri, device_code } => {
                self.handle_copilot_device_code(user_code, verification_uri, device_code.0);
            }
            Action::CopilotDeviceCodeFailed(msg) => {
                self.state.wizard.copilot.error = Some(msg);
                self.state.wizard.model_phase = WizardModelPhase::SelectProvider;
            }
            Action::CopilotAuthSuccess(token) => self.handle_copilot_auth_success(token.0),
            Action::CopilotAuthFailed(msg) => {
                self.state.wizard.copilot.error = Some(msg);
                self.state.wizard.copilot.auth_done = false;
            }
            Action::CopilotModelsLoaded(models) => self.handle_copilot_models_loaded(models),
            Action::CopilotModelsFailed(msg) => self.handle_copilot_models_failed(msg),

            // ── Chat flow ─────────────────────────────────────────────
            Action::ChatContextLoaded(system_msgs) => self.handle_chat_context_loaded(system_msgs),
            Action::ChatContextFailed(msg) => {
                self.state.chat.status = ChatStatus::Error(format!("Contexto: {msg}"));
            }
            Action::ChatTokenUsage {
                input_tokens,
                output_tokens,
                cache_creation_input_tokens,
                cache_read_input_tokens,
            } => {
                self.handle_chat_token_usage(
                    input_tokens,
                    output_tokens,
                    cache_creation_input_tokens,
                    cache_read_input_tokens,
                );
            }
            Action::ChatStreamDelta(token) => self.handle_chat_stream_delta(token),
            Action::ChatThinkingDelta(text) => self.handle_chat_thinking_delta(text),
            Action::ChatToggleSidebar => {
                self.state.chat.sidebar_visible = !self.state.chat.sidebar_visible;
            }
            Action::ChatStreamDone => {
                self.state.chat.stream_elapsed_secs = 0;
                self.state.chat.stream_stalled = false;
                self.chat_abort = None;
                self.handle_chat_stream_done();
            }
            Action::ChatStreamTruncated => {
                self.notify_level(
                    "⚠ Respuesta truncada (max_tokens). Escribe \"continúa\" para seguir.",
                    crate::state::ToastLevel::Warning,
                );
            }
            Action::ChatToolCall { id, name, arguments } => {
                self.handle_chat_tool_call(id, name, arguments);
            }
            Action::ChatToolResult { tool_call_id, content } => {
                self.handle_chat_tool_result(tool_call_id, content);
            }
            Action::ChatStreamFailure(failure) => {
                self.chat_abort = None;
                self.handle_chat_stream_failure(failure);
            }
            Action::ChatStreamAbort => {
                self.handle_chat_stream_abort();
            }
            Action::StreamHeartbeat(secs) => {
                self.state.chat.stream_elapsed_secs = secs;
            }
            Action::StreamWarning(secs) => {
                self.state.chat.stream_elapsed_secs = secs;
                if !self.state.chat.stream_stalled {
                    self.state.chat.stream_stalled = true;
                    self.notify("⚠ Respuesta lenta...".to_string());
                }
            }
            Action::StreamTimeout => {
                self.state.chat.stream_stalled = false;
                self.state.chat.stream_elapsed_secs = 0;
                self.state.chat.status = ChatStatus::Error("Tiempo agotado: sin respuesta por 60s".into());
            }
            Action::ChatRetryScheduled { attempt, max_attempts, delay_secs, reason } => {
                self.handle_chat_retry_scheduled(attempt, max_attempts, delay_secs, reason);
            }
            Action::ChatFallbackSuggested { previous_model, suggested_model } => {
                self.handle_chat_fallback_suggested(previous_model, suggested_model);
            }
            Action::ChatPostToolStall { nudge_number } => {
                self.handle_chat_post_tool_stall(nudge_number);
            }

            // ── Planning / Permissions ────────────────────────────────
            Action::PlanApprove => self.handle_plan_approve(),
            Action::PlanEdit => self.handle_plan_edit(),
            Action::PlanReject => self.handle_plan_reject(),
            Action::AlwaysAllowTool(name) => self.handle_always_allow_tool(name),
            Action::AlwaysDenyTool(name) => self.handle_always_deny_tool(name),

            // ── Doctor ────────────────────────────────────────────────
            Action::DoctorReportReady(report) => self.handle_doctor_report(report),

            // ── Misc ──────────────────────────────────────────────────
            Action::CodeBlockApplied { msg, path, content } => {
                self.notify(msg);
                if let (Some(p), Some(c)) = (path, content) {
                    self.state.lsp.notify_file_changed(&p, &c);
                }
            }
            Action::HistoryList(entries) => self.handle_history_list(entries),
            Action::HistoryLoaded(conv) => self.handle_history_loaded(conv),
            Action::SearchResultsReceived(results) => {
                let factory_key = self.state.factory.filter_key().unwrap_or("");
                let cache_key = format!("{}\0{factory_key}", self.state.search.query);
                self.state.caches.search.insert(cache_key, results.clone());
                self.state.search.results = results;
                self.state.search.cursor = 0;
                self.state.search.loading = false;
                self.state.search.error = None;
            }
            Action::SearchFailed(msg) => {
                self.state.search.loading = false;
                self.state.search.error = Some(msg);
            }
            Action::ToolEventReceived(event) => {
                self.state.tool_events.insert(0, event);
                self.state.tool_events.truncate(MAX_EVENTS);
            }
            Action::HookEventReceived(event) => {
                self.state.hook_events.insert(0, event);
                self.state.hook_events.truncate(MAX_EVENTS);
            }
            Action::HookExecuted(outcome) => self.handle_hook_executed(outcome),
            Action::ElicitationRequested { request, responder } => {
                self.handle_elicitation_requested(request, responder);
            }
            #[cfg(feature = "mcp")]
            Action::McpToolsDiscovered(tools) => {
                for tool in &tools {
                    self.state
                        .caches
                        .tool_schemas
                        .insert(tool.name.clone(), tool.input_schema.clone());
                }
                self.state.mcp_tools = tools;
            }
            #[cfg(feature = "autoskill")]
            Action::AutoSkillScanDone(scan) => self.handle_autoskill_scan_done(scan),
            #[cfg(feature = "autoskill")]
            Action::SkillInstallDone(summary) => self.handle_skill_install_done(summary),
            Action::ProjectTypeDetected(detected) => self.handle_project_type_detected(detected),

            // ── SubAgents (E22a) ──────────────────────────────────────
            Action::AgentResult { id, status, result } => {
                self.handle_agent_result(id, status, result);
            }

            // ── Cron (E23) ────────────────────────────────────────────
            Action::CronJobFired { id } => self.handle_cron_fired(id),

            // ── Process Monitor (E26) ────────────────────────────────
            Action::MonitorOutput { id, line, is_stderr } => {
                self.handle_monitor_output(id, line, is_stderr);
            }
            Action::MonitorFinished { id, exit_code, error, killed } => {
                self.handle_monitor_finished(id, exit_code, error, killed);
            }

            // ── LSP Integration (E25) ────────────────────────────────
            Action::LspDiagnosticsReceived { uri, diagnostics } => {
                self.handle_lsp_diagnostics_received(uri, diagnostics);
            }
            Action::LspServerStarted { name } => {
                self.handle_lsp_server_started(name);
            }
            Action::LspServerFailed { name, error } => {
                self.handle_lsp_server_failed(name, error);
            }

            // ── IDE Bridge (E27) ─────────────────────────────────────
            #[cfg(feature = "ide")]
            Action::BridgeContextUpdate { kind, path, content } => {
                self.handle_bridge_context_update(kind, path, content);
            }
            #[cfg(feature = "ide")]
            Action::BridgeToolApproval { tool_call_id, approved } => {
                self.handle_bridge_tool_approval(tool_call_id, approved);
            }

            // ── ConfigTool (E20) ──────────────────────────────────────
            Action::ApplyConfigChange { field, value } => {
                self.handle_apply_config_change(field, value);
            }
            Action::ApplyTodoWrite { items } => {
                self.handle_apply_todo_write(items);
            }
            Action::ChatUserMessageResolved { augmented_text, refs } => {
                self.handle_chat_user_message_resolved(augmented_text, refs);
            }

            // ── File watcher + Recovery (E42) ─────────────────────────
            Action::ConfigChanged => self.handle_config_changed(),
            Action::KeybindingsChanged => self.handle_keybindings_changed(),
            Action::ClaudeMdChanged => self.handle_claude_md_changed(),
            Action::EnvChanged => self.handle_env_changed(),
            Action::RecoveryRecipeDispatched { scenario_label, step_label } => {
                tracing::info!(
                    scenario = %scenario_label,
                    step = %step_label,
                    "recovery step dispatched"
                );
            }
        }

        // Plugin post-action.
        if let Some(ref t) = tag {
            self.state.plugins.on_post_action(t);
        }

        false
    }

    // ── E42 handlers ────────────────────────────────────────────────────────

    fn handle_config_changed(&mut self) {
        // Reload config desde disco, best-effort. `Config::resolve` nunca
        // falla (cae a defaults), asi que no hay rama Err.
        let cfg = crate::config::Config::resolve(None);
        self.state.developer = cfg.developer.clone();
        self.state.model = cfg.model.clone();
        self.notify("⟳ Config recargada desde disco".to_string());
        tracing::info!("config.json hot-reloaded");
    }

    fn handle_keybindings_changed(&mut self) {
        self.state.keybindings = crate::config::load_keybindings();
        self.notify("⟳ Keybindings recargados".to_string());
        tracing::info!("keybindings.json hot-reloaded");
    }

    fn handle_claude_md_changed(&mut self) {
        self.notify("⟳ CLAUDE.md actualizado — se aplicara al proximo chat".to_string());
        tracing::info!("CLAUDE.md changed");
    }

    fn handle_env_changed(&mut self) {
        self.notify_level(
            "⚠ .env cambio — reinicia para que surta efecto",
            crate::state::ToastLevel::Warning,
        );
    }

    /// Dispara la recipe correspondiente a un StructuredFailure (E13+E42).
    /// Llamado desde handlers cuando el fallo puede beneficiarse de recovery.
    pub(crate) fn dispatch_recovery_from_failure(
        &mut self,
        failure: &crate::domain::failure::StructuredFailure,
    ) {
        let Some(plan) = crate::services::recovery_engine::plan_for_failure(failure) else {
            return;
        };
        let level = plan.toast_level();
        let text = crate::services::recovery_engine::headline_message(&plan);
        self.notify_level(&text, level);
        tracing::info!(
            scenario = ?plan.scenario,
            steps = plan.steps.len(),
            "recovery plan dispatched"
        );
    }

    fn handle_scroll(&mut self, delta: i16) {
        if self.state.screen == AppScreen::Chat {
            if delta < 0 {
                self.state.chat.scroll_up(delta.unsigned_abs());
                // Set scroll anchor for grace period during streaming
                if self.state.chat.status == ChatStatus::Streaming {
                    self.state.chat.scroll_anchor_tick = Some(self.state.tick_count);
                }
            } else {
                self.state.chat.scroll_down(delta as u16);
                // Clear anchor when scrolling back down to bottom
                if self.state.chat.scroll_offset == u16::MAX {
                    self.state.chat.scroll_anchor_tick = None;
                }
            }
        } else if delta < 0 {
            self.state.dashboard.preview.scroll =
                self.state.dashboard.preview.scroll.saturating_sub(delta.unsigned_abs());
        } else {
            self.state.dashboard.preview.scroll =
                self.state.dashboard.preview.scroll.saturating_add(delta as u16);
        }
    }

    /// Salto entre user messages via Ctrl+↑/↓ (delta ±1). No-op si no hay
    /// turnos user o si no estamos en Chat. Durante Streaming tambien setea
    /// scroll_anchor_tick para evitar que el auto-bottom lo anule.
    fn handle_chat_nav(&mut self, delta: i32) {
        if self.state.screen != AppScreen::Chat {
            return;
        }
        self.state.chat.nav_move(delta);
        if self.state.chat.status == ChatStatus::Streaming {
            self.state.chat.scroll_anchor_tick = Some(self.state.tick_count);
        }
    }

    fn handle_mouse_scroll(&mut self, col: u16, delta: i16) {
        if self.state.screen == AppScreen::Chat {
            self.handle_scroll(delta);
        } else if self.state.screen == AppScreen::Dashboard {
            if col < 26 {
                if delta < 0 {
                    self.state.dashboard.sidebar.move_up();
                } else {
                    self.state.dashboard.sidebar.move_down();
                }
            } else {
                self.handle_scroll(delta);
            }
        }
    }

    fn handle_tick(&mut self) {
        self.state.tick_count = self.state.tick_count.wrapping_add(1);
        self.state.toasts.tick(self.state.tick_count);
        // E39: auto-dismiss del checklist de onboarding ~3s despues de completarse.
        self.state.onboarding.checklist.start_dismiss_countdown(self.state.tick_count);
        if self.state.onboarding.checklist.check_auto_dismiss(self.state.tick_count) {
            if let Err(err) = self.state.onboarding.save() {
                tracing::warn!(%err, "onboarding save after auto-dismiss failed");
            }
        }
        // Evict expired cache entries every ~15s (60 ticks at 4Hz)
        if self.state.tick_count.is_multiple_of(60) {
            self.state.caches.evict_all_expired();
        }
        // E40: auto-save del draft cada DRAFT_AUTOSAVE_TICKS (~2s).
        if self.state.tick_count.is_multiple_of(crate::app::input_dx_handler::DRAFT_AUTOSAVE_TICKS)
        {
            self.handle_draft_auto_save();
        }
        // E27: publish bridge snapshot cada ~4s (16 ticks at 4Hz).
        if self.state.tick_count.is_multiple_of(16) {
            self.publish_bridge_snapshot();
        }
        // P1.3: timeout del modal de aprobación de tools. Check cada ~1s (4 ticks).
        if self.state.tick_count.is_multiple_of(4) {
            self.check_approval_timeout();
        }
    }

    /// Si hay approvals pendientes y excedimos el timeout, auto-aprueba o
    /// auto-deniega según `PermissionMode`. Strict congela (no auto-action).
    fn check_approval_timeout(&mut self) {
        let Some(started) = self.state.chat.approval_started_at else {
            return;
        };
        if self.state.chat.pending_approvals.is_empty() {
            self.state.chat.approval_started_at = None;
            return;
        }
        let elapsed = started.elapsed().as_millis() as u64;
        if elapsed < self.state.chat.approval_timeout_ms {
            return;
        }
        // En modo Ask: auto-deny al expirar el timeout.
        // En modo Auto: las tools nunca llegan a pending_approvals.
        // En modo Plan: no hay ejecución de tools.
        self.notify(format!(
            "⏱ Tiempo de aprobación agotado ({:.0}s) → auto-denegado",
            elapsed as f64 / 1000.0
        ));
        self.deny_pending_tools();
    }
}

/// Filtro local fuzzy para queries < 3 caracteres (usa nucleo para scoring)
fn local_search(all_docs: &[DocumentSummary], query: &str) -> Vec<SearchResultItem> {
    use nucleo_matcher::{
        pattern::{CaseMatching, Normalization, Pattern},
        Matcher,
    };

    let mut matcher = Matcher::new(nucleo_matcher::Config::DEFAULT);
    let pattern = Pattern::parse(query, CaseMatching::Ignore, Normalization::Smart);
    let mut buf = Vec::new();

    let mut scored: Vec<(u32, &DocumentSummary)> = all_docs
        .iter()
        .filter_map(|d| {
            let haystack = format!("{} {}", d.name, d.description);
            let atoms_haystack = nucleo_matcher::Utf32Str::new(&haystack, &mut buf);
            pattern.score(atoms_haystack, &mut matcher).map(|score| (score, d))
        })
        .collect();

    scored.sort_by(|a, b| b.0.cmp(&a.0));

    scored
        .into_iter()
        .take(20)
        .map(|(_, d)| SearchResultItem {
            uri: d.uri.clone(),
            name: d.name.clone(),
            doc_type: d.doc_type.clone(),
            factory: d.factory.clone(),
            description: d.description.clone(),
            matches: Vec::new(),
        })
        .collect()
}

#[cfg(test)]
mod tests;
