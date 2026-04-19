use crate::state::{ChatMessage, ChatRole, ChatStatus};

use super::App;

impl App {
    pub(crate) fn start_chat(&mut self) {
        let user_text = self.state.input.trim().to_string();
        self.state.input.clear();

        self.state.chat = crate::state::ChatState::new();
        self.state.chat.messages.push(ChatMessage::new(ChatRole::User, user_text));
        self.state.chat.status = ChatStatus::LoadingContext;
        self.state.screen = crate::state::AppScreen::Chat;

        // E40: restaurar draft si existe para esta sesion (best-effort).
        self.try_restore_draft();

        // E25: auto-start LSP client si no esta corriendo.
        self.try_start_lsp();
        // E39: primer chat enviado (y server respondio previamente).
        self.mark_onboarding_step(crate::services::onboarding::ChecklistStep::FirstChat);
        self.spawn_chat_context();
    }

    pub(crate) fn send_chat_message(&mut self) {
        let text = self.state.chat.input.trim().to_string();
        self.state.chat.push_to_history();
        self.state.chat.input.clear();
        // E40: al enviar se descarta el draft persistido y se limpia el undo stack
        // del buffer anterior (el undo de un mensaje enviado no tiene sentido).
        self.state.chat.input_undo.clear();
        // Un nuevo turn invalida cualquier /redo pendiente: restaurar un turn
        // borrado generaria inconsistencias (ids duplicados, refs rotas a
        // tool_call_ids). Si el user queria rehacer, debio hacerlo antes.
        self.state.chat.undo_redo_stack.clear();
        self.clear_persisted_draft();

        if text.starts_with('/') {
            self.state.chat.pasted_refs.clear();
            self.state.chat.next_paste_id = 1;
            self.handle_slash_command(&text);
            return;
        }

        // Expand paste placeholders before sending
        let text = if self.state.chat.pasted_refs.is_empty() {
            text
        } else {
            let expanded = crate::services::paste_handler::expand_placeholders(
                &text,
                &self.state.chat.pasted_refs,
            );
            self.state.chat.pasted_refs.clear();
            self.state.chat.next_paste_id = 1;
            expanded
        };

        // P2.4: detecta @mentions y delega a resolver async si hay alguna.
        // Sin mentions → flujo clásico síncrono.
        let mentions = crate::services::mentions::parse_mentions(&text);
        if mentions.is_empty() {
            self.push_user_message_and_send(text, Vec::new());
        } else {
            self.spawn_mention_resolution(text, mentions);
        }
    }

