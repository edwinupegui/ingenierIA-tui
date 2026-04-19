//! Plugin handlers (E28).
//!
//! Slash commands:
//!   /plugins — list loaded plugins

use crate::state::{ChatMessage, ChatRole};

use super::App;

impl App {
    /// Run `on_init` effects for all registered plugins.
    /// Called once during startup, after plugins have been registered.
    pub(crate) fn dispatch_plugin_init_effects(&mut self) {
        let effects = self.state.plugins.on_init();
        for effect in effects {
            self.apply_plugin_effect(effect);
        }
    }

    /// Apply a single plugin effect to the app.
    fn apply_plugin_effect(&mut self, effect: ingenieria_domain::plugin::PluginEffect) {
        match effect {
            ingenieria_domain::plugin::PluginEffect::Notify { message, level } => {
                let toast_level = match level {
                    ingenieria_domain::plugin::NotifyLevel::Info => crate::state::ToastLevel::Info,
                    ingenieria_domain::plugin::NotifyLevel::Warning => {
                        crate::state::ToastLevel::Warning
                    }
                    ingenieria_domain::plugin::NotifyLevel::Error => crate::state::ToastLevel::Error,
                };
                self.state.toasts.push(message, toast_level, self.state.tick_count);
            }
            ingenieria_domain::plugin::PluginEffect::InjectMessage { content } => {
                self.state
                    .chat
                    .messages
                    .push(ChatMessage::new(ChatRole::User, format!("[plugin] {content}")));
            }
            ingenieria_domain::plugin::PluginEffect::AuditLog { kind, detail } => {
                tracing::info!(kind, detail, "plugin audit log");
            }
        }
    }

    /// `/plugins reload` — re-scan plugins directory.
    pub(crate) fn handle_plugins_reload(&mut self) {
        // Shutdown existing plugins before replacing.
        self.state.plugins.on_shutdown();
        self.state.plugins = crate::services::plugins::PluginRegistry::default();
        let loaded = if let Some(dir) = crate::services::plugins::default_plugins_dir() {
            crate::services::plugins::load_from_dir(&mut self.state.plugins, &dir)
        } else {
            0
        };
        if loaded > 0 {
            self.dispatch_plugin_init_effects();
        }
        self.notify(format!("✓ Plugins recargados: {loaded} cargados"));
    }

    /// `/plugins` — show loaded plugins.
    pub(crate) fn handle_plugins_command(&mut self) {
        let body = format_plugins_list(&self.state.plugins);
        let cached =
            crate::ui::widgets::markdown::render_markdown(&body, &self.state.active_theme.colors());
        let mut msg = ChatMessage::new(ChatRole::Assistant, body);
        msg.cached_lines = Some(std::sync::Arc::new(cached));
        self.state.chat.messages.push(msg);
        self.state.chat.scroll_offset = u16::MAX;
    }
}

fn format_plugins_list(registry: &crate::services::plugins::PluginRegistry) -> String {
    let mut out = String::from("## Plugins\n\n");
    let plugins = registry.list();
    if plugins.is_empty() {
        out.push_str("No hay plugins cargados.\n\n");
        out.push_str(
            "Los plugins se implementan con el trait `ingenieria_domain::plugin::Plugin`.\n",
        );
        return out;
    }
    out.push_str("| Plugin | Version |\n");
    out.push_str("|--------|----------|\n");
    for (name, version) in &plugins {
        out.push_str(&format!("| {name} | {version} |\n"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_plugins_empty() {
        let reg = crate::services::plugins::PluginRegistry::default();
        let output = format_plugins_list(&reg);
        assert!(output.contains("No hay plugins"));
    }
}
