//! Action handlers extracted from the main reducer to keep file sizes manageable.
//! These handle inline actions that were previously in App::handle() match arms.

use crate::{
    domain::{doctor::DoctorReport, document::DocumentDetail, health::HealthStatus},
    state::{
        AppMode, AppScreen, ChatMessage, ChatRole, ChatState, ChatStatus, ServerStatus, ToolCall,
        ToolCallStatus, WizardModelPhase,
    },
};

use super::App;

impl App {
    /// Refresh data when terminal regains focus (e.g. user switches back).
    pub(crate) fn handle_focus_gained(&mut self) {
        // Refresh document list if on dashboard and cache expired
        if self.state.screen == AppScreen::Dashboard
            && !self.state.dashboard.sidebar.loading
            && self.state.caches.documents.get(&"all".to_string()).is_none()
        {
            self.spawn_load_documents();
            self.state.dashboard.sidebar.loading = true;
        }
    }

    pub(crate) fn handle_exit_to_splash(&mut self) {
        if self.state.screen == AppScreen::Chat {
            self.state.chat = ChatState::new();
            self.state.screen = AppScreen::Splash;
            self.state.input.clear();
        }
    }

    pub(crate) fn handle_health_updated(&mut self, h: HealthStatus) {
        let was_offline = matches!(self.state.server_status, ServerStatus::Offline(_));
        self.state.server_status = ServerStatus::Online(h);
        if was_offline {
            #[cfg(feature = "mcp")]
            self.spawn_discover_mcp_tools();
            self.notify("✓ MCP reconectado".to_string());
        }
    }

    pub(crate) fn handle_health_fetch_failed(&mut self, msg: String) {
        let was_online = matches!(self.state.server_status, ServerStatus::Online(_));
        self.state.server_status = ServerStatus::Offline(msg);
        if was_online {
            self.notify("✗ MCP desconectado — modo degradado".to_string());
        }
    }

    pub(crate) fn handle_document_loaded(&mut self, doc: DocumentDetail) {
        self.state.dashboard.preview.loading = false;
        self.state.caches.doc_details.insert(doc.uri.clone(), doc.clone());
        let cached = crate::ui::widgets::markdown::render_markdown(
            &doc.content,
            &self.state.active_theme.colors(),
        );
        self.state.dashboard.preview.set_doc(doc);
        self.state.dashboard.preview.cached_lines = Some(std::sync::Arc::new(cached));
    }

    pub(crate) fn handle_chat_doc_loaded(&mut self, doc: DocumentDetail) {
        let user_arg = self.state.chat.pending_workflow_arg.take();
        if let Some(arg) = user_arg {
            let prompt = format!(
                "# Skill: {}\n\nUsa el siguiente skill como guia.\n\n{}",
                doc.name, doc.content
            );
            self.state.chat.messages.push(ChatMessage::new(ChatRole::System, prompt));
            self.state.chat.messages.push(ChatMessage::new(ChatRole::User, arg));
            self.state.chat.status = ChatStatus::Streaming;
            self.state.chat.scroll_offset = u16::MAX;
            self.state.chat.tool_rounds = 0;
            self.spawn_chat_completion();
            self.notify(format!("✓ Skill {} — ejecutando...", doc.name));
        } else {
            let header = format!("**{}** — `{}`\n\n", doc.name, doc.uri);
            let content = format!("{header}{}", doc.content);
            self.state.chat.messages.push(ChatMessage::new(ChatRole::User, content));
            self.state.chat.scroll_offset = u16::MAX;
            self.notify(format!("✓ {} inyectado al chat", doc.name));
        }
    }

    pub(crate) fn handle_workflow_loaded(&mut self, workflow_name: String, content: String) {
        let user_arg = self.state.chat.pending_workflow_arg.take();
        let prompt = format!(
            "# Workflow: {workflow_name}\n\n\
             Ejecuta el siguiente workflow paso a paso.\n\n\
             {content}"
        );
        let sys_msg = ChatMessage::new(ChatRole::System, prompt);
        self.state.chat = ChatState::new();
        self.state.chat.messages.push(sys_msg);
        self.state.screen = AppScreen::Chat;

        if let Some(arg) = user_arg {
            self.state.chat.messages.push(ChatMessage::new(ChatRole::User, arg));
            self.state.chat.status = ChatStatus::Streaming;
            self.state.chat.scroll_offset = u16::MAX;
            self.spawn_chat_completion();
            self.notify(format!("✓ Workflow {workflow_name} — ejecutando..."));
        } else {
            self.state.chat.status = ChatStatus::Ready;
            self.notify(format!("✓ Workflow {workflow_name} listo"));
        }
    }

    pub(crate) fn handle_compliance_result(&mut self, report: String) {
        let content = format!("# Reporte de Compliance\n\n{report}");
        let cached = crate::ui::widgets::markdown::render_markdown(
            &content,
            &self.state.active_theme.colors(),
        );
        let mut msg = ChatMessage::new(ChatRole::Assistant, content);
        msg.cached_lines = Some(std::sync::Arc::new(cached));
        self.state.chat.messages.push(msg);
        self.state.chat.scroll_offset = u16::MAX;
        self.notify("✓ Compliance check completado".to_string());
    }

