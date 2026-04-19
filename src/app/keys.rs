use crate::state::{AppMode, AppScreen};

use super::App;

impl App {
    pub(crate) fn open_dashboard(&mut self) {
        self.state.screen = AppScreen::Dashboard;
        if self.state.dashboard.sidebar.all_docs.is_empty() && !self.state.dashboard.sidebar.loading
        {
            self.spawn_load_documents();
            self.state.dashboard.sidebar.loading = true;
        }
        // Trigger sync check on dashboard open (non-blocking)
        if !self.state.dashboard.sync.loading {
            self.state.dashboard.sync.loading = true;
            self.spawn_sync_check();
        }
        // E39: visita al dashboard cuenta como exploracion.
        self.mark_onboarding_step(crate::services::onboarding::ChecklistStep::ExploreDashboard);
    }

    pub(super) fn on_esc(&mut self) -> bool {
        // Elicitation modal has top priority: intercept before anything else.
        if self.on_esc_elicitation() {
            return false;
        }
        // Monitor panel overlay (E26) absorbe Esc.
        if self.state.monitor_panel.is_some() {
            self.state.monitor_panel = None;
            return false;
        }
        // Transcript overlay (E33) absorbe Esc cuando esta activo.
        if self.on_esc_transcript() {
            return false;
        }
        // Modal de history search (E30b).
        if self.on_esc_history_search() {
            return false;
        }
        // Command palette is global — close it from any screen
        if self.state.mode == AppMode::Command {
            self.state.mode = AppMode::Normal;
            self.state.command.reset();
            return false;
        }

        if self.state.screen == AppScreen::Wizard {
            return self.on_esc_wizard();
        }

        if self.state.screen == AppScreen::Splash && self.state.splash_autocomplete.visible {
            self.state.splash_autocomplete.close();
            return false;
        }

        if self.state.screen == AppScreen::Init {
            self.on_esc_init();
            return false;
        }

        if self.state.screen == AppScreen::Chat {
            return self.on_esc_chat();
        }

        match self.state.mode {
            AppMode::Command => unreachable!(),
            AppMode::Search => {
                self.state.mode = AppMode::Normal;
                self.state.search.reset();
                self.cancel_search();
                false
            }
            AppMode::ModelPicker => {
                self.state.mode = AppMode::Normal;
                false
            }
            AppMode::ThemePicker => {
                if let Some(picker) = self.state.theme_picker.take() {
                    // Revertir al original.
                    self.apply_theme(picker.original, false);
                }
                self.state.mode = AppMode::Normal;
                false
            }
            #[cfg(feature = "autoskill")]
            AppMode::AutoskillPicker => {
                self.close_autoskill_picker();
                false
            }
            AppMode::Normal => {
                if self.dismiss_overlay() {
                    return false;
                }
                match self.state.screen {
                    AppScreen::Dashboard => {
                        // Return to active chat if one exists, otherwise splash
                        if self.has_active_chat() {
                            self.state.screen = AppScreen::Chat;
                        } else {
                            self.state.screen = AppScreen::Splash;
                        }
                        false
                    }
                    // Esc never quits — use ctrl+c to exit
                    AppScreen::Splash => false,
                    AppScreen::Wizard | AppScreen::Init | AppScreen::Chat => false,
                }
            }
        }
    }

