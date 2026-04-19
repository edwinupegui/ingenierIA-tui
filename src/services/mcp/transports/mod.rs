//! Implementaciones concretas del trait `McpTransport`.

pub mod sse;
pub mod stdio;
pub mod websocket;

pub use sse::SseTransport;
#[allow(unused_imports, reason = "stdio transport disponible para integraciones futuras")]
pub use stdio::StdioTransport;
#[allow(unused_imports, reason = "websocket transport disponible para integraciones futuras")]
pub use websocket::WebSocketTransport;