    pub(crate) fn handle_copilot_models_failed(&mut self, msg: String) {
        if self.state.mode == AppMode::ModelPicker {
            self.state.model_picker.error = Some(msg);
            self.state.model_picker.loading = false;
        } else {
            self.state.wizard.copilot.error = Some(msg);
            self.state.wizard.model_phase = WizardModelPhase::SelectModel;
        }
    }

    pub(crate) fn handle_chat_token_usage(
        &mut self,
        input_tokens: u32,
        output_tokens: u32,
        cache_creation: u32,
        cache_read: u32,
    ) {
        let model = self.state.model.clone();
        self.state.chat.cost.add_usage(
            input_tokens,
            output_tokens,
            cache_creation,
            cache_read,
            &model,
        );
        if let Some(warning) = self.state.chat.cost.budget_warning() {
            self.notify(format!("⚠ {warning} ({})", self.state.chat.cost.cost_display()));
        }
    }

    pub(crate) fn handle_chat_thinking_delta(&mut self, text: String) {
        if let Some(last) = self.state.chat.messages.last_mut() {
            if last.role == ChatRole::Assistant {
                last.thinking.get_or_insert_with(String::new).push_str(&text);
            } else {
                let mut msg = ChatMessage::new(ChatRole::Assistant, String::new());
                msg.thinking = Some(text);
                self.state.chat.messages.push(msg);
            }
        }
        self.state.chat.stream_stalled = false;
    }

    pub(crate) fn handle_chat_stream_delta(&mut self, token: String) {
        // Clear stall indicator on any content delta
        self.state.chat.stream_stalled = false;
        // E34: captura TTFT en primer delta + acumula chars para OTPS.
        self.state.chat.metrics.on_delta(token.len());

        if let Some(last) = self.state.chat.messages.last_mut() {
            if last.role == ChatRole::Assistant {
                last.content.push_str(&token);
                last.invalidate_cache();
            } else {
                self.state.chat.messages.push(ChatMessage::new(ChatRole::Assistant, token));
            }
        }

        // Scroll preservation: respect user's manual scroll position for 3s (12 ticks)
        let anchored = self
            .state
            .chat
            .scroll_anchor_tick
            .is_some_and(|t| self.state.tick_count.wrapping_sub(t) < 12);
        if !anchored {
            self.state.chat.scroll_offset = u16::MAX;
            // Anchor expirado: limpiar para que siguientes deltas/mensajes
            // no reevaluen ventana de gracia.
            self.state.chat.scroll_anchor_tick = None;
        }
    }

    pub(crate) fn handle_doctor_report(&mut self, report: DoctorReport) {
        let overall = report.overall();
        let icon = overall.glyph();
        let label = overall.label();
        self.state.doctor_report = Some(report);
        self.state.panels.show_doctor = true;
        self.notify(format!("{icon} Doctor: {label}"));
    }

    /// Marca un paso del checklist de onboarding como completado y persiste si
    /// hubo cambio. Best-effort — una falla de IO solo se loguea. Tambien
    /// registra una vista para que `full_views` avance si ya esta todo hecho.
    pub(crate) fn mark_onboarding_step(
        &mut self,
        step: crate::services::onboarding::ChecklistStep,
    ) {
        if !self.state.onboarding.checklist.mark(step) {
            return;
        }
        self.state.onboarding.checklist.record_view();
        self.state.onboarding.checklist.start_dismiss_countdown(self.state.tick_count);
        if let Err(err) = self.state.onboarding.save() {
            tracing::warn!(%err, "onboarding save after mark failed");
        }
        self.notify(format!("✓ Onboarding: {}", step.label()));
    }

    pub(crate) fn handle_chat_tool_call(&mut self, id: String, name: String, arguments: String) {
        // Audit log (E13): fire-and-forget con redactor automatico.
        let session_id = self.state.chat.session_id.clone();
        let audit_name = name.clone();
        let audit_args = arguments.clone();
        tokio::spawn(async move {
            crate::services::audit::log_entry(crate::services::audit::AuditEntry::new(
                session_id,
                crate::services::audit::AuditKind::ToolCall {
                    tool: audit_name,
                    arguments: audit_args,
                    approved: None,
                },
            ));
        });

        // E34: registrar tool start para medir duracion.
        self.state.chat.metrics.on_tool_call(id.clone());

        // If no assistant message exists yet (model responded with only tool_use,
        // no text delta), create one so the tool call has a home.
        if self.state.chat.messages.last().map(|m| m.role != ChatRole::Assistant).unwrap_or(true) {
            self.state.chat.messages.push(ChatMessage::new(ChatRole::Assistant, String::new()));
        }

        if let Some(last) = self.state.chat.messages.last_mut() {
            if last.role == ChatRole::Assistant {
                last.tool_calls.push(ToolCall {
                    id,
                    name,
                    arguments,
                    status: ToolCallStatus::Pending,
                    duration_ms: None,
                });
                self.state.chat.cost.add_tool_call();
            }
        }
    }

