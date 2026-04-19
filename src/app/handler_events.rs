use crate::{
    domain::{document::DocumentSummary, event::IngenieriaEvent},
    services::init as init_service,
    state::{
        system_time_str, ActiveSession, ChatMessage, ChatMode, ChatRole, ChatStatus, InitStep,
        TimedEvent, WizardModelPhase, MAX_TOOL_ROUNDS,
    },
};

use super::App;

impl App {
    pub(crate) fn handle_server_event(&mut self, event: IngenieriaEvent) {
        tracing::debug!(kind = event.kind_str(), "SSE event");
        match &event {
            IngenieriaEvent::Sync { docs_changed, .. } => {
                self.state.caches.invalidate_documents();
                if !self.state.dashboard.sidebar.loading {
                    self.spawn_load_documents();
                    self.state.dashboard.sidebar.loading = true;
                }
                if !self.state.dashboard.sync.loading {
                    self.state.dashboard.sync.loading = true;
                    self.spawn_sync_check();
                }
                self.notify(format!("↻ {docs_changed} docs actualizados"));
            }
            IngenieriaEvent::Reload { file, .. } => {
                self.state.caches.invalidate_documents();
                if let Some(doc) = &self.state.dashboard.preview.doc {
                    if file.contains(&doc.name) {
                        self.spawn_fetch_document(
                            doc.doc_type.clone(),
                            doc.factory.clone(),
                            doc.name.clone(),
                        );
                        self.state.dashboard.preview.loading = true;
                    }
                }
                if !self.state.dashboard.sidebar.loading {
                    self.spawn_load_documents();
                }
            }
            IngenieriaEvent::Session { action, developer, .. } => match action.as_str() {
                "connect" => {
                    if !self.state.sessions.iter().any(|s| s.developer == *developer) {
                        self.state.sessions.push(ActiveSession {
                            developer: developer.clone(),
                            time: system_time_str(),
                        });
                    }
                }
                "disconnect" => {
                    self.state.sessions.retain(|s| s.developer != *developer);
                }
                _ => {}
            },
            IngenieriaEvent::Heartbeat { .. } | IngenieriaEvent::Unknown => {}
            IngenieriaEvent::Connected { .. } => {}
        }
        if !matches!(event, IngenieriaEvent::Heartbeat { .. } | IngenieriaEvent::Unknown) {
            self.state.events.insert(0, TimedEvent::new(event));
            self.state.events.truncate(50);
        }
    }

    pub(crate) fn handle_documents_loaded(&mut self, docs: Vec<DocumentSummary>) {
        let docs_for_disk = docs.clone();
        tokio::spawn(async move {
            crate::services::doc_cache::save(&docs_for_disk);
        });
        self.state.caches.documents.insert("all".to_string(), docs.clone());
        self.state.dashboard.sidebar.all_docs = docs;
        self.state.dashboard.sidebar.loading = false;
        self.state.dashboard.sidebar.error = None;
        self.state.dashboard.sidebar.is_cached = false;
        let key = self.state.factory.filter_key();
        let priority = self.state.detected_factory.as_deref();
        self.state.dashboard.sidebar.rebuild_with_priority(key, priority);

        // Update dynamic workflows in command palette
        self.state.command.load_workflows(&self.state.dashboard.sidebar.all_docs);

        // Open pending doc picker if one was waiting for documents
        if let Some((doc_type, label)) = self.state.pending_picker.take() {
            self.open_doc_picker(&doc_type, &label);
        }
    }

    pub(crate) fn handle_documents_load_failed(&mut self, msg: String) {
        self.state.dashboard.sidebar.loading = false;
        if let Some((cached_docs, cached_at)) = crate::services::doc_cache::load() {
            let count = cached_docs.len();
            self.state.dashboard.sidebar.all_docs = cached_docs;
            self.state.dashboard.sidebar.error = None;
            self.state.dashboard.sidebar.is_cached = true;
            let key = self.state.factory.filter_key();
            let priority = self.state.detected_factory.as_deref();
            self.state.dashboard.sidebar.rebuild_with_priority(key, priority);
            let date = cached_at.get(..10).unwrap_or(&cached_at);
            self.notify(format!("⚠ MCP offline — {count} docs desde cache ({date})"));

            // Open pending picker with cached docs
            if let Some((doc_type, label)) = self.state.pending_picker.take() {
                self.open_doc_picker(&doc_type, &label);
            }
        } else {
            self.state.pending_picker = None;
            self.state.dashboard.sidebar.error = Some(msg);
            self.notify("✗ No se pudieron cargar los documentos".to_string());
        }
    }

