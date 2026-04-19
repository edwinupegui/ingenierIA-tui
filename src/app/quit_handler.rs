//! Handler para Ctrl+C con confirmacion doble (patron codex-rs / opencode).
//!
//! El primer Ctrl+C arma la ventana de salida (2s) y notifica al usuario. Un
//! segundo Ctrl+C dentro de esa ventana sale del TUI. Si el stream o la
//! ejecucion de tools esta activa, el primer Ctrl+C aborta en su lugar y no
//! arma la salida — consistente con Esc pero disparable sin perder el draft.

use crate::{
    actions::Action,
    state::{AppScreen, ChatStatus},
};

use super::App;

/// Ventana de confirmacion en ticks (4Hz → 8 ticks = 2 segundos).
const QUIT_ARM_TICKS: u64 = 8;

impl App {
    /// Retorna `true` si Ctrl+C confirma la salida del TUI, `false` si fue
    /// consumido por un nivel de escalada (abort, clear input, o arm).
    pub(crate) fn on_ctrl_c(&mut self) -> bool {
        if let Some(expires) = self.state.quit_armed_until {
            if self.state.tick_count <= expires {
                return true;
            }
            self.state.quit_armed_until = None;
        }

        if self.state.screen == AppScreen::Chat
            && matches!(self.state.chat.status, ChatStatus::Streaming | ChatStatus::ExecutingTools)
        {
            let _ = self.tx.try_send(Action::ChatStreamAbort);
            self.notify("Turn abortado".to_string());
            return false;
        }

        if self.state.screen == AppScreen::Chat && !self.state.chat.input.is_empty() {
            self.state.chat.input.clear();
            return false;
        }

        if self.state.screen == AppScreen::Splash && !self.state.input.is_empty() {
            self.state.input.clear();
            return false;
        }

        self.state.quit_armed_until = Some(self.state.tick_count + QUIT_ARM_TICKS);
        self.notify("Ctrl+C otra vez para salir".to_string());
        false
    }
}
