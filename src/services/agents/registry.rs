//! Registry de subagentes activos/historicos (E22a).
//!
//! Vive en `AppState.agents` y es la unica fuente de verdad sobre el estado
//! de los subagentes. El widget `agent_panel` lo lee, los slash commands lo
//! mutan via reducer y el spawner envia `Action::AgentResult` para finalizar.

use std::time::{Duration, SystemTime};

use super::role::AgentRole;

#[derive(Debug, Clone, PartialEq)]
pub enum AgentStatus {
    /// Encolado pero aun no aceptado por el spawner (rechazado por pool lleno).
    #[allow(dead_code, reason = "variant prevista para Sprint 11 cuando agreguemos waiting queue")]
    Pending,
    /// Tokio task spawneado y trabajando.
    Running,
    /// Termino con resultado exitoso (texto disponible en AgentInfo.result).
    Done,
    /// Fallo. Razon corta en `AgentInfo.result`.
    Failed,
    /// Cancelado por el usuario via `/agent-cancel`.
    Cancelled,
}

impl AgentStatus {
    pub fn label(&self) -> &'static str {
        match self {
            AgentStatus::Pending => "pending",
            AgentStatus::Running => "running",
            AgentStatus::Done => "done",
            AgentStatus::Failed => "failed",
            AgentStatus::Cancelled => "cancel",
        }
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self, AgentStatus::Done | AgentStatus::Failed | AgentStatus::Cancelled)
    }
}

#[derive(Debug, Clone)]
pub struct AgentInfo {
    pub id: String,
    pub role: String,
    pub prompt: String,
    pub status: AgentStatus,
    pub started_at: SystemTime,
    pub completed_at: Option<SystemTime>,
    pub result: Option<String>,
    /// Token cooperativo de cancelacion. El spawner lo chequea antes de mandar
    /// el resultado final; si esta `true`, lo descarta.
    pub cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
    /// Worktree aislado asignado al agente (E24). `None` si estamos fuera
    /// de un repo git o si el usuario deshabilito worktrees.
    pub worktree: Option<crate::services::worktree::WorktreeHandle>,
}

impl AgentInfo {
    pub fn new(id: String, role: &AgentRole, prompt: String) -> Self {
        Self {
            id,
            role: role.name().to_string(),
            prompt,
            status: AgentStatus::Running,
            started_at: SystemTime::now(),
            completed_at: None,
            result: None,
            cancel: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            worktree: None,
        }
    }

    pub fn duration(&self) -> Option<Duration> {
        let end = self.completed_at.unwrap_or_else(SystemTime::now);
        end.duration_since(self.started_at).ok()
    }

    /// Versión truncada del prompt para mostrar en tablas (≤ `width` chars).
    pub fn short_prompt(&self, width: usize) -> String {
        let trimmed = self.prompt.trim();
        if trimmed.chars().count() <= width {
            trimmed.to_string()
        } else {
            let truncated: String = trimmed.chars().take(width.saturating_sub(1)).collect();
            format!("{truncated}…")
        }
    }
}

/// Tope a partir del cual los registros mas viejos se descartan para evitar
/// crecimiento ilimitado en sesiones largas.
const MAX_AGENT_HISTORY: usize = 50;

#[derive(Debug, Default)]
pub struct AgentRegistry {
    pub agents: Vec<AgentInfo>,
    next_id: usize,
}