    pub(crate) fn handle_sync_result(
        &mut self,
        updated_uris: Vec<String>,
        server_last_update: String,
    ) {
        self.state.dashboard.sync.loading = false;
        self.state.dashboard.sync.updated_uris = updated_uris.into_iter().collect();
        self.state.dashboard.sync.recompute_badges();
        let total = self.state.dashboard.sync.total_updated();
        if total > 0 {
            self.notify(format!("⟳ {total} docs actualizados desde último sync"));
        }
        if let Err(e) = crate::config::save_last_sync_date(&server_last_update) {
            tracing::warn!(error = %e, "Failed to save last_sync_date");
        }
    }

    pub(crate) fn handle_chat_stream_done(&mut self) {
        let has_tool_calls =
            self.state.chat.messages.last().map(|m| !m.tool_calls.is_empty()).unwrap_or(false);

        if has_tool_calls {
            self.state.chat.tool_rounds += 1;
            if self.state.chat.tool_rounds > MAX_TOOL_ROUNDS {
                self.state.chat.status = ChatStatus::Ready;
                self.state.chat.tool_rounds = 0;
                self.notify(format!("Limite de {MAX_TOOL_ROUNDS} rounds de tools alcanzado"));
                // Fall through a la cola: si el usuario encoló mensajes
                // mientras corría el ciclo de tools, no los dejamos colgados.
            } else {
                self.state.chat.status = ChatStatus::ExecutingTools;
                self.state.chat.snap_to_bottom();
                self.execute_pending_tool_calls();
                return;
            }
        } else {
            self.state.chat.status = ChatStatus::Ready;
            self.state.chat.tool_rounds = 0;
        }

        // E34: cerrar turno, capturar aggregates y notificar breakdown si >5s.
        let turn_summary = self.state.chat.metrics.on_turn_end();
        if let Some(ref turn) = turn_summary {
            if let Some(text) = crate::services::chat::metrics::format_turn_summary(turn) {
                self.notify(format!("⏱ {text}"));
            }
            // P3.8: persist fire-and-forget a JSONL para analytics cross-session.
            crate::services::chat_metrics_persist::append_turn_async(
                self.state.chat.session_id.clone(),
                turn.clone(),
            );
        }

        // Cache markdown for the finalized assistant message
        let mut structured_summary: Option<String> = None;
        if let Some(last) = self.state.chat.messages.last_mut() {
            if last.role == ChatRole::Assistant && last.cached_lines.is_none() {
                let cached = crate::ui::widgets::markdown::render_markdown(
                    &last.content,
                    &self.state.active_theme.colors(),
                );
                last.cached_lines = Some(std::sync::Arc::new(cached));

                // E19: detectar output estructurado en la respuesta final.
                // Silencioso si no parsea — el texto sigue siendo el payload.
                if last.structured.is_none() {
                    if let Some(parsed) =
                        crate::services::structured_output::detect_structured_output(&last.content)
                    {
                        tracing::info!(kind = parsed.kind_label(), "structured output detected");
                        structured_summary = Some(parsed.summary());
                        last.structured = Some(parsed);
                    }
                }
            }
        }
        if let Some(summary) = structured_summary {
            self.notify(format!("≡ {summary}"));
        }

        // Planning mode: auto-validate and transition to PlanReview
        if self.state.chat.mode == ChatMode::Planning {
            self.state.chat.mode = ChatMode::PlanReview;
            // E12: capturar plan estructurado si la AI respeto el formato.
            self.try_capture_pending_plan();
            if self.is_mcp_online() {
                if let Some(factory_key) = self.state.factory.filter_key() {
                    self.spawn_compliance_check(factory_key.to_string());
                    self.notify("Plan generado — validando compliance...".to_string());
                } else {
                    self.notify("Plan generado (selecciona una factory para validar compliance)".to_string());
                }
            } else {
                self.notify("Plan generado (MCP offline, sin validación)".to_string());
            }
        }

        // Auto-compact: aplica threshold de la estrategia Balanced (80%).
        // El usuario puede forzar otro perfil con `/compact aggressive|conservative`.
        if self.state.chat.needs_auto_compaction() {
            let outcome = self
                .state
                .chat
                .compact_with_strategy(crate::services::compactor::CompactionStrategy::Balanced);
            if outcome.removed_count > 0 {
                self.notify(format!(
                    "⚠ Auto-compact ({}): {} msgs resumidos (contexto al {:.0}%)",
                    outcome.strategy.label(),
                    outcome.removed_count,
                    self.state.chat.context_percent()
                ));
            }
        }

        self.detect_code_blocks();
        self.auto_save_history();

        // Auto-chain: if queue has committed items, send the first one
        if self.state.chat.message_queue.has_items() {
            if let Some(queued_msg) = self.state.chat.message_queue.pop_front() {
                self.state.chat.messages.push(ChatMessage::new(ChatRole::User, queued_msg.content));
                self.state.chat.status = ChatStatus::Streaming;
                self.state.chat.snap_to_bottom();
                self.state.chat.tool_rounds = 0;
                self.state.chat.metrics.on_turn_start();
                self.spawn_chat_completion();
            }
        } else if let Some(draft) = self.state.chat.message_queue.drain_to_input() {
            self.state.chat.input = draft;
        }
    }

