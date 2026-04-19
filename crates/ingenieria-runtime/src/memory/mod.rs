//! Memory System (E15): persistencia de memorias tipadas + inyeccion en prompt.
//!
//! Directorio: `~/.config/ingenieria-tui/memory/` con archivos `.md` que
//! contienen frontmatter (name/description/type) + body. El archivo
//! `MEMORY.md` es un indice auto-generado que se inyecta en el system
//! prompt al inicio de cada turno.
//!
//! 4 tipos de memoria (ver `types::MemoryType`):
//! - `user` — rol/preferencias del usuario
//! - `feedback` — correcciones y confirmaciones
//! - `project` — contexto de trabajo en curso
//! - `reference` — punteros a sistemas externos
//!
//! API publica:
//! - [`save_memory`] / [`load_memory`] / [`delete_memory`] / [`list_memories`]
//! - [`build_memory_context`] — bloque para system prompt
//!
//! Referencia: CC1 `src/memdir/` (memoryTypes.ts, paths.ts, memdir.ts).

pub mod context;
pub mod parser;
pub mod store;
pub mod types;

pub use context::{build_memory_context, memory_dir_display};
pub use store::{delete_memory, list_memories, save_memory};

// `load_memory` y `memory_dir` son API publica para futuras UIs; aun sin
// consumidor externo, preservarlos evita re-exportarlos en cada adicion.
#[allow(
    unused_imports,
    dead_code,
    reason = "API publica reservada para UIs de memory management"
)]
pub use store::{load_memory, memory_dir};
pub use types::{MemoryEntry, MemoryFrontmatter, MemoryType};

/// Re-export del lock compartido (ver `services::TEST_ENV_LOCK`). Sin este
/// lock, tests de memory/session/audit corren en paralelo y pisan la env global.
#[cfg(test)]
#[cfg(test)]
pub(super) use crate::TEST_ENV_LOCK;
