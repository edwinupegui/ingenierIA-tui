// ── Wizard (first-run) ────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum WizardStep {
    ServerUrl,
    Name,
    Model,
    Role,
}

impl WizardStep {
    pub fn step_number(&self) -> usize {
        match self {
            WizardStep::ServerUrl => 1,
            WizardStep::Name => 2,
            WizardStep::Model => 3,
            WizardStep::Role => 4,
        }
    }

    pub fn total() -> usize {
        4
    }

    pub fn prev(&self) -> Option<WizardStep> {
        match self {
            WizardStep::ServerUrl => None,
            WizardStep::Name => Some(WizardStep::ServerUrl),
            WizardStep::Model => Some(WizardStep::Name),
            WizardStep::Role => Some(WizardStep::Model),
        }
    }
}

// ── Providers disponibles en el wizard ────────────────────────────────────────

/// (id, label, enabled)
pub const WIZARD_PROVIDERS: &[(&str, &str, bool)] =
    &[("github-copilot", "GitHub Copilot", true), ("claude-api", "Claude API (Anthropic)", true)];

/// Sub-fases dentro del paso 3 (Modelo/Provider) del wizard.
#[derive(Debug, Clone, PartialEq)]
pub enum WizardModelPhase {
    SelectProvider,
    Authenticating,
    SelectModel,
}

/// Estado del flujo OAuth de Copilot durante el wizard.
pub struct CopilotWizardState {
    pub user_code: String,
    pub verification_uri: String,
    pub device_code: String,
    pub github_host: String,
    pub oauth_token: String,
    pub models: Vec<crate::services::copilot::CopilotModel>,
    pub model_cursor: usize,
    pub error: Option<String>,
    pub auth_done: bool,
    pub code_copied_at: Option<u64>,
}

impl CopilotWizardState {
    pub fn new() -> Self {
        Self {
            user_code: String::new(),
            verification_uri: String::new(),
            device_code: String::new(),
            github_host: "github.com".to_string(),
            oauth_token: String::new(),
            models: Vec::new(),
            model_cursor: 0,
            error: None,
            auth_done: false,
            code_copied_at: None,
        }
    }

    pub fn selected_model_id(&self) -> Option<&str> {
        self.models.get(self.model_cursor).map(|m| m.id.as_str())
    }
}

pub const WIZARD_ROLES: &[(&str, &str, &str)] = &[
    ("net", "Backend", ".NET · C# · APIs · Clean Architecture"),
    ("ang", "Frontend", "Angular · TypeScript · Components"),
    ("nest", "BFF", "NestJS · TypeScript · API Gateway"),
    ("all", "Full Stack", "Backend + Frontend + BFF"),
];

#[derive(Debug, Clone, PartialEq)]
pub enum UrlValidation {
    Idle,
    Checking,
    Valid,
    Invalid(String),
}

pub struct WizardState {
    pub step: WizardStep,
    pub server_url_input: String,
    pub server_url_cursor: usize,
    pub url_validation: UrlValidation,
    pub name_input: String,
    pub name_cursor: usize,
    pub role_cursor: usize,
    // Provider / model sub-flow (step 3)
    pub model_phase: WizardModelPhase,
    pub provider_cursor: usize,
    pub copilot: CopilotWizardState,
    /// API key input for Claude provider.
    pub claude_api_key: String,
    pub claude_key_cursor: usize,
}

impl WizardState {
    pub fn new(default_name: &str, _default_model: &str) -> Self {
        let name_len = default_name.len();
        Self {
            step: WizardStep::ServerUrl,
            server_url_input: String::new(),
            server_url_cursor: 0,
            url_validation: UrlValidation::Idle,
            name_input: default_name.to_string(),
            name_cursor: name_len,
            role_cursor: 0,
            model_phase: WizardModelPhase::SelectProvider,
            provider_cursor: 0,
            copilot: CopilotWizardState::new(),
            claude_api_key: String::new(),
            claude_key_cursor: 0,
        }
    }

    /// Returns the selected model id based on the active provider.
    pub fn selected_model_id(&self) -> String {
        match self.selected_provider_id() {
            "claude-api" => "claude-sonnet-4-20250514".to_string(),
            _ => self
                .copilot
                .selected_model_id()
                .unwrap_or(WIZARD_PROVIDERS[self.provider_cursor].0)
                .to_string(),
        }
    }

