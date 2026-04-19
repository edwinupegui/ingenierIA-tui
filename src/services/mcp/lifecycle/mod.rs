//! Gestion del ciclo de vida de servers MCP multi-instancia.
//!
//! E17b: `McpLifecycleManager` coordina multiples servers con degraded-mode,
//! auto-retry con backoff y routing de tools. Ver `manager.rs` para el diseño.

pub mod config;
pub mod manager;
pub mod retry;
pub mod state;

pub use config::{load_servers, servers_config_path};
#[allow(
    unused_imports,
    reason = "re-exports consumidos por integraciones futuras (chat_tools routing E22)"
)]
pub use config::{ServerConfig, ServerKind};
pub use manager::McpLifecycleManager;
pub use state::ServerState;
#[allow(
    unused_imports,
    reason = "re-exports de tipos snapshot consumidos via self::lifecycle::State en slash_commands"
)]
pub use state::{LifecycleSnapshot, ServerSnapshot};
