//! Estado del modal de busqueda de historial de inputs (E30b — Ctrl+R).
//!
//! Reutiliza nucleo-matcher (mismo scoring que el command palette) para
//! permitir busqueda fuzzy sobre los ultimos N inputs del usuario. Vive en
//! `ChatState.history_search` como `Option<HistorySearch>` — el modal se
//! activa con Ctrl+R y se cierra con Esc / Enter.

use nucleo_matcher::{
    pattern::{CaseMatching, Normalization, Pattern},
    Matcher,
};

/// Limite superior de matches mostrados en el modal.
pub const HISTORY_SEARCH_LIMIT: usize = 20;

#[derive(Debug)]
pub struct HistorySearch {
    /// Texto de busqueda capturado por el usuario.
    pub query: String,
    /// Indices (sobre input_history) ordenados por score descendente.
    pub matches: Vec<(usize, u32)>,
    /// Cursor activo dentro de `matches`.
    pub cursor: usize,
}

impl HistorySearch {
    pub fn new() -> Self {
        Self { query: String::new(), matches: Vec::new(), cursor: 0 }
    }

    /// Recalcula la lista de matches contra `history` (la lista cronologica
    /// de inputs del usuario, mas vieja primero).
    pub fn recompute(&mut self, history: &[String]) {
        self.matches.clear();
        if history.is_empty() {
            self.cursor = 0;
            return;
        }
        if self.query.is_empty() {
            // Sin query: muestra los mas recientes primero.
            self.matches = history
                .iter()
                .enumerate()
                .rev()
                .take(HISTORY_SEARCH_LIMIT)
                .map(|(i, _)| (i, 0u32))
                .collect();
            self.cursor = 0;
            return;
        }
        let mut matcher = Matcher::new(nucleo_matcher::Config::DEFAULT);
        let pattern = Pattern::parse(&self.query, CaseMatching::Ignore, Normalization::Smart);
        let mut buf = Vec::new();
        let mut scored: Vec<(usize, u32)> = history
            .iter()
            .enumerate()
            .filter_map(|(i, entry)| {
                let needle = nucleo_matcher::Utf32Str::new(entry, &mut buf);
                pattern.score(needle, &mut matcher).map(|score| (i, score))
            })
            .collect();
        scored.sort_by(|a, b| b.1.cmp(&a.1));
        scored.truncate(HISTORY_SEARCH_LIMIT);
        self.matches = scored;
        if self.cursor >= self.matches.len() {
            self.cursor = self.matches.len().saturating_sub(1);
        }
    }

    pub fn move_up(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
    }

    pub fn move_down(&mut self) {
        if self.cursor + 1 < self.matches.len() {
            self.cursor += 1;
        }
    }

    /// Devuelve el indice en `input_history` del match seleccionado.
    pub fn selected_index(&self) -> Option<usize> {
        self.matches.get(self.cursor).map(|(i, _)| *i)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_history() -> Vec<String> {
        vec![
            "/help".to_string(),
            "/clear".to_string(),
            "/cron-list".to_string(),
            "show me the diff".to_string(),
            "explain hooks".to_string(),
        ]
    }

    #[test]
    fn empty_query_returns_recent_first() {
        let mut s = HistorySearch::new();
        s.recompute(&sample_history());
        assert_eq!(s.matches.first().map(|(i, _)| *i), Some(4));
        assert!(s.matches.len() <= HISTORY_SEARCH_LIMIT);
    }

    #[test]
    fn fuzzy_query_filters_matches() {
        let mut s = HistorySearch::new();
        s.query = "cron".into();
        s.recompute(&sample_history());
        assert_eq!(s.matches.len(), 1);
        assert_eq!(s.matches[0].0, 2);
    }

    #[test]
    fn move_up_clamps_at_zero() {
        let mut s = HistorySearch::new();
        s.recompute(&sample_history());
        s.move_up();
        assert_eq!(s.cursor, 0);
    }

    #[test]
    fn move_down_advances_within_bounds() {
        let mut s = HistorySearch::new();
        s.recompute(&sample_history());
        s.move_down();
        assert_eq!(s.cursor, 1);
    }

    #[test]
    fn selected_index_reads_match() {
        let mut s = HistorySearch::new();
        s.query = "diff".into();
        s.recompute(&sample_history());
        assert_eq!(s.selected_index(), Some(3));
    }

    #[test]
    fn empty_history_yields_no_matches() {
        let mut s = HistorySearch::new();
        s.recompute(&[]);
        assert!(s.matches.is_empty());
        assert!(s.selected_index().is_none());
    }
}
