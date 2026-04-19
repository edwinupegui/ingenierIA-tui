use super::App;

impl App {
    /// Update splash slash autocomplete state based on current input.
    pub(super) fn update_splash_autocomplete(&mut self) {
        let input = &self.state.input;
        if input.starts_with('/') && !input.contains(' ') {
            self.state.splash_autocomplete.update(input);
        } else {
            self.state.splash_autocomplete.close();
        }
    }

    /// Complete the splash input with the selected slash command.
    pub(super) fn complete_splash_slash_command(&mut self) {
        if let Some(cmd) = self.state.splash_autocomplete.selected_command() {
            self.state.splash_autocomplete.close();
            // Set input and try to execute immediately
            self.state.input = cmd.to_string();
            if !self.try_splash_slash_command() {
                // Not a splash command — add space for args
                self.state.input = format!("{cmd} ");
            }
        }
    }

    /// Try to handle slash commands directly from the splash screen.
    /// Returns true if the input was a recognized slash command.
    pub(super) fn try_splash_slash_command(&mut self) -> bool {
        let text = self.state.input.trim().to_string();
        let parts: Vec<&str> = text.splitn(2, ' ').collect();
        let cmd = parts[0];
        let arg = parts.get(1).map(|s| s.trim()).unwrap_or("");
        match cmd {
            "/skills" | "/workflows" | "/commands" | "/adrs" | "/policies" | "/agents" => {
                let (doc_type, label) = match cmd {
                    "/skills" | "/workflows" => ("skill", "Skills"),
                    "/commands" => ("command", "Commands"),
                    "/adrs" => ("adr", "ADRs"),
                    "/policies" => ("policy", "Policies"),
                    "/agents" => ("agent", "Agents"),
                    _ => return false,
                };
                self.state.input.clear();
                self.state.splash_autocomplete.close();
                self.state.screen = crate::state::AppScreen::Chat;
                self.state.chat = crate::state::ChatState::new();
                self.state.chat.status = crate::state::ChatStatus::Ready;
                self.open_doc_picker(doc_type, label);
                true
            }
            "/workflow" => {
                self.state.splash_autocomplete.close();
                self.state.input.clear();
                self.state.screen = crate::state::AppScreen::Chat;
                self.state.chat = crate::state::ChatState::new();
                self.state.chat.status = crate::state::ChatStatus::Ready;
                if arg.is_empty() {
                    self.notify("Uso: /workflow <nombre>".to_string());
                } else {
                    self.spawn_load_workflow(arg.to_string());
                    self.notify(format!("Cargando workflow {arg}..."));
                }
                true
            }
            _ => false,
        }
    }
}
