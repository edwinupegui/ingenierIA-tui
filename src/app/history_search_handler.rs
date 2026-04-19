//! Handlers para el modal de busqueda de historial Ctrl+R (E30b).
//!
//! El modal vive en `state.chat.history_search`. Cuando es `Some`, las
//! teclas de chat son interceptadas para alimentar el query, navegar y
//! seleccionar/cerrar.

use crate::state::history_search::HistorySearch;
use crate::state::AppScreen;

use super::App;

impl App {
    /// `Action::InputHistorySearch` (Ctrl+R) — abre o cierra el modal.
    pub(crate) fn handle_input_history_search(&mut self) {
        if self.state.screen != AppScreen::Chat {
            return;
        }
        if self.state.chat.history_search.is_some() {
            self.state.chat.history_search = None;
            return;
        }
        let mut search = HistorySearch::new();
        search.recompute(&self.state.chat.input_history);
        self.state.chat.history_search = Some(search);
    }

    /// Devuelve `true` si el modal absorbio el caracter.
    pub(crate) fn on_char_history_search(&mut self, c: char) -> bool {
        if self.state.chat.history_search.is_none() {
            return false;
        }
        let history = self.state.chat.input_history.clone();
        if let Some(search) = self.state.chat.history_search.as_mut() {
            search.query.push(c);
            search.recompute(&history);
        }
        true
    }

    pub(crate) fn on_backspace_history_search(&mut self) -> bool {
        if self.state.chat.history_search.is_none() {
            return false;
        }
        let history = self.state.chat.input_history.clone();
        if let Some(search) = self.state.chat.history_search.as_mut() {
            search.query.pop();
            search.recompute(&history);
        }
        true
    }

    pub(crate) fn on_up_history_search(&mut self) -> bool {
        if let Some(search) = self.state.chat.history_search.as_mut() {
            search.move_up();
            true
        } else {
            false
        }
    }

    pub(crate) fn on_down_history_search(&mut self) -> bool {
        if let Some(search) = self.state.chat.history_search.as_mut() {
            search.move_down();
            true
        } else {
            false
        }
    }

    pub(crate) fn on_enter_history_search(&mut self) -> bool {
        let Some(search) = self.state.chat.history_search.as_ref() else {
            return false;
        };
        if let Some(idx) = search.selected_index() {
            if let Some(entry) = self.state.chat.input_history.get(idx).cloned() {
                self.state.chat.input = entry;
            }
        }
        self.state.chat.history_search = None;
        true
    }

    pub(crate) fn on_esc_history_search(&mut self) -> bool {
        if self.state.chat.history_search.is_some() {
            self.state.chat.history_search = None;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actions::Action;
    use crate::config::Config;
    use crate::services::IngenieriaClient;
    use crate::state::ChatStatus;
    use std::sync::Arc;

    fn test_app() -> App {
        let (tx, _rx) = tokio::sync::mpsc::channel::<Action>(8);
        let client = Arc::new(IngenieriaClient::new("http://localhost:3001"));
        let cfg = Config::resolve(None);
        let mut app = App::new(client, tx, cfg, false, false, false);
        app.state.screen = AppScreen::Chat;
        app.state.chat.status = ChatStatus::Ready;
        app.state.chat.input_history =
            vec!["/help".into(), "/cron-list".into(), "show diff".into()];
        app
    }

    #[test]
    fn ctrl_r_toggles_modal_open_and_closed() {
        let mut app = test_app();
        assert!(app.state.chat.history_search.is_none());
        app.handle_input_history_search();
        assert!(app.state.chat.history_search.is_some());
        app.handle_input_history_search();
        assert!(app.state.chat.history_search.is_none());
    }

    #[test]
    fn ctrl_r_only_works_on_chat_screen() {
        let mut app = test_app();
        app.state.screen = AppScreen::Splash;
        app.handle_input_history_search();
        assert!(app.state.chat.history_search.is_none());
    }

    #[test]
    fn typing_filters_query_and_recomputes() {
        let mut app = test_app();
        app.handle_input_history_search();
        assert!(app.on_char_history_search('c'));
        let s = app.state.chat.history_search.as_ref().unwrap();
        assert_eq!(s.query, "c");
        assert!(s.matches.iter().any(|(i, _)| *i == 1));
    }

    #[test]
    fn enter_applies_selection_and_closes() {
        let mut app = test_app();
        app.handle_input_history_search();
        // Arriba sin matches no rompe; solo seleccionamos el primero.
        assert!(app.on_enter_history_search());
        assert!(app.state.chat.history_search.is_none());
        // El input recibe el match (recientes-primero, primer = "show diff").
        assert_eq!(app.state.chat.input, "show diff");
    }

    #[test]
    fn esc_closes_without_modifying_input() {
        let mut app = test_app();
        app.state.chat.input = "preserved".into();
        app.handle_input_history_search();
        assert!(app.on_esc_history_search());
        assert_eq!(app.state.chat.input, "preserved");
    }
}