    pub(super) fn on_enter(&mut self) {
        if self.on_enter_elicitation() {
            return;
        }
        if self.on_enter_transcript() {
            return;
        }
        if self.on_enter_history_search() {
            return;
        }
        if self.state.screen == AppScreen::Init {
            self.on_enter_init();
            return;
        }

        if self.state.screen == AppScreen::Wizard {
            self.on_enter_wizard();
            return;
        }

        match self.state.mode {
            AppMode::Command => {
                self.execute_command();
            }
            AppMode::Search => {
                if let Some(r) = self.state.search.results.get(self.state.search.cursor) {
                    let (dt, factory, name) =
                        (r.doc_type.clone(), r.factory.clone(), r.name.clone());
                    self.state.mode = AppMode::Normal;
                    self.state.screen = AppScreen::Dashboard;
                    self.state.search.reset();
                    self.cancel_search();
                    self.spawn_fetch_document(dt, factory, name);
                    self.state.dashboard.preview.loading = true;
                    self.state.dashboard.preview.scroll = 0;
                }
            }
            AppMode::ModelPicker => {
                if let Some(model_id) = self.state.model_picker.selected_model_id() {
                    self.state.model = model_id.to_string();
                    self.state.mode = AppMode::Normal;
                    self.state.config_dirty = true;
                    self.notify(format!("Modelo: {}", self.state.model));
                }
            }
            AppMode::ThemePicker => {
                if let Some(picker) = self.state.theme_picker.take() {
                    let chosen = picker.selected().unwrap_or(picker.original);
                    self.apply_theme(chosen, true);
                }
                self.state.mode = AppMode::Normal;
            }
            #[cfg(feature = "autoskill")]
            AppMode::AutoskillPicker => {
                self.install_selected_autoskills();
            }
            AppMode::Normal => match self.state.screen {
                AppScreen::Wizard | AppScreen::Init => {}
                AppScreen::Splash => {
                    if self.state.splash_autocomplete.visible {
                        self.complete_splash_slash_command();
                        return;
                    }
                    if self.try_splash_slash_command() {
                        return;
                    }
                    if !self.state.input.trim().is_empty() {
                        self.start_chat();
                    }
                }
                AppScreen::Chat => {
                    self.on_enter_chat();
                }
                AppScreen::Dashboard => {
                    if let Some(doc) = self.state.dashboard.sidebar.current_doc() {
                        let (dt, factory, name) =
                            (doc.doc_type.clone(), doc.factory.clone(), doc.name.clone());
                        self.spawn_fetch_document(dt, factory, name);
                        self.state.dashboard.preview.loading = true;
                        self.state.dashboard.preview.scroll = 0;
                    }
                }
            },
        }
    }

    pub(super) fn on_tab(&mut self) {
        if self.state.mode != AppMode::Normal {
            return;
        }
        if self.state.screen == AppScreen::Splash && self.state.splash_autocomplete.visible {
            self.complete_splash_slash_command();
            return;
        }
        // En Chat: si el popup de slash-autocomplete esta visible, Tab completa
        // (prefijo comun, unico match, o ciclo por candidatos) en vez de cambiar factory.
        if self.state.screen == AppScreen::Chat && self.on_tab_chat() {
            return;
        }
        self.state.factory = self.state.factory.next();
        if self.state.screen == AppScreen::Dashboard {
            let key = self.state.factory.filter_key();
            let priority = self.state.detected_factory.as_deref();
            self.state.dashboard.sidebar.rebuild_with_priority(key, priority);
        }
        self.state.config_dirty = true;
        // E39: primera seleccion explicita de factory.
        self.mark_onboarding_step(crate::services::onboarding::ChecklistStep::SelectFactory);
    }

    /// Shift+Tab: cicla el AgentMode (Ask → Auto → Plan → Ask) en la pantalla de chat.
    pub(super) fn on_backtab(&mut self) {
        if self.state.screen != AppScreen::Chat {
            return;
        }
        let new_mode = self.state.chat.agent_mode.next();
        let entering_plan = new_mode == crate::state::AgentMode::Plan;
        let leaving_plan = self.state.chat.agent_mode == crate::state::AgentMode::Plan;
        self.state.chat.agent_mode = new_mode.clone();
        if entering_plan {
            self.activate_plan_mode();
        } else if leaving_plan {
            self.state.chat.mode = crate::state::ChatMode::Normal;
        }
        self.notify(format!(
            "{} Modo: {}  (Shift+Tab para cambiar)",
            new_mode.icon(),
            new_mode.label()
        ));
    }

    /// Check if command palette should open on this char press.
    fn try_open_command_palette(&mut self, c: char) -> bool {
        let kb_key = &self.state.keybindings.command_palette;
        if c.to_string() != *kb_key {
            return false;
        }
        match self.state.screen {
            AppScreen::Splash if self.state.input.is_empty() => {}
            AppScreen::Chat
                if self.state.chat.input.is_empty()
                    && self.state.chat.status == crate::state::ChatStatus::Ready
                    && !self.state.chat.doc_picker.visible => {}
            AppScreen::Dashboard => {}
            _ => return false,
        }
        self.state.mode = AppMode::Command;
        self.state.command.reset();
        // Populate recent history from chat input history
        self.state.command.recent_history =
            self.state.chat.input_history.iter().rev().take(5).cloned().collect();
        true
    }

