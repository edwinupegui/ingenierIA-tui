//! Persistencia del chat (E11): JSONL append-only + fallback a legacy `.json`.
//!
//! `services/session/` (nuevo) maneja el JSONL crash-safe. Los helpers puros
//! de conversion viven en `app/history_bridge.rs`. Este archivo solo
//! contiene los handlers de comando (`&mut self`) del trait `App`.

use crate::services::history::{HistoryEntry, SavedConversation};
use crate::services::session::{self, SessionMeta, TimedEntry};
use crate::state::{ChatMessage, ChatRole, ChatStatus};

use super::history_bridge::{list_history_merged, load_most_recent_any, message_to_timed_entry};
use super::App;

impl App {
    /// Auto-save: append de `messages[persisted..]` al JSONL + actualiza meta.
    /// Backward-compatible: no reescribe archivos `.json` legacy.
    pub(crate) fn auto_save_history(&mut self) {
        let total = self.state.chat.messages.len();
        if total < 2 {
            return; // no vale la pena persistir intercambios vacios
        }

        // Mensajes nuevos desde el ultimo save.
        let start = self.state.chat.persisted_msg_count.min(total);
        let new_entries: Vec<TimedEntry> =
            self.state.chat.messages[start..total].iter().map(message_to_timed_entry).collect();

        let session_id = self.state.chat.session_id.clone();
        let meta = self.build_meta(total);
        let append_entries = new_entries;

        tokio::spawn(async move {
            for entry in &append_entries {
                if let Err(e) = session::append_entry(&session_id, entry) {
                    tracing::warn!(error = %e, "failed to append session entry");
                    return;
                }
            }
            if let Some(path) = session::meta_path(&session_id) {
                if let Err(e) = meta.save(&path) {
                    tracing::warn!(error = %e, "failed to save session meta sidecar");
                }
            }
        });

        self.state.chat.persisted_msg_count = total;
    }

    fn build_meta(&self, message_count: usize) -> SessionMeta {
        let title = self
            .state
            .chat
            .messages
            .iter()
            .find(|m| m.role == ChatRole::User)
            .map(|m| session::title_from_content(&m.content))
            .unwrap_or_else(|| "Sin titulo".to_string());
        let turn_count =
            self.state.chat.messages.iter().filter(|m| m.role == ChatRole::User).count();
        let cost = &self.state.chat.cost;
        SessionMeta {
            id: self.state.chat.session_id.clone(),
            title,
            factory: self.state.factory.label().to_string(),
            model: self.state.model.clone(),
            provider: self.state.provider.clone(),
            created_at: crate::services::sync::now_iso(),
            updated_at: crate::services::sync::now_iso(),
            parent_id: None,
            fork_label: None,
            turn_count,
            message_count,
            total_input_tokens: cost.total_input,
            total_output_tokens: cost.total_output,
            total_cost: cost.total_cost(),
            mode: self.state.chat.mode.as_str().to_string(),
        }
    }

    /// Handler del resultado de `/history` — pinta la lista en el chat.
    pub(crate) fn handle_history_list(&mut self, entries: Vec<HistoryEntry>) {
        if entries.is_empty() {
            self.notify("No hay conversaciones guardadas".to_string());
            return;
        }

        let mut msg = String::from("## Sesiones guardadas\n\n");
        for (i, entry) in entries.iter().take(20).enumerate() {
            let date = entry.created_at.get(..10).unwrap_or(&entry.created_at);
            let turns = if entry.turn_count > 0 {
                format!("{} turns", entry.turn_count)
            } else {
                format!("{} msgs", entry.message_count)
            };
            let cost_str = if entry.total_cost > 0.0 {
                if entry.total_cost < 0.01 {
                    format!(" · ${:.4}", entry.total_cost)
                } else {
                    format!(" · ${:.2}", entry.total_cost)
                }
            } else {
                String::new()
            };
            msg.push_str(&format!(
                "{}. **{}** — {} · {} · {}{}\n",
                i + 1,
                entry.title,
                entry.factory,
                date,
                turns,
                cost_str,
            ));
        }
        msg.push_str("\n`/load <n>` cargar · `/resume` continuar la mas reciente · `/fork <label>` ramificar");

        let content = msg.clone();
        let cached = crate::ui::widgets::markdown::render_markdown(
            &content,
            &self.state.active_theme.colors(),
        );
        let mut chat_msg = ChatMessage::new(ChatRole::Assistant, content);
        chat_msg.cached_lines = Some(std::sync::Arc::new(cached));
        self.state.chat.messages.push(chat_msg);
        self.state.chat.scroll_offset = u16::MAX;
    }