    pub(crate) fn handle_chat_tool_result(&mut self, tool_call_id: String, content: String) {
        let is_err = content.starts_with("Error") || content.starts_with("Unknown tool");
        let mut tool_name = String::from("?");
        for msg in self.state.chat.messages.iter_mut().rev() {
            if let Some(tc) = msg.tool_calls.iter_mut().find(|tc| tc.id == tool_call_id) {
                tool_name = tc.name.clone();
                tc.status = if is_err {
                    crate::state::ToolCallStatus::Error
                } else {
                    crate::state::ToolCallStatus::Success
                };
                // E34: poblar duration_ms desde metrics tracker.
                if let Some(duration) =
                    self.state.chat.metrics.on_tool_end(&tool_call_id, tool_name.clone(), !is_err)
                {
                    tc.duration_ms = Some(duration.as_millis() as u64);
                }
                break;
            }
        }

        // Audit log (E13) fire-and-forget.
        let session_id = self.state.chat.session_id.clone();
        let audit_name = tool_name;
        let audit_call_id = tool_call_id.clone();
        let bytes = content.len();
        tokio::spawn(async move {
            crate::services::audit::log_entry(crate::services::audit::AuditEntry::new(
                session_id,
                crate::services::audit::AuditKind::ToolResult {
                    tool: audit_name,
                    tool_call_id: audit_call_id,
                    success: !is_err,
                    bytes,
                },
            ));
        });

        self.state.chat.messages.push(ChatMessage::tool_result(tool_call_id, content));
        self.state.chat.snap_to_bottom();
        self.maybe_continue_after_tools();
    }

    pub(crate) fn handle_copilot_models_loaded(
        &mut self,
        models: Vec<crate::services::copilot::CopilotModel>,
    ) {
        if self.state.mode == crate::state::AppMode::ModelPicker {
            self.state.model_picker.models = models;
            self.state.model_picker.cursor = 0;
            self.state.model_picker.loading = false;
            self.state.model_picker.error = None;
        } else {
            self.state.wizard.copilot.models = models;
            self.state.wizard.copilot.model_cursor = 0;
            self.state.wizard.copilot.error = None;
            self.state.wizard.model_phase = WizardModelPhase::SelectModel;
        }
    }

    pub(crate) fn handle_init_detected(
        &mut self,
        project_type: init_service::ProjectType,
        dir: String,
    ) {
        let resolved = if project_type == init_service::ProjectType::Unknown {
            factory_to_project_type(&self.state.factory)
        } else {
            project_type
        };
        self.state.init.detected_type = resolved.clone();
        self.state.init.project_dir = dir;
        self.state.init.type_cursor =
            init_service::ProjectType::ALL.iter().position(|t| *t == resolved).unwrap_or(0);
        self.state.init.step = InitStep::SelectType;
    }