    pub(super) fn on_char(&mut self, c: char) {
        // Monitor panel absorbe 'f' (follow toggle) y 'k' (kill).
        if self.state.monitor_panel.is_some() {
            self.on_char_monitor_panel(c);
            return;
        }
        if self.on_char_elicitation(c) {
            return;
        }
        if self.on_char_transcript(c) {
            return;
        }
        if self.on_char_history_search(c) {
            return;
        }
        if self.state.screen == AppScreen::Init {
            self.on_char_init(c);
            return;
        }

        if self.state.screen == AppScreen::Wizard {
            self.on_char_wizard(c);
            return;
        }

        // Command palette input (works from any screen while in Command mode)
        if self.state.mode == AppMode::Command {
            self.state.command.query.push(c);
            let len = self.state.command.filtered().len();
            if self.state.command.cursor >= len && len > 0 {
                self.state.command.cursor = len - 1;
            }
            return;
        }

        // Try to open command palette before screen-specific handlers
        if self.state.mode == AppMode::Normal && self.try_open_command_palette(c) {
            return;
        }

        if self.state.screen == AppScreen::Chat {
            self.on_char_chat(c);
            return;
        }

        match self.state.mode {
            AppMode::Command => unreachable!(),
            AppMode::Search => {
                self.state.search.query.push(c);
                let q = self.state.search.query.clone();
                self.spawn_search_debounced(q);
            }
            AppMode::ModelPicker => {}
            AppMode::ThemePicker => {
                if let Some(picker) = self.state.theme_picker.as_mut() {
                    picker.query.push(c);
                    picker.cursor = 0;
                    self.apply_theme_preview();
                }
            }
            #[cfg(feature = "autoskill")]
            AppMode::AutoskillPicker => {
                if c == ' ' {
                    self.toggle_autoskill_current();
                }
            }
            AppMode::Normal => match self.state.screen {
                AppScreen::Wizard | AppScreen::Init | AppScreen::Chat => {}
                AppScreen::Splash => {
                    self.state.input.push(c);
                    self.update_splash_autocomplete();
                }
                AppScreen::Dashboard => {
                    let kb = &self.state.keybindings;
                    let cs = c.to_string();
                    // Space char " " must match keybinding name "space"
                    let cs_norm = if cs == " " { "space".to_string() } else { cs.clone() };
                    if cs == kb.search {
                        self.state.mode = AppMode::Search;
                        self.state.search.reset();
                    } else if cs_norm == kb.toggle_sidebar {
                        self.state.dashboard.sidebar.toggle_current();
                    } else if cs == kb.copy {
                        self.copy_to_clipboard();
                    } else if c == 'Y' {
                        self.copy_slash_command();
                    } else if c == 'f' && self.state.panels.show_tool_monitor {
                        self.state.tool_monitor_filter = self.state.tool_monitor_filter.next();
                    } else if c == 'T' {
                        self.state.panels.show_tool_monitor = !self.state.panels.show_tool_monitor;
                    } else if c == 'H' {
                        self.state.panels.show_enforcement = !self.state.panels.show_enforcement;
                    } else if c == 'K' {
                        self.state.panels.show_agents = !self.state.panels.show_agents;
                    } else if c == 'N' {
                        self.state.panels.show_notifications =
                            !self.state.panels.show_notifications;
                    } else if c == 'S' {
                        #[cfg(feature = "autoskill")]
                        {
                            self.spawn_autoskill_scan();
                            self.notify("Escaneando tecnologias y skills...".to_string());
                        }
                        #[cfg(not(feature = "autoskill"))]
                        self.notify("Feature 'autoskill' no habilitada".to_string());
                    }
                }
            },
        }
    }

    pub(super) fn on_backspace(&mut self) {
        if self.on_backspace_elicitation() {
            return;
        }
        if self.on_backspace_transcript() {
            return;
        }
        if self.on_backspace_history_search() {
            return;
        }
        // Command palette backspace works from any screen
        if self.state.mode == AppMode::Command {
            self.state.command.query.pop();
            return;
        }
        if self.state.screen == AppScreen::Chat {
            self.on_backspace_chat();
            return;
        }
        if self.state.screen == AppScreen::Wizard {
            self.on_backspace_wizard();
            return;
        }

        match self.state.mode {
            AppMode::Command => unreachable!(),
            AppMode::Search => {
                self.state.search.query.pop();
                let q = self.state.search.query.clone();
                self.spawn_search_debounced(q);
            }
            AppMode::ModelPicker => {}
            AppMode::ThemePicker => {
                if let Some(picker) = self.state.theme_picker.as_mut() {
                    picker.query.pop();
                    picker.cursor = 0;
                    self.apply_theme_preview();
                }
            }
            #[cfg(feature = "autoskill")]
            AppMode::AutoskillPicker => {}
            AppMode::Normal => {
                if self.state.screen == AppScreen::Splash {
                    self.state.input.pop();
                    self.update_splash_autocomplete();
                }
            }
        }
    }

