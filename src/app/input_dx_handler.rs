//! Input DX handlers (E40): Ctrl+Z / Ctrl+Y + auto-save de draft por tick.

use crate::services::draft_store;
use crate::state::{AppScreen, ChatStatus};

use super::App;

/// Cada cuantos ticks del loop principal corremos el auto-save.
/// El tick dispara cada 250ms (4Hz), asi que 8 ticks ≈ 2s — el criterio del
/// roadmap (E40: "auto-save cada 2s a session store").
pub const DRAFT_AUTOSAVE_TICKS: u64 = 8;

impl App {
    /// `Ctrl+Z` — revierte el ultimo cambio del input del chat.
    pub(crate) fn handle_input_undo(&mut self) {
        if self.state.screen != AppScreen::Chat {
            return;
        }
        if self.state.chat.status != ChatStatus::Ready {
            return;
        }
        let current = self.state.chat.input.clone();
        match self.state.chat.input_undo.undo(&current) {
            Some(prev) => {
                self.state.chat.input = prev;
                self.update_slash_autocomplete();
            }
            None => {
                self.notify("Nada que deshacer".to_string());
            }
        }
    }

    /// `Ctrl+Y` — rehace el ultimo cambio deshecho.
    pub(crate) fn handle_input_redo(&mut self) {
        if self.state.screen != AppScreen::Chat {
            return;
        }
        if self.state.chat.status != ChatStatus::Ready {
            return;
        }
        let current = self.state.chat.input.clone();
        match self.state.chat.input_undo.redo(&current) {
            Some(next) => {
                self.state.chat.input = next;
                self.update_slash_autocomplete();
            }
            None => {
                self.notify("Nada que rehacer".to_string());
            }
        }
    }

    /// Auto-save del draft. Solo escribe cuando el contenido difiere del
    /// ultimo snapshot persistido para evitar I/O innecesario.
    pub(crate) fn handle_draft_auto_save(&mut self) {
        if self.state.screen != AppScreen::Chat {
            return;
        }
        if self.state.chat.input == self.state.chat.persisted_draft {
            return;
        }
        let session_id = self.state.chat.session_id.clone();
        let content = self.state.chat.input.clone();
        if let Err(e) = draft_store::save_draft(&session_id, &content) {
            tracing::warn!(error = %e, session = %session_id, "draft_store: save failed");
            return;
        }
        self.state.chat.persisted_draft = content;
    }

    /// Intenta restaurar el draft guardado al entrar a una sesion. Solo opera
    /// si el input actual esta vacio (no pisar escritura activa).
    pub(crate) fn try_restore_draft(&mut self) {
        if !self.state.chat.input.is_empty() {
            return;
        }
        let session_id = self.state.chat.session_id.clone();
        if let Some(saved) = draft_store::load_draft(&session_id) {
            if saved.is_empty() {
                return;
            }
            let len = saved.len();
            self.state.chat.input = saved.clone();
            self.state.chat.persisted_draft = saved;
            self.notify(format!("↩ Draft restaurado ({len} chars)"));
        }
    }

    /// Descarta el draft tras envio exitoso.
    pub(crate) fn clear_persisted_draft(&mut self) {
        let session_id = self.state.chat.session_id.clone();
        if let Err(e) = draft_store::clear_draft(&session_id) {
            tracing::warn!(error = %e, session = %session_id, "draft_store: clear failed");
        }
        self.state.chat.persisted_draft.clear();
    }
}