impl AgentRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Genera el siguiente ID corto (`a1`, `a2`, …).
    pub fn allocate_id(&mut self) -> String {
        self.next_id += 1;
        format!("a{}", self.next_id)
    }

    pub fn insert(&mut self, info: AgentInfo) {
        self.agents.push(info);
        // Drop oldest terminal records if we exceed the cap.
        if self.agents.len() > MAX_AGENT_HISTORY {
            if let Some(idx) = self.agents.iter().position(|a| a.status.is_terminal()) {
                self.agents.remove(idx);
            }
        }
    }

    #[allow(
        dead_code,
        reason = "consumido por widget agent_panel + tests; expuesto para integraciones futuras"
    )]
    pub fn get(&self, id: &str) -> Option<&AgentInfo> {
        self.agents.iter().find(|a| a.id == id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut AgentInfo> {
        self.agents.iter_mut().find(|a| a.id == id)
    }

    pub fn active_count(&self) -> usize {
        self.agents.iter().filter(|a| a.status == AgentStatus::Running).count()
    }

    #[allow(
        dead_code,
        reason = "consumido por widget agent_panel cuando muestra solo subagentes activos"
    )]
    pub fn active(&self) -> impl Iterator<Item = &AgentInfo> {
        self.agents.iter().filter(|a| a.status == AgentStatus::Running)
    }

    pub fn recent(&self, n: usize) -> impl Iterator<Item = &AgentInfo> {
        let len = self.agents.len();
        let start = len.saturating_sub(n);
        self.agents[start..].iter().rev()
    }

    pub fn finalize(&mut self, id: &str, status: AgentStatus, result: Option<String>) {
        if let Some(info) = self.get_mut(id) {
            info.status = status;
            info.completed_at = Some(SystemTime::now());
            info.result = result;
        }
    }

    /// Marca el flag cooperativo de cancelacion. Devuelve `true` si encontro el id.
    pub fn request_cancel(&mut self, id: &str) -> bool {
        if let Some(info) = self.get_mut(id) {
            info.cancel.store(true, std::sync::atomic::Ordering::Relaxed);
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(reg: &mut AgentRegistry) -> AgentInfo {
        let id = reg.allocate_id();
        AgentInfo::new(id, &AgentRole::Discovery, "find docs".into())
    }

    #[test]
    fn allocate_id_increments() {
        let mut r = AgentRegistry::new();
        assert_eq!(r.allocate_id(), "a1");
        assert_eq!(r.allocate_id(), "a2");
    }

    #[test]
    fn insert_and_lookup() {
        let mut r = AgentRegistry::new();
        let info = sample(&mut r);
        let id = info.id.clone();
        r.insert(info);
        assert_eq!(r.active_count(), 1);
        assert!(r.get(&id).is_some());
    }

    #[test]
    fn finalize_marks_done_and_completed_at() {
        let mut r = AgentRegistry::new();
        let info = sample(&mut r);
        let id = info.id.clone();
        r.insert(info);
        r.finalize(&id, AgentStatus::Done, Some("ok".into()));
        let entry = r.get(&id).unwrap();
        assert_eq!(entry.status, AgentStatus::Done);
        assert!(entry.completed_at.is_some());
        assert_eq!(entry.result.as_deref(), Some("ok"));
    }

    #[test]
    fn request_cancel_sets_flag() {
        let mut r = AgentRegistry::new();
        let info = sample(&mut r);
        let id = info.id.clone();
        let cancel_handle = info.cancel.clone();
        r.insert(info);
        assert!(r.request_cancel(&id));
        assert!(cancel_handle.load(std::sync::atomic::Ordering::Relaxed));
    }

    #[test]
    fn active_count_excludes_terminal() {
        let mut r = AgentRegistry::new();
        let info = sample(&mut r);
        let id = info.id.clone();
        r.insert(info);
        r.finalize(&id, AgentStatus::Done, None);
        assert_eq!(r.active_count(), 0);
    }

    #[test]
    fn short_prompt_truncates() {
        let mut r = AgentRegistry::new();
        let mut info = sample(&mut r);
        info.prompt = "x".repeat(100);
        assert!(info.short_prompt(20).chars().count() <= 20);
    }

    #[test]
    fn cap_enforced_when_exceeding_max_history() {
        let mut r = AgentRegistry::new();
        for _ in 0..(MAX_AGENT_HISTORY + 5) {
            let info = sample(&mut r);
            let id = info.id.clone();
            r.insert(info);
            r.finalize(&id, AgentStatus::Done, None);
        }
        assert!(r.agents.len() <= MAX_AGENT_HISTORY);
    }
}