    /// `/costs` — resumen de costos (agrega tanto sesiones JSONL como legacy).
    pub(crate) fn handle_costs_command(&mut self) {
        let entries = list_history_merged();
        let total_cost: f64 = entries.iter().map(|e| e.total_cost).sum();
        let total_input: u64 = entries.iter().map(|e| e.total_input_tokens as u64).sum();
        let total_output: u64 = entries.iter().map(|e| e.total_output_tokens as u64).sum();
        let sessions_with_cost = entries.iter().filter(|e| e.total_cost > 0.0).count();

        let current = &self.state.chat.cost;
        let msg = format!(
            "## Resumen de costos\n\n\
             ```\n\
             Sesion actual\n\
             ─────────────────────────\n\
             Input:    {:>8} tok\n\
             Output:   {:>8} tok\n\
             Costo:    {:>10}\n\n\
             Historial ({} sesiones con costo)\n\
             ─────────────────────────\n\
             Input:    {:>8} tok\n\
             Output:   {:>8} tok\n\
             Total:    {:>10}\n\
             ```",
            current.total_input,
            current.total_output,
            current.cost_display(),
            sessions_with_cost,
            total_input,
            total_output,
            if total_cost < 0.01 {
                format!("${:.4}", total_cost)
            } else {
                format!("${:.2}", total_cost)
            },
        );

        let cached =
            crate::ui::widgets::markdown::render_markdown(&msg, &self.state.active_theme.colors());
        let mut chat_msg = ChatMessage::new(ChatRole::Assistant, msg);
        chat_msg.cached_lines = Some(std::sync::Arc::new(cached));
        self.state.chat.messages.push(chat_msg);
        self.state.chat.scroll_offset = u16::MAX;
    }

