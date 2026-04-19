use crate::{
    config::Config,
    services::copilot as copilot_service,
    state::{AppMode, AppScreen, WizardStep},
};

use super::App;

impl App {
    /// Navigate to Dashboard if not already there (for commands that show dashboard content).
    fn ensure_dashboard(&mut self) {
        if self.state.screen != AppScreen::Dashboard {
            self.open_dashboard();
        }
    }

    /// Navigate to Chat if not already there (for commands that show chat content).
    fn ensure_chat(&mut self) {
        if self.state.screen != AppScreen::Chat {
            self.state.screen = AppScreen::Chat;
        }
    }

    pub(crate) fn execute_command(&mut self) {
        // Auto-detect ingenieria:// URIs pasted into the command palette
        let query = self.state.command.query.trim().to_string();
        if query.starts_with("ingenieria://") {
            if let Some(uri) = crate::services::uri::parse(&query) {
                self.ensure_dashboard();
                self.spawn_fetch_document(uri.doc_type, uri.factory, uri.name);
                self.state.dashboard.preview.loading = true;
                self.notify(format!("Navegando a {query}..."));
                self.state.mode = AppMode::Normal;
                self.state.command.reset();
                return;
            }
        }

        let cmds = self.state.command.filtered();
        if let Some(cmd) = cmds.get(self.state.command.cursor) {
            let cmd_id = &cmd.id;
            match cmd_id.as_str() {
                "sync" => {
                    self.ensure_dashboard();
                    self.spawn_load_documents();
                    self.state.dashboard.sidebar.loading = true;
                    self.notify("↻ Sincronizando documentos...".to_string());
                }
                "health" => {
                    self.spawn_health_check();
                    self.notify("↻ Refrescando estado del servidor...".to_string());
                }
                "doctor" => {
                    self.exec_doctor_cmd();
                }
                "mcp-status" => {
                    self.ensure_chat();
                    self.handle_mcp_status_command();
                }
                "audit" => {
                    self.ensure_chat();
                    self.handle_audit_command("");
                }
                "context" => {
                    let next = self.state.factory.next();
                    self.switch_factory_context(next);
                }
                "search" => {
                    self.ensure_dashboard();
                    self.state.mode = AppMode::Search;
                    self.state.search.reset();
                    return; // skip the reset below, we're entering search mode
                }
                "skills" => self.open_doc_picker("skill", "Skills"),
                "commands" => self.open_doc_picker("command", "Commands"),
                "adrs" => self.open_doc_picker("adr", "ADRs"),
                "policies" => self.open_doc_picker("policy", "Policies"),
                "agents" => self.open_doc_picker("agent", "Agents"),
                "workflows" => self.open_doc_picker("workflow", "Workflows"),
                "model" => {
                    if let Some(auth) = copilot_service::load_saved_auth() {
                        self.state.model_picker = crate::state::ModelPickerState::new();
                        self.state.model_picker.loading = true;
                        self.state.mode = AppMode::ModelPicker;
                        self.spawn_copilot_models(auth.github_host, auth.oauth_token);
                        return;
                    } else {
                        self.notify("No hay sesion activa. Usa 'config' primero.".to_string());
                    }
                }
                "permissions" => {
                    let new_mode = self.state.chat.agent_mode.next();
                    self.state.chat.agent_mode = new_mode.clone();
                    self.notify(format!("Modo: {} {}", new_mode.icon(), new_mode.label()));
                }
                "plugins" => {
                    self.ensure_chat();
                    self.handle_plugins_command();
                }
                "plugins-reload" => {
                    self.handle_plugins_reload();
                }
                "config" => {
                    self.wizard_from_config = true;
                    self.state.screen = AppScreen::Wizard;
                    // Pre-fill wizard with current config values
                    let url = self.client.base_url().to_string();
                    let mut wiz =
                        crate::state::WizardState::new(&self.state.developer, &self.state.model);
                    wiz.server_url_cursor = url.len();
                    wiz.server_url_input = url;
                    wiz.name_cursor = self.state.developer.len();
                    // Pre-select current provider
                    let provider = crate::config::Config::resolve(None).provider;
                    wiz.provider_cursor = crate::state::WIZARD_PROVIDERS
                        .iter()
                        .position(|(id, _, _)| *id == provider)
                        .unwrap_or(0);
                    // Pre-select current role/factory
                    if let Some(key) = self.state.factory.filter_key() {
                        wiz.role_cursor = crate::state::WIZARD_ROLES
                            .iter()
                            .position(|(k, _, _)| *k == key)
                            .unwrap_or(0);
                    }
                    self.state.wizard = wiz;
                }
                "autoskill" => {
                    #[cfg(feature = "autoskill")]
                    {
                        self.open_autoskill_picker();
                    }
                    #[cfg(not(feature = "autoskill"))]
                    self.notify("Feature 'autoskill' no habilitada".to_string());
                }
                "init" => {
                    self.init_return_screen = self.state.screen.clone();
                    self.state.init = crate::state::InitState::new();
                    self.state.screen = AppScreen::Init;
                    self.spawn_init_detect();
                }
                id if id.starts_with("workflow ") => {
                    let workflow_name = id.strip_prefix("workflow ").unwrap_or("").to_string();
                    self.spawn_load_workflow(workflow_name.clone());
                    self.notify(format!("⟳ Cargando workflow {workflow_name}..."));
                }
                id if id.starts_with("history-") => {
                    if let Some(idx_str) = id.strip_prefix("history-") {
                        if let Ok(idx) = idx_str.parse::<usize>() {
                            if let Some(text) = self.state.command.recent_history.get(idx).cloned()
                            {
                                self.ensure_chat();
                                self.state.chat.input = text;
                            }
                        }
                    }
                }
                "disconnect" => {
                    let _ = copilot_service::delete_saved_auth();
                    self.wizard_from_config = true;
                    self.state.screen = AppScreen::Wizard;
                    let mut wiz =
                        crate::state::WizardState::new(&self.state.developer, &self.state.model);
                    wiz.server_url_input = self.client.base_url().to_string();
                    wiz.step = WizardStep::Model;
                    self.state.wizard = wiz;
                    self.notify("Proveedor desconectado".to_string());
                }
                "dashboard" => {
                    self.open_dashboard();
                }
                "home" => {
                    self.handle_exit_to_splash();
                }
                "theme" => {
                    self.open_theme_picker();
                    return;
                }
                "transcript" => {
                    self.handle_toggle_transcript();
                }
                "history-search" => {
                    self.handle_input_history_search();
                }
                _ => {}
            }
        }
        self.state.mode = AppMode::Normal;
        self.state.command.reset();
    }

