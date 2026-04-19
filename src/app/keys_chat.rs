use crate::state::{AppScreen, ChatStatus};

use super::App;

impl App {
    /// Handle paste event. Dispatches to chat or splash.
    pub(super) fn on_paste(&mut self, text: String) {
        if self.state.screen == AppScreen::Chat {
            self.on_paste_chat(text);
        } else if self.state.screen == AppScreen::Splash {
            self.state.input.push_str(&text);
        }
    }

    fn on_paste_chat(&mut self, text: String) {
        let len = text.len();
        match self.state.chat.status {
            ChatStatus::Streaming | ChatStatus::ExecutingTools => {
                self.state.chat.message_queue.append_to_draft(&text);
            }
            ChatStatus::Ready => {
                // E40: snapshot pre-paste para Ctrl+Z. El paste grande tambien
                // se graba: la consecuencia es que Undo revierte el placeholder
                // completo (comportamiento deseado).
                let current = self.state.chat.input.clone();
                self.state.chat.input_undo.record_if_changed(&current);
                let classification = crate::services::paste_handler::classify(&text);
                if classification.is_large {
                    let id = self.state.chat.next_paste_id;
                    self.state.chat.next_paste_id += 1;
                    let placeholder = crate::services::paste_handler::make_placeholder(
                        id,
                        classification.line_count,
                    );
                    self.state.chat.pasted_refs.push(crate::services::paste_handler::PastedRef {
                        id,
                        full_text: text,
                        line_count: classification.line_count,
                    });
                    self.state.chat.input.push_str(&placeholder);
                } else {
                    self.state.chat.input.push_str(&text);
                }
            }
            _ => return,
        }
        if len >= 5000 {
            let display = format!("{:.1}KB", len as f64 / 1024.0);
            self.notify(format!("⚠ Pegado grande ({display}) — considera un archivo"));
        } else if len >= 500 {
            let display =
                if len >= 1000 { format!("{:.1}K", len as f64 / 1000.0) } else { format!("{len}") };
            self.notify(format!("(pasted {display} chars)"));
        }
    }

    /// Handle Shift+Enter in the Chat screen (inserts newline).
    pub(super) fn on_shift_enter(&mut self) {
        if self.state.screen == AppScreen::Chat && self.state.chat.status == ChatStatus::Ready {
            self.state.chat.input.push('\n');
        }
    }

    /// Handle Esc key when on the Chat screen. Returns false always.
    pub(super) fn on_esc_chat(&mut self) -> bool {
        if self.state.chat.mention_picker.visible {
            self.state.chat.mention_picker.close();
            return false;
        }
        if self.state.chat.doc_picker.visible {
            self.state.chat.doc_picker.close();
            return false;
        }
        if self.state.chat.slash_autocomplete.visible {
            self.state.chat.slash_autocomplete.close();
            return false;
        }
        if self.state.panels.show_cost_panel {
            self.state.panels.show_cost_panel = false;
            return false;
        }
        // During streaming: clear draft first, then pop queue, then abort turn,
        // then navigate. Esc para abortar solo dispara si no hay draft ni queue
        // pendiente, para no perder lo que el user estaba escribiendo.
        if matches!(self.state.chat.status, ChatStatus::Streaming | ChatStatus::ExecutingTools) {
            // Esc en el modal de permisos → denegar tool seleccionado.
            if self.state.chat.status == ChatStatus::ExecutingTools
                && !self.state.chat.pending_approvals.is_empty()
            {
                self.deny_chosen_tools();
                return false;
            }
            if !self.state.chat.message_queue.is_draft_empty() {
                self.state.chat.message_queue.draft_mut().clear();
                return false;
            }
            if self.state.chat.message_queue.has_items() {
                let count = self.state.chat.message_queue.len() - 1;
                self.state.chat.message_queue.pop_back();
                if count > 0 {
                    self.notify(format!("{count} mensaje(s) en cola"));
                }
                return false;
            }
            // Sin draft ni queue → Esc aborta el turn en curso.
            let _ = self.tx.try_send(crate::actions::Action::ChatStreamAbort);
            return false;
        }
        // Preserve conversation — go to dashboard, not splash
        self.state.screen = AppScreen::Dashboard;
        false
    }

