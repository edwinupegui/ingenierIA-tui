//! `FocusTrap`: atrapa navegacion Tab/Shift+Tab dentro de una lista finita
//! de focusables. Usado en modals y overlays para que el focus no escape.
//!
//! Referencia: claude-code 2 `FocusTrap`. El trap es agnostico al tipo de
//! item — trabaja con un id opaco `FocusableId` (u32) que el caller mapea
//! a widgets concretos.

#![cfg_attr(not(test), allow(dead_code, reason = "E37 toolkit — integracion pendiente"))]

/// Id opaco asignado a cada item focusable dentro de un trap.
pub type FocusableId = u32;

/// Trampa de focus: la navegacion Tab cicla entre `focusable_items`.
#[derive(Debug, Clone)]
pub struct FocusTrap {
    focusable_items: Vec<FocusableId>,
    current_index: usize,
}

impl FocusTrap {
    /// Crea un trap con los ids dados. El primer item empieza focuseado.
    pub fn new(items: Vec<FocusableId>) -> Self {
        Self { focusable_items: items, current_index: 0 }
    }

    /// `true` si no hay focusables (trap vacio — no-op).
    pub fn is_empty(&self) -> bool {
        self.focusable_items.is_empty()
    }

    /// Id actualmente focuseado.
    pub fn current(&self) -> Option<FocusableId> {
        self.focusable_items.get(self.current_index).copied()
    }

    /// Avanza al siguiente item. Si `shift` es true retrocede.
    /// Wraps around en ambos sentidos.
    pub fn handle_tab(&mut self, shift: bool) -> Option<FocusableId> {
        if self.focusable_items.is_empty() {
            return None;
        }
        let len = self.focusable_items.len();
        if shift {
            self.current_index = self.current_index.checked_sub(1).unwrap_or(len - 1);
        } else {
            self.current_index = (self.current_index + 1) % len;
        }
        self.current()
    }

    /// Fuerza el focus a un id especifico (si existe en el trap).
    pub fn focus_on(&mut self, id: FocusableId) -> bool {
        if let Some(idx) = self.focusable_items.iter().position(|x| *x == id) {
            self.current_index = idx;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_trap_is_noop() {
        let mut trap = FocusTrap::new(vec![]);
        assert!(trap.is_empty());
        assert_eq!(trap.current(), None);
        assert_eq!(trap.handle_tab(false), None);
    }

    #[test]
    fn starts_at_first_item() {
        let trap = FocusTrap::new(vec![10, 20, 30]);
        assert_eq!(trap.current(), Some(10));
    }

    #[test]
    fn tab_advances_forward() {
        let mut trap = FocusTrap::new(vec![10, 20, 30]);
        assert_eq!(trap.handle_tab(false), Some(20));
        assert_eq!(trap.handle_tab(false), Some(30));
    }

    #[test]
    fn tab_wraps_to_first() {
        let mut trap = FocusTrap::new(vec![10, 20]);
        trap.handle_tab(false);
        assert_eq!(trap.handle_tab(false), Some(10));
    }

    #[test]
    fn shift_tab_goes_backward() {
        let mut trap = FocusTrap::new(vec![10, 20, 30]);
        assert_eq!(trap.handle_tab(true), Some(30));
        assert_eq!(trap.handle_tab(true), Some(20));
    }

    #[test]
    fn focus_on_moves_index() {
        let mut trap = FocusTrap::new(vec![10, 20, 30]);
        assert!(trap.focus_on(30));
        assert_eq!(trap.current(), Some(30));
    }

    #[test]
    fn focus_on_unknown_id_returns_false() {
        let mut trap = FocusTrap::new(vec![10, 20]);
        assert!(!trap.focus_on(999));
        // No cambia el focus.
        assert_eq!(trap.current(), Some(10));
    }
}
