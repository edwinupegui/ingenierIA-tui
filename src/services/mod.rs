// ── Re-exports from ingenieria-runtime crate (E28 Phase 4) ───────────────────
pub use ingenieria_runtime::audit;
pub use ingenieria_runtime::config_validation;
pub use ingenieria_runtime::memory;
pub use ingenieria_runtime::session;

// ── Local modules (not yet extracted) ──────────────────────────────────────
// Re-export bash from ingenieria-tools (E28 Phase 5).
pub use ingenieria_tools::bash;

pub mod agents;
pub mod auth;
#[cfg(feature = "autoskill")]
pub mod autoskill_map;
pub mod bridge;
pub mod cache;
pub mod chat;
pub mod chat_metrics_persist;
pub mod codeblocks;
pub mod compactor;
pub mod compliance;
pub mod context;
pub mod copilot;
pub mod copilot_chat;
pub mod cron;
pub mod doc_cache;
pub mod doctor;
pub mod draft_store;
pub mod ingenieria_client;
pub mod features;
pub mod history;
pub mod hooks;
pub mod init;
pub mod init_gen;
mod init_templates;
pub mod lsp;
pub mod mcp;
pub mod mentions;
pub mod monitor;
pub mod onboarding;
pub mod paste_handler;
pub mod permissions;
#[allow(dead_code, reason = "E28: API consumed once external plugins are loaded at startup")]
pub mod plugins;
#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "E30b: typeahead service entregado con tests; consumido por slash_autocomplete en Sprint 11"
    )
)]
pub mod prompt_suggestions;
pub mod recovery_engine;
#[cfg(feature = "autoskill")]
pub mod skill_installer;
pub mod structured_output;
pub mod sync;
pub mod tools;
pub mod uri;
pub mod worktree;
pub use ingenieria_client::IngenieriaClient;