    /// Handle Backspace key when on the Chat screen.
    pub(super) fn on_backspace_chat(&mut self) {
        if self.state.chat.mention_picker.visible {
            if self.state.chat.mention_picker.query.pop().is_some() {
                self.state.chat.mention_picker.recompute();
            } else {
                self.state.chat.mention_picker.close();
            }
            return;
        }
        if self.state.chat.doc_picker.visible {
            self.state.chat.doc_picker.query.pop();
            self.state.chat.doc_picker.update_filter();
            return;
        }
        if self.state.chat.status == ChatStatus::Ready {
            // E40: snapshot previo al cambio para soportar Ctrl+Z.
            let current = self.state.chat.input.clone();
            self.state.chat.input_undo.record_if_changed(&current);
            self.state.chat.input.pop();
            self.update_slash_autocomplete();
        } else if matches!(
            self.state.chat.status,
            ChatStatus::Streaming | ChatStatus::ExecutingTools
        ) {
            self.state.chat.message_queue.pop_draft_char();
        }
    }

    /// Handle Enter key when on the Chat screen (Normal mode).
    pub(super) fn on_enter_chat(&mut self) {
        if self.state.chat.status == ChatStatus::ExecutingTools
            && !self.state.chat.pending_approvals.is_empty()
        {
            self.approve_chosen_tools();
            return;
        }
        if self.state.chat.mention_picker.visible {
            if let Some(item) = self.state.chat.mention_picker.selected() {
                let insert = item.insert_text();
                self.state.chat.input.push_str(&insert);
                self.state.chat.input.push(' ');
            }
            self.state.chat.mention_picker.close();
            return;
        }
        if self.state.chat.doc_picker.visible {
            self.select_doc_picker_item();
            return;
        }
        match &self.state.chat.status {
            ChatStatus::Ready => {
                if self.state.chat.slash_autocomplete.visible {
                    self.complete_slash_command();
                    return;
                }
                if !self.state.chat.input.trim().is_empty() {
                    self.send_chat_message();
                }
            }
            ChatStatus::Error(_) => {
                if !self.state.chat.input.trim().is_empty() {
                    self.state.chat.status = ChatStatus::Ready;
                    self.send_chat_message();
                } else {
                    self.state.chat.status = ChatStatus::Streaming;
                    self.spawn_chat_completion();
                }
            }
            ChatStatus::Streaming | ChatStatus::ExecutingTools => {
                if !self.state.chat.message_queue.is_draft_empty() {
                    self.state.chat.message_queue.commit_draft();
                    let count = self.state.chat.message_queue.len();
                    self.notify(format!("{count} mensaje(s) en cola"));
                }
            }
            _ => {}
        }
    }