    pub(crate) fn copy_to_clipboard(&mut self) {
        let Some(doc) = &self.state.dashboard.preview.doc else {
            return;
        };
        let content = doc.content.clone();
        copy_text_to_clipboard(content);
        self.notify("✓ Copiado al clipboard".to_string());
    }

    pub(crate) fn copy_slash_command(&mut self) {
        let Some(doc) = &self.state.dashboard.preview.doc else {
            return;
        };
        let slash = format!("/{}", doc.name);
        let msg = format!("✓ Copiado: {slash}");
        copy_text_to_clipboard(slash);
        self.notify(msg);
    }

    pub(crate) fn save_config(&self) {
        #[cfg(test)]
        return;

        #[allow(unreachable_code)]
        {
            let cfg = Config {
                server_url: self.client.base_url().to_string(),
                developer: self.state.developer.clone(),
                provider: "github-copilot".to_string(),
                model: self.state.model.clone(),
                default_factory: self.state.factory.api_key().map(String::from),
                theme: None,
            };
            tokio::spawn(async move {
                if let Err(e) = cfg.save() {
                    tracing::warn!(error = %e, "No se pudo guardar la configuración");
                }
            });
        }
    }

    pub(crate) fn is_mcp_online(&self) -> bool {
        matches!(self.state.server_status, crate::state::ServerStatus::Online(_))
    }

    pub(crate) fn notify_level(&mut self, msg: &str, level: crate::state::ToastLevel) {
        self.state.toasts.push(msg.to_string(), level, self.state.tick_count);
    }

    /// Backward-compatible wrapper: infers level from emoji prefix.
    /// Prefer `notify_level()` for new code.
    pub(crate) fn notify(&mut self, msg: String) {
        let level = if msg.starts_with('✓') || msg.starts_with('✔') {
            crate::state::ToastLevel::Success
        } else if msg.starts_with('✗') {
            crate::state::ToastLevel::Error
        } else if msg.starts_with('⚠') {
            crate::state::ToastLevel::Warning
        } else {
            crate::state::ToastLevel::Info
        };
        self.notify_level(&msg, level);
    }

    /// Check if there's an active chat session with messages.
    pub(crate) fn has_active_chat(&self) -> bool {
        !self.state.chat.messages.is_empty()
    }

    pub(crate) fn switch_factory_context(&mut self, factory: crate::state::UiFactory) {
        self.ensure_dashboard();
        let from = self.state.factory.label().to_string();
        self.state.factory = factory;
        let key = self.state.factory.filter_key();
        let priority = self.state.detected_factory.as_deref();
        self.state.dashboard.sidebar.rebuild_with_priority(key, priority);
        // Only fetch from server if no docs are loaded (cache handles freshness)
        if self.state.dashboard.sidebar.all_docs.is_empty() {
            self.spawn_load_documents();
            self.state.dashboard.sidebar.loading = true;
        }
        let to = self.state.factory.label().to_string();
        self.notify(format!("↻ Contexto {to}"));
        self.state.config_dirty = true;
        // E39: cambio de factory cuenta como SelectFactory.
        self.mark_onboarding_step(crate::services::onboarding::ChecklistStep::SelectFactory);
        if from != to {
            self.hooks.fire(
                crate::services::hooks::HookTrigger::OnFactorySwitch,
                crate::services::hooks::HookContext::for_factory_switch(&from, &to),
                self.tx.clone(),
            );
        }
    }
}

/// Copy text to clipboard: tries arboard first, falls back to OSC 52.
fn copy_text_to_clipboard(text: String) {
    std::thread::spawn(move || {
        if arboard::Clipboard::new().and_then(|mut cb| cb.set_text(&text)).is_err() {
            let _ = crate::ui::hyperlinks::osc52_copy(&text);
        }
    });
}