    #[cfg(feature = "autoskill")]
    pub(crate) fn handle_autoskill_scan_done(
        &mut self,
        scan: crate::services::autoskill_map::AutoSkillScan,
    ) {
        use crate::services::autoskill_map;

        // Update detected factory siempre (independiente del modo de salida).
        if let Some(factory) = scan.primary_factory {
            self.state.detected_factory = Some(factory.to_string());
        }

        // Si el modal esta abierto (usuario pulso `:` + autoskill), poblar items
        // y retornar — no contaminamos el chat con markdown.
        if self.populate_autoskill_picker(&scan) {
            return;
        }

        // Flujo legacy (ej: atajo debug Shift+S): imprime resumen markdown en chat.
        let available_docs = &self.state.dashboard.sidebar.all_docs;
        let ingenieria = autoskill_map::collect_ingenieria_skills(&scan, available_docs);
        let dir = std::env::current_dir().unwrap_or_default();
        let external = autoskill_map::collect_external_skills(&scan, &dir);
        let suggestions = autoskill_map::SkillSuggestions { ingenieria, external };
        self.state.pending_external_skills =
            suggestions.external.iter().filter(|s| !s.installed).map(|s| s.path.clone()).collect();

        let content = autoskill_map::format_scan(&scan, &suggestions);
        let cached = crate::ui::widgets::markdown::render_markdown(
            &content,
            &self.state.active_theme.colors(),
        );
        let mut chat_msg = ChatMessage::new(ChatRole::Assistant, content);
        chat_msg.cached_lines = Some(std::sync::Arc::new(cached));
        self.state.chat.messages.push(chat_msg);
        self.state.chat.scroll_offset = u16::MAX;
    }

    #[cfg(feature = "autoskill")]
    pub(crate) fn handle_skill_install_done(
        &mut self,
        summary: crate::services::skill_installer::InstallSummary,
    ) {
        let content = crate::services::skill_installer::format_summary(&summary);
        let cached = crate::ui::widgets::markdown::render_markdown(
            &content,
            &self.state.active_theme.colors(),
        );
        let mut chat_msg = ChatMessage::new(ChatRole::Assistant, content);
        chat_msg.cached_lines = Some(std::sync::Arc::new(cached));
        self.state.chat.messages.push(chat_msg);
        self.state.chat.scroll_offset = u16::MAX;
        self.state.pending_external_skills.clear();
        self.notify(format!(
            "Skills: {} instalados, {} fallidos",
            summary.installed, summary.failed
        ));
    }

    pub(crate) fn handle_project_type_detected(&mut self, detected: init_service::ProjectType) {
        if let Some(factory) = project_type_to_factory(&detected) {
            self.state.detected_factory = factory.api_key().map(String::from);
            if factory != self.state.factory {
                self.state.factory = factory.clone();
                self.notify(format!("ℹ Detectado: {}. Usa tab para cambiar", factory.label(),));
            }
            let key = self.state.factory.filter_key();
            let priority = self.state.detected_factory.as_deref();
            self.state.dashboard.sidebar.rebuild_with_priority(key, priority);
        }
    }
}

fn project_type_to_factory(pt: &init_service::ProjectType) -> Option<crate::state::UiFactory> {
    match pt {
        init_service::ProjectType::Net => Some(crate::state::UiFactory::Net),
        init_service::ProjectType::Ang => Some(crate::state::UiFactory::Ang),
        init_service::ProjectType::Nest => Some(crate::state::UiFactory::Nest),
        init_service::ProjectType::FullStack => Some(crate::state::UiFactory::All),
        init_service::ProjectType::Unknown => None,
    }
}

fn factory_to_project_type(factory: &crate::state::UiFactory) -> init_service::ProjectType {
    match factory {
        crate::state::UiFactory::Net => init_service::ProjectType::Net,
        crate::state::UiFactory::Ang => init_service::ProjectType::Ang,
        crate::state::UiFactory::Nest => init_service::ProjectType::Nest,
        crate::state::UiFactory::All => init_service::ProjectType::FullStack,
    }
}
