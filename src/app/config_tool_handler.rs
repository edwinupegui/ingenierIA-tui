//! Handler de `Action::ApplyConfigChange` (E20 ConfigTool).
//!
//! Despachado cuando el AI ejecuta el tool `update_config` con action=set.
//! El flujo:
//! 1. Captura `old_value` desde AppState antes de mutar.
//! 2. Valida + aplica el cambio (campo por campo).
//! 3. Audita como `AuditKind::ConfigUpdated { field, old, new }`.
//! 4. Notifica al usuario con toast Info.
//!
//! Campos soportados: `model`, `factory`, `permission_mode`, `theme`.
//! Validaciones ya ejecutadas en `services::tools::config_tool`, pero se
//! repiten aqui defensivamente (handler puede recibir Actions desde otros
//! callers futuros).

use crate::services::audit::{log_entry, AuditEntry, AuditKind};
use crate::services::tools::todowrite::{TodoInput, TodoInputStatus};
use crate::state::UiFactory;
use ingenieria_domain::todos::TodoStatus;

use super::App;

impl App {
    /// Aplica la sincronizacion del TodoList enviada por el AI via TodoWrite.
    /// Reemplaza la lista actual, preservando ids monotonicos asignados por
    /// `TodoList` (los ids existentes se descartan porque el AI no los envia).
    pub(crate) fn handle_apply_todo_write(&mut self, items: Vec<TodoInput>) {
        let before_total = self.state.chat.todos.len();
        self.state.chat.todos.clear();
        for item in &items {
            let id = self.state.chat.todos.add(item.content.clone());
            if item.status != TodoInputStatus::Pending {
                let status: TodoStatus = item.status.into();
                self.state.chat.todos.set_status(id, status);
            }
        }
        let after_total = self.state.chat.todos.len();
        self.notify(format!(
            "✓ TodoList sync: {before_total} → {after_total} items ({})",
            self.state.chat.todos.short_summary()
        ));
    }

    /// Aplica un cambio de configuracion solicitado por el AI (via ConfigTool).
    /// No-op si el campo es invalido o el valor no parsea (un toast Warning
    /// informa al usuario).
    pub(crate) fn handle_apply_config_change(&mut self, field: String, value: String) {
        let old_value = match field.as_str() {
            "model" => self.state.model.clone(),
            "factory" => self.state.factory.api_key().unwrap_or("all").to_string(),
            "permission_mode" => self.state.chat.agent_mode.label().to_string(),
            "theme" => self.state.active_theme.label().to_string(),
            _ => {
                self.notify_warning(format!("ConfigTool: campo '{field}' desconocido"));
                return;
            }
        };

        if !apply_field(self, &field, &value) {
            self.notify_warning(format!("ConfigTool: valor '{value}' invalido para '{field}'"));
            return;
        }

        audit_config_change(&self.state.chat.session_id, &field, &old_value, &value);
        self.notify(format!("⚙ {field}: {old_value} → {value} (via AI)"));
    }

    fn notify_warning(&mut self, msg: String) {
        self.notify_level(&msg, crate::state::ToastLevel::Warning);
    }
}

/// Aplica el cambio al AppState. Retorna `false` si el valor no parsea.
fn apply_field(app: &mut App, field: &str, value: &str) -> bool {
    match field {
        "model" => {
            if value.is_empty() {
                return false;
            }
            app.state.model = value.to_string();
            app.state.config_dirty = true;
            true
        }
        "factory" => {
            let parsed = UiFactory::from_key(Some(value));
            // `UiFactory::from_key` cae a All para inputs desconocidos, asi que
            // validamos explicitamente el whitelist aqui.
            if !matches!(value, "net" | "ang" | "nest" | "all") {
                return false;
            }
            app.state.factory = parsed;
            true
        }
        "permission_mode" => match value {
            "Ask" | "Standard" => {
                app.state.chat.agent_mode = crate::state::chat_types::AgentMode::Ask;
                true
            }
            "Auto" | "Permissive" => {
                app.state.chat.agent_mode = crate::state::chat_types::AgentMode::Auto;
                true
            }
            "Plan" | "Strict" => {
                app.state.chat.agent_mode = crate::state::chat_types::AgentMode::Plan;
                true
            }
            _ => false,
        },
        "theme" => {
            let Some(variant) = crate::state::parse_theme_variant(value) else {
                return false;
            };
            app.state.active_theme = variant;
            app.state.invalidate_markdown_caches();
            crate::config::persist_theme(crate::state::theme_variant_to_str(variant));
            true
        }
        _ => false,
    }
}

/// Fire-and-forget audit log para el cambio de config.
fn audit_config_change(session_id: &str, field: &str, old_value: &str, new_value: &str) {
    let entry = AuditEntry::new(
        session_id.to_string(),
        AuditKind::ConfigUpdated {
            field: field.to_string(),
            old_value: old_value.to_string(),
            new_value: new_value.to_string(),
        },
    );
    tokio::spawn(async move {
        log_entry(entry);
    });
}