    pub(crate) fn handle_chat_retry_scheduled(
        &mut self,
        attempt: u32,
        max_attempts: u32,
        delay_secs: u16,
        reason: String,
    ) {
        self.notify_level(
            &format!("⟳ Retry {attempt}/{max_attempts} en {delay_secs}s ({reason})"),
            crate::state::ToastLevel::Warning,
        );
    }

    pub(crate) fn handle_chat_fallback_suggested(
        &mut self,
        previous_model: String,
        suggested_model: String,
    ) {
        self.notify_level(
            &format!("⚠ {previous_model} acumula fallos. Sugerencia: {suggested_model}"),
            crate::state::ToastLevel::Warning,
        );
    }

    pub(crate) fn handle_chat_post_tool_stall(&mut self, nudge_number: u32) {
        self.notify_level(
            &format!("⚠ Sin actividad post-herramienta: estímulo #{nudge_number}"),
            crate::state::ToastLevel::Warning,
        );
    }

    /// Usuario aborto el turn actual con Esc.
    /// Abortea el task del provider (HTTP request pendiente se cancela), limpia
    /// estado parcial y regresa a Ready. Si el ultimo mensaje es un Assistant
    /// con contenido vacio (aun no llego ningun delta), se remueve para que el
    /// timeline no quede con mensajes fantasma.
    pub(crate) fn handle_chat_stream_abort(&mut self) {
        if let Some(h) = self.chat_abort.take() {
            h.abort();
        }

        // Marcar tool calls Pending como Error para que el UI no quede con
        // spinners colgados. Patrón de opencode: cleanup on interrupt.
        if let Some(last) = self.state.chat.messages.last_mut() {
            if last.role == ChatRole::Assistant {
                for tc in last.tool_calls.iter_mut() {
                    if tc.status == ToolCallStatus::Pending {
                        tc.status = ToolCallStatus::Error;
                    }
                }
            }
        }
        // Limpiar aprobaciones pendientes: ya no hay turn activo que las espere.
        self.state.chat.pending_approvals.clear();
        self.state.chat.approval_started_at = None;

        if let Some(last) = self.state.chat.messages.last() {
            if last.role == ChatRole::Assistant
                && last.content.trim().is_empty()
                && last.tool_calls.is_empty()
            {
                self.state.chat.messages.pop();
            }
        }
        self.state.chat.status = ChatStatus::Ready;
        self.state.chat.stream_stalled = false;
        self.state.chat.stream_elapsed_secs = 0;
        self.state.chat.tool_rounds = 0;
        self.state.chat.snap_to_bottom();
        self.notify("⏹ Turn abortado".to_string());
    }

    /// E13 — handler estructurado para fallos de stream.
    /// Mapea severity a ToastLevel, muestra hint de recovery, registra en
    /// audit log y actualiza `ChatStatus::Error` con el mensaje humano.
    pub(crate) fn handle_chat_stream_failure(
        &mut self,
        failure: crate::domain::failure::StructuredFailure,
    ) {
        self.state.chat.status = crate::state::ChatStatus::Error(failure.display());
        self.state.chat.stream_stalled = false;
        self.state.chat.stream_elapsed_secs = 0;

        let level = crate::state::ToastLevel::from(failure.severity);
        let mut display = format!("✗ [{}] {}", failure.category.label(), failure.message);
        if let Some(hint) = &failure.recovery_hint {
            display.push_str(" · ");
            display.push_str(hint);
        }
        self.notify_level(&display, level);

        // Inline en timeline: el toast desaparece, el mensaje persiste y el
        // user ve claramente por que fallo el turno sin tener que mirar el
        // status bar. Enter sobre ChatStatus::Error reintenta el turno.
        let inline = match &failure.recovery_hint {
            Some(hint) => {
                format!("**✗ {}** — {}\n\n*{}*", failure.category.label(), failure.message, hint)
            }
            None => format!("**✗ {}** — {}", failure.category.label(), failure.message),
        };
        let cached = crate::ui::widgets::markdown::render_markdown(
            &inline,
            &self.state.active_theme.colors(),
        );
        let mut msg = ChatMessage::new(ChatRole::Assistant, inline);
        msg.cached_lines = Some(std::sync::Arc::new(cached));
        self.state.chat.messages.push(msg);
        self.state.chat.snap_to_bottom();

        // E42: si el fallo mapea a un scenario conocido, disparar recovery recipe.
        // El toast adicional complementa (no reemplaza) el mensaje del fallo.
        self.dispatch_recovery_from_failure(&failure);

        // Fire-and-forget audit log.
        let session_id = self.state.chat.session_id.clone();
        let failure_clone = failure.clone();
        tokio::spawn(async move {
            crate::services::audit::log_entry(crate::services::audit::AuditEntry::new(
                session_id,
                crate::services::audit::AuditKind::Failure { failure: failure_clone },
            ));
        });
    }
}