    pub fn selected_provider_id(&self) -> &'static str {
        WIZARD_PROVIDERS[self.provider_cursor].0
    }

    pub fn selected_role_key(&self) -> &'static str {
        WIZARD_ROLES[self.role_cursor].0
    }

    /// Insert a char at cursor position in the active text input.
    pub fn insert_char(&mut self, c: char) {
        let (text, cursor) = self.active_input_mut();
        text.insert(*cursor, c);
        *cursor += c.len_utf8();
    }

    /// Delete the char before cursor (backspace) in the active text input.
    pub fn delete_char_before(&mut self) {
        let (text, cursor) = self.active_input_mut();
        if *cursor > 0 {
            // Find the previous char boundary
            let prev = text[..*cursor].char_indices().next_back().map(|(i, _)| i).unwrap_or(0);
            text.remove(prev);
            *cursor = prev;
        }
    }

    /// Delete the char at cursor (delete key) in the active text input.
    pub fn delete_char_at(&mut self) {
        let (text, cursor) = self.active_input_mut();
        if *cursor < text.len() {
            text.remove(*cursor);
        }
    }

    /// Move cursor left one char.
    pub fn cursor_left(&mut self) {
        let (text, cursor) = self.active_input_mut();
        if *cursor > 0 {
            *cursor = text[..*cursor].char_indices().next_back().map(|(i, _)| i).unwrap_or(0);
        }
    }

    /// Move cursor right one char.
    pub fn cursor_right(&mut self) {
        let (text, cursor) = self.active_input_mut();
        if *cursor < text.len() {
            let rest = &text[*cursor..];
            let next = rest.chars().next().map(|c| c.len_utf8()).unwrap_or(0);
            *cursor += next;
        }
    }

    /// Move cursor to start (Home).
    pub fn cursor_home(&mut self) {
        let (_, cursor) = self.active_input_mut();
        *cursor = 0;
    }

    /// Move cursor to end (End).
    pub fn cursor_end(&mut self) {
        let (text, cursor) = self.active_input_mut();
        *cursor = text.len();
    }

    /// Returns mutable references to the active text input and its cursor.
    fn active_input_mut(&mut self) -> (&mut String, &mut usize) {
        match self.step {
            WizardStep::ServerUrl => (&mut self.server_url_input, &mut self.server_url_cursor),
            WizardStep::Name => (&mut self.name_input, &mut self.name_cursor),
            WizardStep::Model => (&mut self.claude_api_key, &mut self.claude_key_cursor),
            WizardStep::Role => (&mut self.name_input, &mut self.name_cursor), // fallback
        }
    }

    pub fn move_up(&mut self) {
        match self.step {
            WizardStep::Model => match self.model_phase {
                WizardModelPhase::SelectProvider => {
                    self.provider_cursor = self.provider_cursor.saturating_sub(1);
                    // Skip disabled providers when moving up
                    while self.provider_cursor > 0 && !WIZARD_PROVIDERS[self.provider_cursor].2 {
                        self.provider_cursor = self.provider_cursor.saturating_sub(1);
                    }
                }
                WizardModelPhase::SelectModel => {
                    self.copilot.model_cursor = self.copilot.model_cursor.saturating_sub(1);
                }
                _ => {}
            },
            WizardStep::Role => {
                self.role_cursor = self.role_cursor.saturating_sub(1);
            }
            _ => {}
        }
    }

    pub fn move_down(&mut self) {
        match self.step {
            WizardStep::Model => match self.model_phase {
                WizardModelPhase::SelectProvider => {
                    let max = WIZARD_PROVIDERS.len() - 1;
                    self.provider_cursor = (self.provider_cursor + 1).min(max);
                    // Snap back to last enabled provider
                    if !WIZARD_PROVIDERS[self.provider_cursor].2 {
                        // find the last enabled one at or before cursor
                        while self.provider_cursor > 0 && !WIZARD_PROVIDERS[self.provider_cursor].2
                        {
                            self.provider_cursor -= 1;
                        }
                    }
                }
                WizardModelPhase::SelectModel => {
                    if !self.copilot.models.is_empty() {
                        self.copilot.model_cursor =
                            (self.copilot.model_cursor + 1).min(self.copilot.models.len() - 1);
                    }
                }
                _ => {}
            },
            WizardStep::Role => {
                self.role_cursor = (self.role_cursor + 1).min(WIZARD_ROLES.len() - 1);
            }
            _ => {}
        }
    }
}
