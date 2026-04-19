//! MCP (Model Context Protocol) client.
//!
//! E17: refactor a arquitectura basada en `McpTransport` trait. SSE sigue
//! siendo el transporte default; stdio disponible para servers locales.

mod client;
pub mod elicitation;
pub mod lifecycle;
pub mod pool;
pub mod protocol;
pub mod transport;
pub mod transports;
pub mod truncation;
pub mod validation;

pub use client::McpClient;
#[cfg(feature = "mcp")]
pub use client::McpToolInfo;
pub use pool::McpPool;
#[allow(unused_imports, reason = "re-export para futuras integraciones multi-transport")]
pub use transport::TransportKind;