    /// P2.4: arranca la resolución async de @mentions. Cambia el status a
    /// `LoadingContext` (el spinner del header) mientras llegan los docs del
    /// MCP, y dispatchea `ChatUserMessageResolved` al terminar.
    fn spawn_mention_resolution(
        &mut self,
        text: String,
        mentions: Vec<crate::services::mentions::ParsedMention>,
    ) {
        let count = mentions.len();
        self.state.chat.status = ChatStatus::LoadingContext;
        self.notify(format!("Resolviendo {count} @mention(s)..."));
        let pool = self.mcp_pool.clone();
        let factory = self.state.factory.api_key().unwrap_or("all").to_string();
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let resolved =
                crate::services::mentions::resolve_prompt(&pool, &factory, &text, &mentions).await;
            let _ = tx
                .send(crate::actions::Action::ChatUserMessageResolved {
                    augmented_text: resolved.augmented_text,
                    refs: resolved.refs,
                })
                .await;
        });
    }

    /// Handler de `ChatUserMessageResolved`: empuja el user message con el
    /// texto ya augmentado + refs y arranca la completion del provider.
    pub(crate) fn handle_chat_user_message_resolved(
        &mut self,
        augmented_text: String,
        refs: Vec<crate::state::DocReference>,
    ) {
        self.push_user_message_and_send(augmented_text, refs);
    }

    /// Helper: empuja el user message (con context_refs opcional) y arranca
    /// el streaming del provider. Compartido entre flujo sin/con mentions.
    fn push_user_message_and_send(&mut self, text: String, refs: Vec<crate::state::DocReference>) {
        let mut msg = ChatMessage::new(ChatRole::User, text);
        msg.context_refs = refs;
        self.state.chat.messages.push(msg);
        self.state.chat.status = ChatStatus::Streaming;
        self.state.chat.snap_to_bottom();
        self.state.chat.tool_rounds = 0;
        self.state.chat.metrics.on_turn_start();
        self.mark_onboarding_step(crate::services::onboarding::ChecklistStep::FirstChat);
        self.spawn_chat_completion();
    }

    pub(crate) fn open_doc_picker(&mut self, doc_type: &str, label: &str) {
        let all = &self.state.dashboard.sidebar.all_docs;
        if all.is_empty() {
            // Trigger async document load; open picker when docs arrive
            if !self.state.dashboard.sidebar.loading {
                self.spawn_load_documents();
                self.state.dashboard.sidebar.loading = true;
            }
            self.state.pending_picker = Some((doc_type.to_string(), label.to_string()));
            self.notify("Cargando documentos...".to_string());
            return;
        }
        let factory_key = self.state.factory.filter_key();
        self.state.chat.doc_picker =
            crate::state::DocPickerState::open(doc_type, label, all, factory_key);
        let found = self.state.chat.doc_picker.items.len();
        if found == 0 {
            self.state.chat.doc_picker.close();
            self.notify(format!("No hay {label} para la factory activa"));
        } else {
            tracing::info!(doc_type, found, "DocPicker opened");
        }
    }

    /// Select a document from the picker.
    /// For skills/workflows: store as selected and let user type their request.
    /// For other doc types (adrs, policies, etc.): inject content immediately.
    pub(crate) fn select_doc_picker_item(&mut self) {
        let doc = match self.state.chat.doc_picker.selected() {
            Some(d) => d.clone(),
            None => return,
        };
        let doc_type = self.state.chat.doc_picker.doc_type.clone();
        self.state.chat.doc_picker.close();

        if doc_type == "skill" || doc_type == "workflow" {
            // Store as selected skill — user types context, then Enter sends
            self.state.chat.selected_skill =
                Some(crate::state::SelectedSkill { name: doc.name.clone() });
            self.state.chat.input = format!("/{} ", doc.name);
            self.notify(format!("/{} — escribe tu peticion y presiona Enter", doc.name));
        } else {
            // ADRs, policies, agents, commands: inject content directly
            self.spawn_fetch_doc_for_chat(doc.doc_type, doc.factory, doc.name);
        }
    }

    pub(crate) fn inject_git_diff(&mut self) {
        let dir = std::env::current_dir().unwrap_or_default();
        if let Some(diff) = crate::services::context::git_diff_only(&dir) {
            let msg = format!("```diff\n{diff}\n```");
            self.state.chat.messages.push(ChatMessage::new(ChatRole::User, msg));
            self.state.chat.scroll_offset = u16::MAX;
            self.notify("Git diff inyectado".to_string());
        } else {
            self.notify("No hay cambios sin commitear".to_string());
        }
    }

    pub(crate) fn inject_recent_files(&mut self) {
        let dir = std::env::current_dir().unwrap_or_default();
        let ctx = crate::services::context::collect(&dir);
        if ctx.recent_files.is_empty() {
            self.notify("No se encontraron archivos recientes".to_string());
            return;
        }
        let mut msg = String::from("Archivos modificados recientemente:\n");
        for f in &ctx.recent_files {
            msg.push_str(&format!("- `{f}`\n"));
        }
        self.state.chat.messages.push(ChatMessage::new(ChatRole::User, msg));
        self.state.chat.scroll_offset = u16::MAX;
        self.notify(format!("{} archivos inyectados", ctx.recent_files.len()));
    }

    pub(crate) fn handle_chat_context_loaded(&mut self, system_msgs: Vec<ChatMessage>) {
        let mut all = system_msgs;
        all.append(&mut self.state.chat.messages);
        self.state.chat.messages = all;
        self.state.chat.context_loaded = true;
        self.state.chat.status = ChatStatus::Streaming;
        self.state.chat.scroll_offset = u16::MAX;
        // E34: el primer turno arranca aqui cuando se viene del splash.
        self.state.chat.metrics.on_turn_start();
        self.spawn_chat_completion();
    }
}
