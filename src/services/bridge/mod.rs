//! IDE Bridge (E27).
//!
//! HTTP server local que permite a IDEs (VS Code, JetBrains, etc) interactuar
//! con la TUI: enviar contexto, aprobar/denegar tools, y recibir estado.
//!
//! Requiere feature `ide` para compilar el server axum.

pub mod protocol;

#[cfg(feature = "ide")]
pub mod server;

#[cfg(feature = "ide")]
pub use server::{spawn_bridge_server, BridgeSnapshot, DEFAULT_PORT};
