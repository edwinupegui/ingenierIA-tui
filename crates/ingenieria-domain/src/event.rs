use serde::Deserialize;

/// Eventos emitidos por el servidor via SSE en /api/events.
/// `serde(tag = "type")` usa el campo "type" del JSON como discriminador.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum IngenieriaEvent {
    Connected {
        message: String,
        timestamp: String,
    },
    Sync {
        factory: String,
        docs_changed: u32,
        timestamp: String,
    },
    Reload {
        file: String,
        timestamp: String,
    },
    Session {
        action: String,
        developer: String,
        total: u32,
    },
    Heartbeat {
        timestamp: String,
    },
    /// Captura cualquier tipo desconocido sin fallar
    #[serde(other)]
    Unknown,
}

impl IngenieriaEvent {
    pub fn kind_str(&self) -> &'static str {
        match self {
            IngenieriaEvent::Connected { .. } => "conn",
            IngenieriaEvent::Sync { .. } => "sync",
            IngenieriaEvent::Reload { .. } => "reload",
            IngenieriaEvent::Session { .. } => "session",
            IngenieriaEvent::Heartbeat { .. } => "heartbeat",
            IngenieriaEvent::Unknown => "unknown",
        }
    }

    pub fn summary(&self) -> String {
        match self {
            IngenieriaEvent::Connected { message, .. } => message.clone(),
            IngenieriaEvent::Sync { factory, docs_changed, .. } => {
                format!("sync {factory} ({docs_changed} docs)")
            }
            IngenieriaEvent::Reload { file, .. } => format!("reload {file}"),
            IngenieriaEvent::Session { developer, action, total, .. } => {
                format!("{developer} {action} (total: {total})")
            }
            IngenieriaEvent::Heartbeat { .. } => "heartbeat".into(),
            IngenieriaEvent::Unknown => "unknown event".into(),
        }
    }
}
