//! Undo/redo stack para el input del chat (E40).
//!
//! Patron:
//! - Cada cambio del input (char push, backspace, paste, draft restore)
//!   debe llamar `record()` ANTES de aplicar el cambio.
//! - `undo()` mueve el estado actual al redo stack y retorna el snapshot
//!   previo (o `None` si no hay historial).
//! - `redo()` es la inversa.
//! - Cualquier cambio que no sea undo/redo invalida el redo stack (pattern
//!   estandar de editores).
//!
//! Limite: [`MAX_UNDO_STACK`] entries. Al llenarse se descarta el item mas
//! antiguo (drop front). Los snapshots son strings clonadas — el costo esta
//! acotado por el tamano tipico de un prompt (< 10KB en promedio).

pub const MAX_UNDO_STACK: usize = 50;

#[derive(Debug, Clone, Default)]
pub struct InputUndoStack {
    undo: Vec<String>,
    redo: Vec<String>,
}

impl InputUndoStack {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registra un snapshot del estado actual ANTES de aplicar un cambio.
    /// Invalida el redo stack (semantica estandar de editor).
    pub fn record(&mut self, current: &str) {
        self.undo.push(current.to_string());
        if self.undo.len() > MAX_UNDO_STACK {
            // Descarta el mas antiguo manteniendo los recientes.
            let excess = self.undo.len() - MAX_UNDO_STACK;
            self.undo.drain(..excess);
        }
        self.redo.clear();
    }

    /// Registra solo si el snapshot difiere del ultimo para evitar duplicados
    /// cuando el handler llama `record` repetidamente con el mismo buffer.
    pub fn record_if_changed(&mut self, current: &str) {
        if self.undo.last().map(|s| s.as_str()) != Some(current) {
            self.record(current);
        }
    }

    /// Undo: mueve `current` al redo stack y retorna el snapshot anterior.
    pub fn undo(&mut self, current: &str) -> Option<String> {
        let prev = self.undo.pop()?;
        self.redo.push(current.to_string());
        if self.redo.len() > MAX_UNDO_STACK {
            let excess = self.redo.len() - MAX_UNDO_STACK;
            self.redo.drain(..excess);
        }
        Some(prev)
    }

    /// Redo: mueve `current` al undo stack y retorna el snapshot siguiente.
    pub fn redo(&mut self, current: &str) -> Option<String> {
        let next = self.redo.pop()?;
        self.undo.push(current.to_string());
        if self.undo.len() > MAX_UNDO_STACK {
            let excess = self.undo.len() - MAX_UNDO_STACK;
            self.undo.drain(..excess);
        }
        Some(next)
    }

    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
    }

    /// Longitud del stack de undo. Usada por tests y futuros widgets de
    /// debug/indicador en el footer.
    #[allow(dead_code)]
    pub fn undo_len(&self) -> usize {
        self.undo.len()
    }

    /// Longitud del stack de redo. Usada por tests y futuros widgets.
    #[allow(dead_code)]
    pub fn redo_len(&self) -> usize {
        self.redo.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_and_undo_returns_previous() {
        let mut s = InputUndoStack::new();
        s.record("a");
        s.record("ab");
        assert_eq!(s.undo("abc"), Some("ab".to_string()));
        assert_eq!(s.undo("ab"), Some("a".to_string()));
        assert_eq!(s.undo("a"), None);
    }

    #[test]
    fn redo_reverses_undo() {
        let mut s = InputUndoStack::new();
        s.record("a");
        s.record("ab");
        let prev = s.undo("abc").unwrap();
        assert_eq!(prev, "ab");
        let next = s.redo("ab").unwrap();
        assert_eq!(next, "abc");
    }

    #[test]
    fn new_record_invalidates_redo() {
        let mut s = InputUndoStack::new();
        s.record("a");
        let _ = s.undo("ab");
        assert_eq!(s.redo_len(), 1);
        s.record("x");
        assert_eq!(s.redo_len(), 0);
    }

    #[test]
    fn limit_drops_oldest_when_exceeded() {
        let mut s = InputUndoStack::new();
        for i in 0..MAX_UNDO_STACK + 5 {
            s.record(&format!("{i}"));
        }
        assert_eq!(s.undo_len(), MAX_UNDO_STACK);
        // El mas antiguo deberia ser "5" (drops 0..4).
        let first = s.undo("current").unwrap();
        // El popped es el mas reciente registrado.
        assert_eq!(first, format!("{}", MAX_UNDO_STACK + 4));
    }

    #[test]
    fn record_if_changed_dedupes_consecutive_identical() {
        let mut s = InputUndoStack::new();
        s.record_if_changed("hello");
        s.record_if_changed("hello");
        s.record_if_changed("hello");
        assert_eq!(s.undo_len(), 1);
        s.record_if_changed("world");
        assert_eq!(s.undo_len(), 2);
    }
}
