//! LSP integration (E25).
//!
//! Client generico que detecta automaticamente el language server adecuado,
//! captura diagnosticos via `publishDiagnostics` y los inyecta en el
//! contexto del AI para mejorar la calidad del codigo generado.

pub mod client;
pub mod context;
pub mod detection;
pub mod transport;
pub mod types;

pub use client::{spawn_lsp_client, LspCommand};
pub use context::format_diagnostics_context;
pub use detection::detect;
pub use types::{LspDiagnostic, Severity};