    /// Handle character input when on the Chat screen.
    pub(super) fn on_char_chat(&mut self, c: char) {
        // Mention picker input: absorbe chars mientras está visible.
        if self.state.chat.mention_picker.visible {
            self.state.chat.mention_picker.query.push(c);
            self.state.chat.mention_picker.recompute();
            return;
        }
        // Doc picker input
        if self.state.chat.doc_picker.visible {
            self.state.chat.doc_picker.query.push(c);
            self.state.chat.doc_picker.update_filter();
            return;
        }
        // `@` trigger para mention picker. Solo en Ready, para no perder `@`
        // durante streaming, y siempre que haya documentos cargados.
        if c == '@'
            && self.state.chat.status == ChatStatus::Ready
            && self.state.chat.mode != crate::state::ChatMode::PlanReview
        {
            let docs = &self.state.dashboard.sidebar.all_docs;
            if !docs.is_empty() {
                self.state.chat.mention_picker.open(docs);
                return;
            }
        }
        // Cost panel toggle
        if c == '$' {
            self.state.panels.show_cost_panel = !self.state.panels.show_cost_panel;
            return;
        }
        // Tool details toggle
        if c == 't'
            && self.state.chat.status == ChatStatus::Ready
            && self.state.chat.input.is_empty()
            && self.state.chat.mode != crate::state::ChatMode::PlanReview
        {
            self.state.chat.tools_expanded = !self.state.chat.tools_expanded;
            // Invalidate cached lines so tool rendering updates
            for msg in &mut self.state.chat.messages {
                if !msg.tool_calls.is_empty() {
                    msg.invalidate_cache();
                }
            }
            return;
        }
        // Message navigator jump + toggle expanded (input vacío + Ready).
        if self.state.chat.status == ChatStatus::Ready
            && self.state.chat.input.is_empty()
            && self.state.chat.mode != crate::state::ChatMode::PlanReview
        {
            match c {
                '[' => {
                    self.state.chat.nav_move(-1);
                    return;
                }
                ']' => {
                    self.state.chat.nav_move(1);
                    return;
                }
                'm' => {
                    self.state.chat.nav_expanded = !self.state.chat.nav_expanded;
                    return;
                }
                _ => {}
            }
        }
        // Handle plan review actions
        if self.state.chat.mode == crate::state::ChatMode::PlanReview
            && self.state.chat.status == ChatStatus::Ready
        {
            match c {
                'a' | 'A' => {
                    let _ = self.tx.try_send(crate::actions::Action::PlanApprove);
                }
                'e' | 'E' => {
                    let _ = self.tx.try_send(crate::actions::Action::PlanEdit);
                }
                'r' | 'R' => {
                    let _ = self.tx.try_send(crate::actions::Action::PlanReject);
                }
                _ => {}
            }
            return;
        }
        // Handle tool approval prompt (P1.3 modal multi-tool).
        // Keys:
        //   y  → approve chosen (selected set or cursor item)
        //   n  → deny chosen
        //   Y  → approve ALL pending
        //   N  → deny ALL pending
        //   Space → toggle selection of cursor item
        //   a  → always-allow tool del cursor (persist)
        //   d  → always-deny tool del cursor (persist)
        if self.state.chat.status == ChatStatus::ExecutingTools
            && !self.state.chat.pending_approvals.is_empty()
        {
            match c {
                'y' => self.approve_chosen_tools(),
                'Y' => self.approve_pending_tools(),
                'n' => self.deny_chosen_tools(),
                'N' => self.deny_pending_tools(),
                ' ' => self.approval_toggle_selection(),
                'a' => {
                    let idx = self.state.chat.approval_cursor;
                    let name = self
                        .state
                        .chat
                        .pending_approvals
                        .get(idx)
                        .map(|p| p.tool_name.clone())
                        .unwrap_or_default();
                    let _ = self.tx.try_send(crate::actions::Action::AlwaysAllowTool(name));
                }
                'd' => {
                    let idx = self.state.chat.approval_cursor;
                    let name = self
                        .state
                        .chat
                        .pending_approvals
                        .get(idx)
                        .map(|p| p.tool_name.clone())
                        .unwrap_or_default();
                    let _ = self.tx.try_send(crate::actions::Action::AlwaysDenyTool(name));
                }
                _ => {}
            }
            return;
        }
        if self.state.chat.status == ChatStatus::Ready {
            // E40: grabamos el estado previo para Ctrl+Z. `record_if_changed`
            // evita duplicar snapshots cuando se pulsan rafagas sin cambios.
            let current = self.state.chat.input.clone();
            self.state.chat.input_undo.record_if_changed(&current);
            self.state.chat.input.push(c);
            self.update_slash_autocomplete();
        } else if matches!(
            self.state.chat.status,
            ChatStatus::Streaming | ChatStatus::ExecutingTools
        ) {
            self.state.chat.message_queue.push_draft_char(c);
        } else if matches!(self.state.chat.status, ChatStatus::Error(_)) {
            self.state.chat.status = ChatStatus::Ready;
            let current = self.state.chat.input.clone();
            self.state.chat.input_undo.record_if_changed(&current);
            self.state.chat.input.push(c);
            self.update_slash_autocomplete();
        }
    }

