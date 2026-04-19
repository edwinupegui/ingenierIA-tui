//! `FocusStack`: stack de contextos de focus para hierarchy de overlays.
//!
//! Al abrir un overlay (permission_modal, command_palette, doc_picker, etc.)
//! se hace `push` del contexto actual. Al cerrarlo, `pop` restaura el focus
//! anterior. Maxima profundidad para prevenir memory leaks si un overlay
//! no se cierra correctamente.
//!
//! Referencia: claude-code `FocusManager` (stack 32 items en CC2).

#![cfg_attr(not(test), allow(dead_code, reason = "E37 toolkit — integracion pendiente"))]

use super::focus_trap::FocusTrap;

/// Maximo numero de contextos en el stack antes de evict the oldest.
pub const DEFAULT_MAX_STACK_SIZE: usize = 16;

/// Contexto de focus capturado al momento de abrir un overlay.
#[derive(Debug, Clone)]
pub struct FocusContext {
    /// Tag descriptivo para debugging/telemetria (ej: "permission_modal").
    pub label: String,
    /// Trap opcional activo para este contexto.
    pub trap: Option<FocusTrap>,
}

impl FocusContext {
    pub fn new(label: impl Into<String>) -> Self {
        Self { label: label.into(), trap: None }
    }

    pub fn with_trap(label: impl Into<String>, trap: FocusTrap) -> Self {
        Self { label: label.into(), trap: Some(trap) }
    }
}

/// Stack LIFO de focus contexts con cap automatico.
#[derive(Debug, Clone)]
pub struct FocusStack {
    stack: Vec<FocusContext>,
    max_size: usize,
}

impl FocusStack {
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_MAX_STACK_SIZE)
    }

    pub fn with_capacity(max_size: usize) -> Self {
        Self { stack: Vec::new(), max_size: max_size.max(1) }
    }

    /// Apila un contexto. Si excede `max_size`, elimina el mas viejo.
    pub fn push(&mut self, ctx: FocusContext) {
        if self.stack.len() >= self.max_size {
            self.stack.remove(0);
        }
        self.stack.push(ctx);
    }

    /// Desapila el contexto actual.
    pub fn pop(&mut self) -> Option<FocusContext> {
        self.stack.pop()
    }

    /// Contexto actual (cima del stack) sin desapilar.
    pub fn current(&self) -> Option<&FocusContext> {
        self.stack.last()
    }

    pub fn current_mut(&mut self) -> Option<&mut FocusContext> {
        self.stack.last_mut()
    }

    pub fn len(&self) -> usize {
        self.stack.len()
    }

    pub fn is_empty(&self) -> bool {
        self.stack.is_empty()
    }

    /// Limpia todo el stack (por ejemplo al salir de una pantalla).
    pub fn clear(&mut self) {
        self.stack.clear();
    }
}

impl Default for FocusStack {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_stack_returns_none() {
        let mut s = FocusStack::new();
        assert!(s.is_empty());
        assert_eq!(s.len(), 0);
        assert!(s.pop().is_none());
        assert!(s.current().is_none());
    }

    #[test]
    fn push_pop_lifo() {
        let mut s = FocusStack::new();
        s.push(FocusContext::new("a"));
        s.push(FocusContext::new("b"));
        assert_eq!(s.len(), 2);
        assert_eq!(s.current().map(|c| c.label.as_str()), Some("b"));
        let popped = s.pop().unwrap();
        assert_eq!(popped.label, "b");
        assert_eq!(s.len(), 1);
    }

    #[test]
    fn cap_evicts_oldest_when_full() {
        let mut s = FocusStack::with_capacity(3);
        for i in 0..5 {
            s.push(FocusContext::new(format!("ctx-{i}")));
        }
        assert_eq!(s.len(), 3);
        // El mas viejo ("ctx-0", "ctx-1") fue evicted.
        let labels: Vec<String> = s.stack.iter().map(|c| c.label.clone()).collect();
        assert_eq!(labels, vec!["ctx-2", "ctx-3", "ctx-4"]);
    }

    #[test]
    fn zero_capacity_clamps_to_one() {
        let mut s = FocusStack::with_capacity(0);
        s.push(FocusContext::new("a"));
        s.push(FocusContext::new("b"));
        assert_eq!(s.len(), 1);
        assert_eq!(s.current().map(|c| c.label.as_str()), Some("b"));
    }

    #[test]
    fn with_trap_stores_trap() {
        let trap = FocusTrap::new(vec![1, 2]);
        let ctx = FocusContext::with_trap("modal", trap);
        assert!(ctx.trap.is_some());
    }

    #[test]
    fn clear_empties_stack() {
        let mut s = FocusStack::new();
        s.push(FocusContext::new("a"));
        s.push(FocusContext::new("b"));
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn current_mut_allows_mutation() {
        let mut s = FocusStack::new();
        s.push(FocusContext::with_trap("modal", FocusTrap::new(vec![10, 20])));
        if let Some(ctx) = s.current_mut() {
            if let Some(trap) = ctx.trap.as_mut() {
                trap.handle_tab(false);
            }
        }
        let current_focus = s.current().and_then(|c| c.trap.as_ref()).and_then(|t| t.current());
        assert_eq!(current_focus, Some(20));
    }
}