    /// `/resume` — carga la sesion mas reciente (JSONL o legacy).
    pub(crate) fn handle_resume_command(&mut self) {
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let conv = tokio::task::spawn_blocking(load_most_recent_any).await.ok().flatten();
            if let Some(conv) = conv {
                let _ = tx.send(crate::actions::Action::HistoryLoaded(conv)).await;
            }
        });
    }

    /// `/history` y `/sessions` — lista enriquecida con metas.
    pub(crate) fn handle_sessions_command(&mut self) {
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let entries =
                tokio::task::spawn_blocking(list_history_merged).await.unwrap_or_default();
            let _ = tx.send(crate::actions::Action::HistoryList(entries)).await;
        });
    }

    /// Handler tras cargar una conversacion. Marca `persisted_msg_count` para
    /// que siguientes auto_save solo agreguen lo nuevo.
    pub(crate) fn handle_history_loaded(&mut self, conv: SavedConversation) {
        self.state.chat = crate::state::ChatState::new();
        self.state.chat.session_id = conv.id;
        self.state.chat.status = ChatStatus::Ready;
        self.state.chat.context_loaded = true;
        self.state.chat.mode = crate::state::ChatMode::from_str_lossy(&conv.mode);
        self.state.chat.cost.total_input = conv.total_input_tokens;
        self.state.chat.cost.total_output = conv.total_output_tokens;

        for msg in conv.messages {
            let role = match msg.role.as_str() {
                "system" => ChatRole::System,
                "user" => ChatRole::User,
                "assistant" => ChatRole::Assistant,
                "tool" => ChatRole::Tool,
                _ => continue,
            };
            let mut chat_msg = ChatMessage::new(role, msg.content);
            for tc in msg.tool_calls {
                chat_msg.tool_calls.push(crate::state::ToolCall {
                    id: tc.id,
                    name: tc.name,
                    arguments: tc.arguments,
                    status: crate::state::ToolCallStatus::Success,
                    duration_ms: None,
                });
            }
            chat_msg.tool_call_id = msg.tool_call_id;
            if chat_msg.role == ChatRole::Assistant && !chat_msg.content.is_empty() {
                chat_msg.cached_lines =
                    Some(std::sync::Arc::new(crate::ui::widgets::markdown::render_markdown(
                        &chat_msg.content,
                        &self.state.active_theme.colors(),
                    )));
            }
            self.state.chat.messages.push(chat_msg);
        }

        // Parchea tool_use huerfanos (E31 synthetic results).
        let patched = crate::services::chat::synthetic_results::patch_orphaned_tool_uses(
            &mut self.state.chat.messages,
        );
        if patched > 0 {
            self.notify_level(
                &format!("⚠ {patched} tool_use huerfanos parcheados con sintetico"),
                crate::state::ToastLevel::Warning,
            );
        }

        // Todo lo cargado ya esta en JSONL (o en legacy JSON). Marcar persistido.
        self.state.chat.persisted_msg_count = self.state.chat.messages.len();

        self.state.screen = crate::state::AppScreen::Chat;
        self.state.chat.scroll_offset = u16::MAX;
        self.notify(format!("✓ Conversacion restaurada: {}", conv.title));
    }

    /// `/fork <label>` — duplica la sesion actual con un label humano.
    pub(crate) fn handle_fork_command(&mut self, arg: &str) {
        let label = arg.trim();
        if label.is_empty() {
            self.notify("Uso: /fork <label>".to_string());
            return;
        }
        if self.state.chat.messages.len() < 2 {
            self.notify("No hay conversacion suficiente para forkear".to_string());
            return;
        }

        let parent_id = self.state.chat.session_id.clone();
        let new_id = session::generate_session_id();
        let label_owned = label.to_string();
        let parent_meta = session::meta_path(&parent_id).and_then(|p| SessionMeta::load(&p));

        match session::fork_session(
            &parent_id,
            new_id.clone(),
            label_owned.clone(),
            parent_meta.as_ref(),
        ) {
            Ok(info) => {
                self.state.chat.session_id = info.child_id.clone();
                self.state.chat.persisted_msg_count = self.state.chat.messages.len();
                self.notify(format!("✓ Fork '{}' creado ({})", info.label, info.child_id));
            }
            Err(e) => {
                self.notify(format!("✗ Error en fork: {e}"));
            }
        }
    }

    /// `/undo` — deshace el ultimo turn. Pop el ultimo `ChatRole::User` y
    /// todo lo que vino despues (assistant + tool messages). Si el draft de
    /// input esta vacio, restaura el contenido del user message como draft
    /// para facilitar edicion + re-envio. No opera durante streaming.
    pub(crate) fn handle_undo_command(&mut self) {
        use crate::state::ChatStatus;
        if !matches!(self.state.chat.status, ChatStatus::Ready) {
            self.notify("No se puede /undo durante streaming/herramientas".to_string());
            return;
        }
        let Some(last_user_idx) =
            self.state.chat.messages.iter().rposition(|m| m.role == ChatRole::User)
        else {
            self.notify("No hay turnos previos para deshacer".to_string());
            return;
        };
        let restored_content = self.state.chat.messages[last_user_idx].content.clone();
        let draft_before = self.state.chat.input.clone();
        let popped: Vec<ChatMessage> = self.state.chat.messages.drain(last_user_idx..).collect();
        // Guardar en stack para permitir /redo simetrico.
        self.state.chat.undo_redo_stack.push((popped, draft_before));

        // Invalidar campos efimeros atados al turn abortado.
        self.state.chat.tool_rounds = 0;
        self.state.chat.pending_approvals.clear();
        self.state.chat.code_blocks.clear();
        self.state.chat.snap_to_bottom();

        if self.state.chat.input.trim().is_empty() {
            self.state.chat.input = restored_content;
            self.notify("↶ Turn deshecho (draft restaurado, /redo disponible)".to_string());
        } else {
            self.notify("↶ Turn deshecho (/redo disponible, draft preservado)".to_string());
        }
    }

    /// `/redo` — rehace el ultimo `/undo`. Solo funciona si el stack tiene
    /// entries y no hubo envio de un nuevo user message entre el undo y el
    /// redo (el send limpia el stack para evitar restaurar turns stale).
    pub(crate) fn handle_redo_command(&mut self) {
        use crate::state::ChatStatus;
        if !matches!(self.state.chat.status, ChatStatus::Ready) {
            self.notify("No se puede /redo durante streaming/herramientas".to_string());
            return;
        }
        let Some((restored_messages, prior_draft)) = self.state.chat.undo_redo_stack.pop() else {
            self.notify("Nada para rehacer".to_string());
            return;
        };
        self.state.chat.messages.extend(restored_messages);
        self.state.chat.input = prior_draft;
        self.state.chat.snap_to_bottom();
        self.notify("↷ Turn rehecho".to_string());
    }

    /// `/export [path]` — escribe el JSONL completo a un archivo.
    pub(crate) fn handle_export_command(&mut self, arg: &str) {
        use crate::services::session::ExportFormat;
        let id = self.state.chat.session_id.clone();
        let arg = arg.trim();

        // Sintaxis aceptada: `/export`, `/export <format>`, `/export <format> <path>`,
        // o `/export <path>` (formato inferido por extension).
        let (format, dest): (ExportFormat, std::path::PathBuf) = {
            let mut parts = arg.splitn(2, char::is_whitespace);
            let first = parts.next().unwrap_or("").trim();
            let rest = parts.next().map(str::trim).unwrap_or("");

            let parsed_format = match first.to_ascii_lowercase().as_str() {
                "md" | "markdown" => Some(ExportFormat::Markdown),
                "csv" => Some(ExportFormat::Csv),
                "jsonl" | "json" => Some(ExportFormat::Jsonl),
                _ => None,
            };

            match (parsed_format, rest) {
                (Some(f), "") => {
                    let ext = match f {
                        ExportFormat::Markdown => "md",
                        ExportFormat::Csv => "csv",
                        ExportFormat::Jsonl => "jsonl",
                    };
                    (f, std::env::temp_dir().join(format!("ingenieria-session-{id}.{ext}")))
                }
                (Some(f), path) => (f, std::path::PathBuf::from(path)),
                (None, _) if first.is_empty() => (
                    ExportFormat::Jsonl,
                    std::env::temp_dir().join(format!("ingenieria-session-{id}.jsonl")),
                ),
                (None, _) => {
                    let p = std::path::PathBuf::from(arg);
                    (ExportFormat::from_path(&p), p)
                }
            }
        };

        match session::export_session_as(&id, &dest, format) {
            Ok(count) => {
                self.notify(format!("✓ {count} entries exportados a {}", dest.display()));
            }
            Err(e) => {
                self.notify(format!("✗ Error en export: {e}"));
            }
        }
    }
}