    /// Handle Up key when on the Chat screen.
    pub(super) fn on_up_chat(&mut self) {
        if self.state.chat.mention_picker.visible {
            self.state.chat.mention_picker.move_up();
            return;
        }
        if self.state.chat.doc_picker.visible {
            self.state.chat.doc_picker.move_up();
            return;
        }
        if self.state.chat.slash_autocomplete.visible {
            self.state.chat.slash_autocomplete.move_up();
            return;
        }
        if self.state.chat.status == ChatStatus::ExecutingTools
            && !self.state.chat.pending_approvals.is_empty()
        {
            self.approval_cursor_move(-1);
            return;
        }
        if self.state.chat.status == ChatStatus::Ready && !self.state.chat.input.is_empty() {
            self.state.chat.history_up();
        } else {
            self.state.chat.scroll_up(3);
            if self.state.chat.status == ChatStatus::Streaming {
                self.state.chat.scroll_anchor_tick = Some(self.state.tick_count);
            }
        }
    }

    /// Handle Down key when on the Chat screen.
    pub(super) fn on_down_chat(&mut self) {
        if self.state.chat.mention_picker.visible {
            self.state.chat.mention_picker.move_down();
            return;
        }
        if self.state.chat.doc_picker.visible {
            self.state.chat.doc_picker.move_down();
            return;
        }
        if self.state.chat.slash_autocomplete.visible {
            self.state.chat.slash_autocomplete.move_down();
            return;
        }
        if self.state.chat.status == ChatStatus::ExecutingTools
            && !self.state.chat.pending_approvals.is_empty()
        {
            self.approval_cursor_move(1);
            return;
        }
        if self.state.chat.status == ChatStatus::Ready && !self.state.chat.input.is_empty() {
            self.state.chat.history_down();
        } else {
            self.state.chat.scroll_down(3);
        }
    }

    /// Update slash autocomplete state based on current input.
    pub(super) fn update_slash_autocomplete(&mut self) {
        let input = &self.state.chat.input;
        if input.starts_with('/') && !input.contains(' ') {
            self.state.chat.slash_autocomplete.update(input);
        } else {
            self.state.chat.slash_autocomplete.close();
        }
        // Clear selected skill if input no longer matches the skill prefix
        if let Some(ref skill) = self.state.chat.selected_skill {
            let prefix = format!("/{}", skill.name);
            if !self.state.chat.input.starts_with(&prefix) {
                self.state.chat.selected_skill = None;
            }
        }
    }

    /// Handle Tab in the Chat screen. Delegates to slash-command completion
    /// when the autocomplete popup is visible. Returns true if Tab was consumed.
    pub(super) fn on_tab_chat(&mut self) -> bool {
        if self.state.chat.status != ChatStatus::Ready {
            return false;
        }
        if !self.state.chat.slash_autocomplete.visible {
            return false;
        }
        let filtered_len = self.state.chat.slash_autocomplete.filtered.len();
        if filtered_len == 0 {
            return false;
        }
        // Unique match: complete as if Enter.
        if filtered_len == 1 {
            self.complete_slash_command();
            return true;
        }
        // Multiple matches: try to extend to the longest common prefix first.
        let current = self.state.chat.input.clone();
        if let Some(prefix) = self.state.chat.slash_autocomplete.common_prefix() {
            if prefix.len() > current.len() {
                self.state.chat.input = prefix;
                self.update_slash_autocomplete();
                return true;
            }
        }
        // Already at the common prefix: cycle through candidates.
        self.state.chat.slash_autocomplete.move_down();
        true
    }

    /// Complete the input with the selected slash command.
    pub(super) fn complete_slash_command(&mut self) {
        if let Some(cmd) = self.state.chat.slash_autocomplete.selected_command() {
            self.state.chat.slash_autocomplete.close();
            // Commands that open pickers or have no args: execute immediately
            match cmd {
                "/clear" | "/exit" | "/help" | "/costs" | "/plan" | "/compact" | "/memory"
                | "/blocks" | "/diff" | "/files" | "/history" | "/resume" | "/todos"
                | "/todo-clear" | "/undo" | "/redo" => {
                    self.state.chat.input = cmd.to_string();
                    self.send_chat_message();
                }
                _ => {
                    // Commands that need args (e.g. /workflow, /model, /go, /load)
                    // or workflow shortcuts: just complete the text + space
                    self.state.chat.input = format!("{cmd} ");
                }
            }
        }
    }
}
