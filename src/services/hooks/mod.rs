//! E16 — Sistema de hooks configurable.
//!
//! Permite ejecutar comandos shell ante eventos del TUI:
//! - `PreToolUse` / `PostToolUse`: observabilidad de tool calls.
//! - `PreCodeApply`: gate informativo antes de aplicar code blocks.
//! - `OnFactorySwitch`: reacciones a cambio de factory context.
//!
//! Config en `$XDG_CONFIG_HOME/ingenieria-tui/hooks.json`. Los hooks cargan
//! al arrancar la App y se comparten via `Arc`. La ejecucion es fire-and-forget:
//! cada hook matching se ejecuta en paralelo y reporta su resultado como
//! `Action::HookExecuted` para que el reducer lo renderice o lo logee.
//!
//! Referencia: `claw-code/rust/crates/runtime/plugins/hooks.rs`.

// Re-export types and config from ingenieria-tools crate.
pub use ingenieria_tools::hooks::config;
pub use ingenieria_tools::hooks::types;

// Local: runner stays here because it depends on Action.
pub mod runner;

pub use config::load_hooks;
#[allow(unused_imports)]
pub use config::{HookDef, HookFailurePolicy};
pub use runner::HookRunner;
pub use types::{HookContext, HookOutcome, HookTrigger};

/// Inicializa el runner cargando `hooks.json`. Retorna el runner y warnings
/// no-fatales (hooks invalidos, JSON mal-formado). Nunca falla.
pub fn init_runner() -> (HookRunner, Vec<String>) {
    let (defs, warnings) = load_hooks();
    (HookRunner::new(defs), warnings)
}
