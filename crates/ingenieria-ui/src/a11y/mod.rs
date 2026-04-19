//! Modulo de accesibilidad TUI (E37).
//!
//! Contiene primitivas para:
//!   - Desactivar animaciones (`reduced_motion`).
//!   - Atrapar navegacion en modals (`focus_trap`).
//!   - Gestionar focus hierarchy en overlays (`focus_stack`).
//!
//! Estas utilidades son puras (sin I/O fuera de lectura de env vars) y no
//! dependen de `AppState` ni `services/`. La integracion concreta con
//! modals/overlays vive en `ui/widgets/*` y se engancha explicitamente.

#![allow(unused_imports, reason = "re-exports E37 — toolkit pendiente de integrar")]
#![cfg_attr(not(test), allow(dead_code, reason = "E37 toolkit — integracion pendiente"))]

pub mod focus_stack;
pub mod focus_trap;
pub mod reduced_motion;

pub use focus_stack::{FocusContext, FocusStack, DEFAULT_MAX_STACK_SIZE};
pub use focus_trap::{FocusTrap, FocusableId};
pub use reduced_motion::{
    is_accessibility_mode, should_reduce_motion, spinner_frame, ENV_ACCESSIBILITY,
    ENV_REDUCE_MOTION, STATIC_SPINNER_FRAME,
};

/// Helper de alto nivel: si accesibilidad esta activa, ajusta el tick de
/// auto-dismiss de toasts (duplica la duracion para dar mas tiempo de lectura).
pub fn toast_lifetime_multiplier() -> u64 {
    if is_accessibility_mode() {
        2
    } else {
        1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn toast_multiplier_doubles_in_accessibility_mode() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        unsafe {
            std::env::set_var(ENV_ACCESSIBILITY, "1");
        }
        assert_eq!(toast_lifetime_multiplier(), 2);
        unsafe {
            std::env::remove_var(ENV_ACCESSIBILITY);
        }
    }

    #[test]
    fn toast_multiplier_default_is_one() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        unsafe {
            std::env::remove_var(ENV_ACCESSIBILITY);
        }
        assert_eq!(toast_lifetime_multiplier(), 1);
    }
}
