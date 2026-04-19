//! Handler del overlay de transcript (E33).
//!
//! El transcript es un snapshot read-only de toda la conversacion activado
//! con Ctrl+O. Soporta busqueda literal case-insensitive via Ctrl+F + n/N.
//! Mientras esta activo intercepta las teclas principales antes de los
//! handlers de chat.

use crate::state::AppScreen;

use super::App;

impl App {
    /// Dispatcher del Action `ToggleTranscript` (Ctrl+O). Solo efectivo en la
    /// pantalla Chat — en otras screens es no-op para evitar atajos "fantasma".
    pub(crate) fn handle_toggle_transcript(&mut self) {
        if self.state.screen != AppScreen::Chat {
            return;
        }
        if self.state.chat.transcript.active {
            self.state.chat.transcript.close();
            self.notify("Transcript cerrado".to_string());
        } else {
            self.state.chat.transcript.open();
            self.notify("Transcript: / buscar, n/N navegar, Esc salir".to_string());
        }
    }

    /// Intercepta Esc cuando el transcript esta activo. Retorna `true` si
    /// consumio el evento (los handlers generales deben abortar).
    pub(crate) fn on_esc_transcript(&mut self) -> bool {
        if !self.state.chat.transcript.active {
            return false;
        }
        if self.state.chat.transcript.search_active {
            self.state.chat.transcript.exit_search();
        } else {
            self.state.chat.transcript.close();
        }
        true
    }

    /// Intercepta caracteres cuando el transcript esta activo. Maneja `n`/`N`
    /// para navegar matches y acumula la query cuando `search_active`.
    pub(crate) fn on_char_transcript(&mut self, c: char) -> bool {
        if !self.state.chat.transcript.active {
            return false;
        }
        if self.state.chat.transcript.search_active {
            self.state.chat.transcript.query.push(c);
            self.state.chat.transcript.match_cursor = 0;
            return true;
        }
        match c {
            '/' => self.state.chat.transcript.enter_search(),
            'n' => self.state.chat.transcript.next_match(),
            'N' => self.state.chat.transcript.prev_match(),
            'q' => self.state.chat.transcript.close(),
            _ => {}
        }
        true
    }

    /// Backspace dentro del prompt de busqueda del transcript.
    pub(crate) fn on_backspace_transcript(&mut self) -> bool {
        if !self.state.chat.transcript.active || !self.state.chat.transcript.search_active {
            return false;
        }
        self.state.chat.transcript.query.pop();
        self.state.chat.transcript.match_cursor = 0;
        true
    }

    /// Enter cierra el prompt de busqueda conservando la query. Si no esta en
    /// busqueda, no-op (el transcript es read-only, Enter no envia nada).
    pub(crate) fn on_enter_transcript(&mut self) -> bool {
        if !self.state.chat.transcript.active {
            return false;
        }
        if self.state.chat.transcript.search_active {
            self.state.chat.transcript.exit_search();
        }
        true
    }

    /// Scroll del transcript via flechas. Devuelve `true` si consumio el evento.
    pub(crate) fn on_up_transcript(&mut self) -> bool {
        if !self.state.chat.transcript.active {
            return false;
        }
        self.state.chat.transcript.scroll_offset =
            self.state.chat.transcript.scroll_offset.saturating_sub(1);
        true
    }

    pub(crate) fn on_down_transcript(&mut self) -> bool {
        if !self.state.chat.transcript.active {
            return false;
        }
        self.state.chat.transcript.scroll_offset =
            self.state.chat.transcript.scroll_offset.saturating_add(1);
        true
    }
}

#[cfg(test)]
mod tests {
    use crate::state::TranscriptView;

    fn make_view(active: bool, search_active: bool) -> TranscriptView {
        TranscriptView { active, search_active, ..Default::default() }
    }

    #[test]
    fn next_match_is_noop_when_transcript_closed() {
        let mut t = make_view(false, false);
        t.next_match();
        assert_eq!(t.match_cursor, 0);
    }

    #[test]
    fn query_accumulates_chars_in_search_mode() {
        let mut t = make_view(true, true);
        t.query.push('f');
        t.query.push('o');
        assert_eq!(t.query, "fo");
    }
}
