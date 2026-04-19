use crate::{
    config::Config,
    services::copilot as copilot_service,
    state::{AppScreen, WizardModelPhase, WizardStep},
};

use super::App;

/// Save Claude API key to config directory.
fn save_claude_api_key(key: &str) {
    let Some(dir) = dirs::config_dir() else { return };
    let path = dir.join("ingenieria-tui").join("claude_key");
    if let Err(e) = std::fs::create_dir_all(path.parent().unwrap_or(&dir)) {
        tracing::warn!(error = %e, "Failed to create config directory");
        return;
    }
    if let Err(e) = std::fs::write(&path, key) {
        tracing::warn!(error = %e, "Failed to save Claude API key");
    } else {
        // Restrict permissions on Unix (file contains API secret)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
        }
    }
}

/// Load Claude API key from config directory.
pub fn load_claude_api_key() -> Option<String> {
    let path = dirs::config_dir()?.join("ingenieria-tui").join("claude_key");
    std::fs::read_to_string(path).ok().filter(|s| !s.is_empty())
}

impl App {
    pub(crate) fn complete_wizard(&mut self) {
        let server_url = self.state.wizard.server_url_input.trim().to_string();
        let developer = self.state.wizard.name_input.trim().to_string();
        let provider = self.state.wizard.selected_provider_id().to_string();
        let model = self.state.wizard.selected_model_id();
        let default_factory = self.state.wizard.selected_role_key().to_string();

        self.state.developer = developer.clone();
        self.state.model = model.clone();
        self.state.factory = crate::state::UiFactory::from_key(Some(&default_factory));
        self.client.set_base_url(&server_url);

        if self.wizard_from_config {
            self.state.screen = AppScreen::Dashboard;
            self.notify("✓ Configuración guardada".to_string());
        } else {
            self.state.screen = AppScreen::Splash;
        }
        // E39: completar el wizard equivale a ConfigureServer.
        self.mark_onboarding_step(crate::services::onboarding::ChecklistStep::ConfigureServer);

        tokio::spawn(async move {
            let cfg = Config {
                server_url,
                developer,
                provider,
                model,
                default_factory: Some(default_factory),
                theme: None,
            };
            if let Err(e) = cfg.save() {
                tracing::warn!(error = %e, "No se pudo guardar la configuración");
            }
        });
    }

    pub(crate) fn handle_wizard_model_enter(&mut self) {
        match self.state.wizard.model_phase {
            WizardModelPhase::SelectProvider => {
                let provider = self.state.wizard.selected_provider_id();
                match provider {
                    "github-copilot" => {
                        let host = "github.com".to_string();
                        self.state.wizard.copilot.github_host = host.clone();
                        if let Some(auth) = copilot_service::load_saved_auth() {
                            self.state.wizard.copilot.oauth_token = auth.oauth_token.clone();
                            self.state.wizard.copilot.github_host = auth.github_host.clone();
                            self.state.wizard.copilot.auth_done = true;
                            self.state.wizard.model_phase = WizardModelPhase::Authenticating;
                            self.spawn_copilot_models(auth.github_host, auth.oauth_token);
                        } else {
                            self.spawn_copilot_device_code(host);
                        }
                    }
                    "claude-api" => {
                        // Switch to API key input phase
                        self.state.wizard.model_phase = WizardModelPhase::Authenticating;
                    }
                    _ => {}
                }
            }
            WizardModelPhase::Authenticating => {
                let provider = self.state.wizard.selected_provider_id();
                if provider == "claude-api" {
                    let key = self.state.wizard.claude_api_key.trim().to_string();
                    if key.is_empty() {
                        return;
                    }
                    // Save the API key and proceed to role selection
                    save_claude_api_key(&key);
                    self.state.wizard.step = WizardStep::Role;
                }
            }
            WizardModelPhase::SelectModel => {
                if !self.state.wizard.copilot.models.is_empty() {
                    self.state.wizard.step = WizardStep::Role;
                }
            }
        }
    }

    pub(crate) fn handle_copilot_device_code(
        &mut self,
        user_code: String,
        verification_uri: String,
        device_code: String,
    ) {
        self.state.wizard.copilot.user_code = user_code.clone();
        self.state.wizard.copilot.verification_uri = verification_uri.clone();
        self.state.wizard.copilot.device_code = device_code.clone();
        self.state.wizard.copilot.error = None;
        self.state.wizard.copilot.code_copied_at = Some(self.state.tick_count);
        self.state.wizard.model_phase = WizardModelPhase::Authenticating;
        let code = user_code;
        std::thread::spawn(move || {
            if let Ok(mut cb) = arboard::Clipboard::new() {
                let _ = cb.set_text(code);
            }
        });
        copilot_service::open_browser(&verification_uri);
        self.spawn_copilot_poll(self.state.wizard.copilot.github_host.clone(), device_code);
    }

    pub(crate) fn handle_copilot_auth_success(&mut self, token: String) {
        self.state.wizard.copilot.oauth_token = token.clone();
        self.state.wizard.copilot.auth_done = true;
        self.state.wizard.copilot.error = None;
        let host = self.state.wizard.copilot.github_host.clone();
        let auth =
            copilot_service::CopilotAuth { oauth_token: token.clone(), github_host: host.clone() };
        tokio::spawn(async move {
            if let Err(e) = copilot_service::save_auth(&auth) {
                tracing::warn!(error = %e, "Failed to save copilot auth");
            }
        });
        self.spawn_copilot_models(host, token);
    }
}