    pub(super) fn on_up(&mut self) {
        if let Some(panel) = &mut self.state.monitor_panel {
            panel.follow = false;
            panel.scroll_offset = panel.scroll_offset.saturating_add(3);
            return;
        }
        if self.on_up_elicitation() {
            return;
        }
        if self.on_up_transcript() {
            return;
        }
        if self.on_up_history_search() {
            return;
        }
        // Command palette navigation is global
        if self.state.mode == AppMode::Command {
            self.state.command.move_up();
            return;
        }
        if self.state.screen == AppScreen::Splash && self.state.splash_autocomplete.visible {
            self.state.splash_autocomplete.move_up();
            return;
        }
        if self.state.screen == AppScreen::Chat {
            self.on_up_chat();
            return;
        }
        if self.state.screen == AppScreen::Init {
            self.on_char('k');
            return;
        }
        if self.state.screen == AppScreen::Wizard {
            self.state.wizard.move_up();
            return;
        }

        match self.state.mode {
            AppMode::Command => unreachable!(),
            AppMode::Search => self.state.search.move_up(),
            AppMode::ModelPicker => self.state.model_picker.move_up(),
            AppMode::ThemePicker => {
                if let Some(picker) = self.state.theme_picker.as_mut() {
                    picker.move_up();
                }
                self.apply_theme_preview();
            }
            #[cfg(feature = "autoskill")]
            AppMode::AutoskillPicker => {
                if let Some(picker) = self.state.autoskill_picker.as_mut() {
                    picker.move_up();
                }
            }
            AppMode::Normal => {
                if self.state.screen == AppScreen::Dashboard {
                    self.state.dashboard.sidebar.move_up();
                }
            }
        }
    }

    pub(super) fn on_down(&mut self) {
        if let Some(panel) = &mut self.state.monitor_panel {
            panel.scroll_offset = panel.scroll_offset.saturating_sub(3);
            if panel.scroll_offset == 0 {
                panel.follow = true;
            }
            return;
        }
        if self.on_down_elicitation() {
            return;
        }
        if self.on_down_transcript() {
            return;
        }
        if self.on_down_history_search() {
            return;
        }
        // Command palette navigation is global
        if self.state.mode == AppMode::Command {
            let len = self.state.command.filtered().len();
            self.state.command.move_down(len);
            return;
        }
        if self.state.screen == AppScreen::Splash && self.state.splash_autocomplete.visible {
            self.state.splash_autocomplete.move_down();
            return;
        }
        if self.state.screen == AppScreen::Chat {
            self.on_down_chat();
            return;
        }
        if self.state.screen == AppScreen::Init {
            self.on_char('j');
            return;
        }
        if self.state.screen == AppScreen::Wizard {
            self.state.wizard.move_down();
            return;
        }

        match self.state.mode {
            AppMode::Command => unreachable!(),
            AppMode::Search => self.state.search.move_down(),
            AppMode::ModelPicker => self.state.model_picker.move_down(),
            AppMode::ThemePicker => {
                if let Some(picker) = self.state.theme_picker.as_mut() {
                    let len = picker.filtered().len();
                    picker.move_down(len);
                }
                self.apply_theme_preview();
            }
            #[cfg(feature = "autoskill")]
            AppMode::AutoskillPicker => {
                if let Some(picker) = self.state.autoskill_picker.as_mut() {
                    picker.move_down();
                }
            }
            AppMode::Normal => {
                if self.state.screen == AppScreen::Dashboard {
                    self.state.dashboard.sidebar.move_down();
                }
            }
        }
    }

    pub(super) fn on_left(&mut self) {
        if self.state.screen == AppScreen::Wizard {
            self.state.wizard.cursor_left();
        }
    }

    pub(super) fn on_right(&mut self) {
        if self.state.screen == AppScreen::Wizard {
            self.state.wizard.cursor_right();
        }
    }

    pub(super) fn on_home(&mut self) {
        if self.state.screen == AppScreen::Wizard {
            self.state.wizard.cursor_home();
        }
    }

    pub(super) fn on_end(&mut self) {
        if self.state.screen == AppScreen::Wizard {
            self.state.wizard.cursor_end();
        }
    }

    pub(super) fn on_delete(&mut self) {
        if self.state.screen == AppScreen::Wizard {
            self.on_delete_wizard();
        }
    }

    /// Dismiss the first visible overlay. Returns true if one was closed.
    fn dismiss_overlay(&mut self) -> bool {
        let flags = [
            &mut self.state.panels.show_doctor,
            &mut self.state.panels.show_tool_monitor,
            &mut self.state.panels.show_enforcement,
            &mut self.state.panels.show_agents,
            &mut self.state.panels.show_notifications,
            &mut self.state.panels.show_sessions,
        ];
        for flag in flags {
            if *flag {
                *flag = false;
                return true;
            }
        }
        false
    }
}
