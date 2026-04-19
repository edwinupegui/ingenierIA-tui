use crate::{
    services::init as init_service,
    state::{InitStep, UrlValidation, WizardModelPhase, WizardStep},
};

use super::App;

impl App {
    /// Handle Esc key when on the Wizard screen.
    /// Returns `false` always (wizard never quits the app).
    pub(super) fn on_esc_wizard(&mut self) -> bool {
        if self.state.wizard.step == WizardStep::Model {
            match self.state.wizard.model_phase {
                WizardModelPhase::SelectProvider => {
                    self.state.wizard.step = WizardStep::Name;
                    return false;
                }
                WizardModelPhase::Authenticating => {
                    self.state.wizard.model_phase = WizardModelPhase::SelectProvider;
                    self.state.wizard.copilot.error = None;
                    return false;
                }
                WizardModelPhase::SelectModel => {
                    self.state.wizard.model_phase = WizardModelPhase::SelectProvider;
                    return false;
                }
            }
        }
        if let Some(prev) = self.state.wizard.step.prev() {
            let is_model = prev == WizardStep::Model;
            self.state.wizard.step = prev;
            if is_model {
                self.state.wizard.model_phase = WizardModelPhase::SelectProvider;
            }
        } else if self.wizard_from_config {
            self.state.screen = crate::state::AppScreen::Splash;
        }
        false
    }

    /// Handle Enter key when on the Wizard screen.
    pub(super) fn on_enter_wizard(&mut self) {
        match self.state.wizard.step {
            WizardStep::ServerUrl => {
                let url = self.state.wizard.server_url_input.trim().to_string();
                if !url.is_empty() {
                    self.state.wizard.url_validation = UrlValidation::Checking;
                    self.spawn_wizard_url_check(url);
                }
            }
            WizardStep::Name => {
                if !self.state.wizard.name_input.trim().is_empty() {
                    self.state.wizard.step = WizardStep::Model;
                }
            }
            WizardStep::Model => {
                self.handle_wizard_model_enter();
            }
            WizardStep::Role => {
                self.complete_wizard();
            }
        }
    }

    /// Handle character input when on the Wizard screen.
    pub(super) fn on_char_wizard(&mut self, c: char) {
        match self.state.wizard.step {
            WizardStep::ServerUrl => {
                self.state.wizard.insert_char(c);
                self.state.wizard.url_validation = UrlValidation::Idle;
            }
            WizardStep::Name => {
                self.state.wizard.insert_char(c);
            }
            WizardStep::Model => match self.state.wizard.model_phase {
                WizardModelPhase::Authenticating => {
                    if self.state.wizard.selected_provider_id() == "claude-api" {
                        self.state.wizard.insert_char(c);
                    } else if c == 'c' {
                        let code = self.state.wizard.copilot.user_code.clone();
                        std::thread::spawn(move || {
                            if let Ok(mut cb) = arboard::Clipboard::new() {
                                let _ = cb.set_text(code);
                            }
                        });
                        self.state.wizard.copilot.code_copied_at = Some(self.state.tick_count);
                    }
                }
                _ => match c {
                    'j' => self.state.wizard.move_down(),
                    'k' => self.state.wizard.move_up(),
                    _ => {}
                },
            },
            WizardStep::Role => match c {
                'j' => self.state.wizard.move_down(),
                'k' => self.state.wizard.move_up(),
                '1' | '2' | '3' => {
                    let idx = (c as usize) - ('1' as usize);
                    if idx < crate::state::WIZARD_ROLES.len() {
                        self.state.wizard.role_cursor = idx;
                        self.complete_wizard();
                    }
                }
                _ => {}
            },
        }
    }

    /// Handle Backspace key when on the Wizard screen.
    pub(super) fn on_backspace_wizard(&mut self) {
        match self.state.wizard.step {
            WizardStep::ServerUrl => {
                self.state.wizard.delete_char_before();
                self.state.wizard.url_validation = UrlValidation::Idle;
            }
            WizardStep::Name => {
                self.state.wizard.delete_char_before();
            }
            WizardStep::Model
                if self.state.wizard.model_phase == WizardModelPhase::Authenticating
                    && self.state.wizard.selected_provider_id() == "claude-api" =>
            {
                self.state.wizard.delete_char_before();
            }
            _ => {}
        }
    }

    /// Handle Delete key when on the Wizard screen.
    pub(super) fn on_delete_wizard(&mut self) {
        match self.state.wizard.step {
            WizardStep::ServerUrl => {
                self.state.wizard.delete_char_at();
                self.state.wizard.url_validation = UrlValidation::Idle;
            }
            WizardStep::Name => {
                self.state.wizard.delete_char_at();
            }
            WizardStep::Model
                if self.state.wizard.model_phase == WizardModelPhase::Authenticating
                    && self.state.wizard.selected_provider_id() == "claude-api" =>
            {
                self.state.wizard.delete_char_at();
            }
            _ => {}
        }
    }

    // ── Init screen key helpers ──────────────────────────────────────

    pub(super) fn on_esc_init(&mut self) {
        match self.state.init.step {
            InitStep::SelectType => {
                self.state.screen = self.init_return_screen.clone();
            }
            InitStep::SelectClient => {
                self.state.init.step = InitStep::SelectType;
            }
            InitStep::Confirm => {
                self.state.init.step = InitStep::SelectClient;
            }
            InitStep::Done => {
                self.state.screen = self.init_return_screen.clone();
            }
            InitStep::Running => {}
        }
    }

    pub(super) fn on_enter_init(&mut self) {
        match self.state.init.step {
            InitStep::SelectType => {
                self.state.init.step = InitStep::SelectClient;
            }
            InitStep::SelectClient => {
                self.state.init.step = InitStep::Confirm;
            }
            InitStep::Confirm => {
                self.state.init.step = InitStep::Running;
                self.spawn_init_run();
            }
            InitStep::Done => {
                self.state.screen = self.init_return_screen.clone();
            }
            InitStep::Running => {}
        }
    }

    pub(super) fn on_char_init(&mut self, c: char) {
        match self.state.init.step {
            InitStep::SelectType => match c {
                'j' => {
                    let max = init_service::ProjectType::ALL.len();
                    self.state.init.type_cursor = (self.state.init.type_cursor + 1).min(max - 1);
                }
                'k' => {
                    self.state.init.type_cursor = self.state.init.type_cursor.saturating_sub(1);
                }
                _ => {}
            },
            InitStep::SelectClient => match c {
                'j' => {
                    let max = init_service::InitClient::ALL.len();
                    self.state.init.client_cursor =
                        (self.state.init.client_cursor + 1).min(max - 1);
                }
                'k' => {
                    self.state.init.client_cursor = self.state.init.client_cursor.saturating_sub(1);
                }
                _ => {}
            },
            _ => {}
        }
    }
}
