//! Estado observable de cada server MCP administrado por el lifecycle manager.
//!
//! `ServerState` modela el ciclo de vida runtime. `ServerSnapshot` es un DTO
//! plano (owned, sin Arc) consumido por la UI para renders read-only (p.ej.
//! `/mcp-status`).

use super::super::transport::TransportKind;

/// Fase del ciclo de vida de un server MCP administrado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServerState {
    /// Spawn del transport en progreso (initialize + handshake).
    Connecting,
    /// Handshake exitoso. `tools_count` es el numero de tools reportados por `tools/list`.
    Ready { tools_count: usize },
    /// Conexion fallida o caida post-init. `attempts` acumula reintentos.
    Failed { reason: String, attempts: u32 },
    /// Explicitamente `enabled:false` en config.
    Disabled,
}

impl ServerState {
    /// True si esta sirviendo tools ahora mismo.
    pub fn is_ready(&self) -> bool {
        matches!(self, Self::Ready { .. })
    }

    /// Numero de tools reportados, o 0 si no ready.
    pub fn tools_count(&self) -> usize {
        match self {
            Self::Ready { tools_count } => *tools_count,
            _ => 0,
        }
    }

    /// Label corto para UI.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Connecting => "connecting",
            Self::Ready { .. } => "ready",
            Self::Failed { .. } => "failed",
            Self::Disabled => "disabled",
        }
    }
}

/// Snapshot inmutable de un server, seguro de pasar a la UI.
#[derive(Debug, Clone)]
pub struct ServerSnapshot {
    pub name: String,
    pub kind: TransportKind,
    pub state: ServerState,
}

/// Resumen agregado del manager (todos los servers).
#[derive(Debug, Clone, Default)]
pub struct LifecycleSnapshot {
    pub servers: Vec<ServerSnapshot>,
}

impl LifecycleSnapshot {
    /// (ready, total) contando solo servers no-disabled.
    pub fn ready_ratio(&self) -> (usize, usize) {
        let active: Vec<&ServerSnapshot> =
            self.servers.iter().filter(|s| !matches!(s.state, ServerState::Disabled)).collect();
        let ready = active.iter().filter(|s| s.state.is_ready()).count();
        (ready, active.len())
    }

    /// Suma de tools disponibles en este momento.
    pub fn total_tools(&self) -> usize {
        self.servers.iter().map(|s| s.state.tools_count()).sum()
    }

    /// True si al menos un server esta degraded (failed) y al menos uno ready.
    pub fn is_degraded(&self) -> bool {
        let any_failed = self.servers.iter().any(|s| matches!(s.state, ServerState::Failed { .. }));
        let any_ready = self.servers.iter().any(|s| s.state.is_ready());
        any_failed && any_ready
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap(name: &str, state: ServerState) -> ServerSnapshot {
        ServerSnapshot { name: name.to_string(), kind: TransportKind::Sse, state }
    }

    #[test]
    fn ready_state_counts_tools() {
        let s = ServerState::Ready { tools_count: 8 };
        assert!(s.is_ready());
        assert_eq!(s.tools_count(), 8);
        assert_eq!(s.label(), "ready");
    }

    #[test]
    fn failed_state_not_ready() {
        let s = ServerState::Failed { reason: "boom".into(), attempts: 2 };
        assert!(!s.is_ready());
        assert_eq!(s.tools_count(), 0);
        assert_eq!(s.label(), "failed");
    }

    #[test]
    fn disabled_excluded_from_ratio() {
        let snap = LifecycleSnapshot {
            servers: vec![
                snap("a", ServerState::Ready { tools_count: 3 }),
                snap("b", ServerState::Disabled),
                snap("c", ServerState::Failed { reason: "x".into(), attempts: 1 }),
            ],
        };
        assert_eq!(snap.ready_ratio(), (1, 2));
        assert_eq!(snap.total_tools(), 3);
    }

    #[test]
    fn degraded_requires_one_failed_and_one_ready() {
        let ok_only =
            LifecycleSnapshot { servers: vec![snap("a", ServerState::Ready { tools_count: 1 })] };
        assert!(!ok_only.is_degraded());

        let fail_only = LifecycleSnapshot {
            servers: vec![snap("a", ServerState::Failed { reason: "x".into(), attempts: 1 })],
        };
        assert!(!fail_only.is_degraded());

        let mixed = LifecycleSnapshot {
            servers: vec![
                snap("a", ServerState::Ready { tools_count: 1 }),
                snap("b", ServerState::Failed { reason: "x".into(), attempts: 1 }),
            ],
        };
        assert!(mixed.is_degraded());
    }

    #[test]
    fn empty_snapshot_defaults_safe() {
        let s = LifecycleSnapshot::default();
        assert_eq!(s.ready_ratio(), (0, 0));
        assert_eq!(s.total_tools(), 0);
        assert!(!s.is_degraded());
    }

    #[test]
    fn connecting_label() {
        assert_eq!(ServerState::Connecting.label(), "connecting");
        assert!(!ServerState::Connecting.is_ready());
    }
}
