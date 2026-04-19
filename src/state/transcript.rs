//! Transcript view state (E33).
//!
//! Modal read-only activado con Ctrl+O que muestra la conversacion completa
//! — incluyendo mensajes `System` y tool results expandidos — con busqueda
//! literal case-insensitive. No muta los mensajes del chat: renderiza sobre
//! snapshots en cada frame.

/// Estado del overlay de transcript.
#[derive(Debug, Clone, Default)]
pub struct TranscriptView {
    /// `true` mientras el overlay esta visible.
    pub active: bool,
    /// `true` cuando el usuario esta capturando la query (Ctrl+F).
    pub search_active: bool,
    /// Query literal (case-insensitive).
    pub query: String,
    /// Posicion de scroll (en lineas del transcript).
    pub scroll_offset: u16,
    /// Indice dentro de `match_positions` seleccionado actualmente.
    pub match_cursor: usize,
    /// Numero total de matches encontrados en el ultimo build del transcript.
    /// Lo popula el render con `record_matches`.
    pub match_count: usize,
}

impl TranscriptView {
    /// Abre el overlay reseteando la busqueda.
    pub fn open(&mut self) {
        self.active = true;
        self.search_active = false;
        self.scroll_offset = 0;
    }

    /// Cierra el overlay y limpia el estado.
    pub fn close(&mut self) {
        *self = Self::default();
    }

    /// Activa el prompt de busqueda (Ctrl+F dentro del transcript).
    pub fn enter_search(&mut self) {
        self.search_active = true;
        self.query.clear();
        self.match_cursor = 0;
    }

    /// Sale del prompt de busqueda conservando la query activa.
    pub fn exit_search(&mut self) {
        self.search_active = false;
    }

    /// Avanza al siguiente match (n). Wrap-around al final.
    pub fn next_match(&mut self) {
        if self.match_count == 0 {
            return;
        }
        self.match_cursor = (self.match_cursor + 1) % self.match_count;
    }

    /// Retrocede al match anterior (N). Wrap-around al inicio.
    pub fn prev_match(&mut self) {
        if self.match_count == 0 {
            return;
        }
        if self.match_cursor == 0 {
            self.match_cursor = self.match_count - 1;
        } else {
            self.match_cursor -= 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_resets_search_state() {
        let mut t = TranscriptView {
            active: false,
            search_active: true,
            query: "foo".into(),
            scroll_offset: 10,
            match_cursor: 2,
            match_count: 5,
        };
        t.open();
        assert!(t.active);
        assert!(!t.search_active);
        assert_eq!(t.scroll_offset, 0);
    }

    #[test]
    fn close_clears_everything() {
        let mut t = TranscriptView { active: true, query: "x".into(), ..Default::default() };
        t.close();
        assert!(!t.active);
        assert!(t.query.is_empty());
    }

    #[test]
    fn next_match_wraps() {
        let mut t = TranscriptView { match_count: 3, match_cursor: 2, ..Default::default() };
        t.next_match();
        assert_eq!(t.match_cursor, 0);
    }

    #[test]
    fn prev_match_wraps() {
        let mut t = TranscriptView { match_count: 3, match_cursor: 0, ..Default::default() };
        t.prev_match();
        assert_eq!(t.match_cursor, 2);
    }

    #[test]
    fn next_match_is_noop_when_empty() {
        let mut t = TranscriptView::default();
        t.next_match();
        assert_eq!(t.match_cursor, 0);
    }
}
