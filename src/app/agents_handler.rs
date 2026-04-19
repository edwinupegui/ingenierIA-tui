//! Slash commands para subagentes (E22a).
//!
//! Comandos:
//!   /spawn <role> <prompt...>   — spawnea un subagent
//!   /agent-list                 — tabla de subagentes activos/recientes
//!   /agent-cancel <id>          — solicita cancel cooperativo
//!
//! El handler de `Action::AgentResult` finaliza el registry y publica el
//! resultado como mensaje del asistente prefijado por `[agent <id>]`.

use crate::services::agents::{
    spawn_agent_task, AgentCreds, AgentInfo, AgentRole, AgentStatus, MAX_CONCURRENT_AGENTS,
};
use crate::state::{ChatMessage, ChatRole};

use super::App;

/// Limite de prompt aceptado por `/spawn`. Mas que esto se rechaza por UX.
const MAX_AGENT_PROMPT_LEN: usize = 2_000;
/// Maxima cantidad de agentes mostrados por `/agent-list`.
const AGENT_LIST_VIEW: usize = 12;

impl App {
    pub(crate) fn handle_spawn_agent_command(&mut self, raw: &str) {
        let (role_token, prompt) = split_role_prompt(raw);
        if role_token.is_empty() || prompt.is_empty() {
            self.notify(
                "Uso: /spawn <role> <prompt>. Roles: orchestrator, discovery, architecture, \
                 migration, execution, testing, docs, planning"
                    .to_string(),
            );
            return;
        }
        if prompt.len() > MAX_AGENT_PROMPT_LEN {
            self.notify(format!(
                "✗ Prompt demasiado largo ({} chars). Limite: {MAX_AGENT_PROMPT_LEN}",
                prompt.len()
            ));
            return;
        }
        if self.state.agents.active_count() >= MAX_CONCURRENT_AGENTS {
            self.notify(format!(
                "✗ Pool lleno: {}/{} agentes activos. Espera o /agent-cancel <id>",
                self.state.agents.active_count(),
                MAX_CONCURRENT_AGENTS,
            ));
            return;
        }

        let role = AgentRole::from_name(role_token);
        let id = self.state.agents.allocate_id();
        let mut info = AgentInfo::new(id.clone(), &role, prompt.to_string());
        // E24: intenta crear worktree aislado. Silencioso si no es git repo.
        match self.state.worktree_manager.create(&id) {
            Ok(Some(handle)) => {
                self.notify(format!("🌿 Worktree en {}", handle.path.display()));
                info.worktree = Some(handle);
            }
            Ok(None) => {}
            Err(e) => {
                tracing::warn!(agent = %id, err = %e, "worktree create fallo, continuando sin aislamiento");
            }
        }
        let cancel = info.cancel.clone();
        self.state.agents.insert(info);

        let creds = AgentCreds {
            claude_key: crate::app::wizard::load_claude_api_key(),
            copilot_auth: crate::services::copilot::load_saved_auth(),
            mock: self.mock_provider,
        };
        spawn_agent_task(
            id.clone(),
            role.clone(),
            prompt.to_string(),
            creds,
            self.state.model.clone(),
            cancel,
            self.tx.clone(),
        );
        self.notify(format!("⇒ Agent {id} ({}) lanzado", role.name()));
    }

    pub(crate) fn handle_agent_list_command(&mut self) {
        let body = format_agent_list(&self.state.agents);
        let cached =
            crate::ui::widgets::markdown::render_markdown(&body, &self.state.active_theme.colors());
        let mut msg = ChatMessage::new(ChatRole::Assistant, body);
        msg.cached_lines = Some(std::sync::Arc::new(cached));
        self.state.chat.messages.push(msg);
        self.state.chat.scroll_offset = u16::MAX;
    }

    pub(crate) fn handle_agent_cancel_command(&mut self, arg: &str) {
        let id = arg.trim();
        if id.is_empty() {
            self.notify("Uso: /agent-cancel <id>".to_string());
            return;
        }
        if self.state.agents.request_cancel(id) {
            self.notify(format!("⊘ Cancel solicitado para {id}"));
        } else {
            self.notify(format!("✗ Agent no encontrado: {id}"));
        }
    }

    pub(crate) fn handle_agent_result(
        &mut self,
        id: String,
        status: AgentStatus,
        result: Option<String>,
    ) {
        self.state.agents.finalize(&id, status.clone(), result.clone());
        // E24: limpiar worktree al finalizar el agente (cualquier estado terminal).
        if let Err(e) = self.state.worktree_manager.cleanup(&id) {
            tracing::warn!(agent = %id, err = %e, "worktree cleanup fallo");
        }
        // E22b: si el agente pertenece a un team, el output individual se
        // suprime — el resumen consolidado lo publica `on_team_member_finished`
        // una vez que todos los miembros terminan.
        let is_team_member = self.state.teams.team_id_of_agent(&id).is_some();
        match status {
            AgentStatus::Done if !is_team_member => {
                let body = format_agent_done(&id, result.as_deref().unwrap_or(""));
                let cached = crate::ui::widgets::markdown::render_markdown(
                    &body,
                    &self.state.active_theme.colors(),
                );
                let mut msg = ChatMessage::new(ChatRole::Assistant, body);
                msg.cached_lines = Some(std::sync::Arc::new(cached));
                self.state.chat.messages.push(msg);
                self.state.chat.scroll_offset = u16::MAX;
                self.notify(format!("✓ Agent {id} completo"));
            }
            AgentStatus::Done => {
                self.notify(format!("✓ Agent {id} (team) completo"));
            }
            AgentStatus::Failed => {
                let reason = result.unwrap_or_else(|| "(sin detalle)".into());
                self.notify_level(
                    &format!("✗ Agent {id} fallo: {reason}"),
                    crate::state::ToastLevel::Error,
                );
            }
            AgentStatus::Cancelled => {
                self.notify(format!("⊘ Agent {id} cancelado"));
            }
            AgentStatus::Pending | AgentStatus::Running => {
                tracing::warn!(id, ?status, "AgentResult con estado no terminal — ignorado");
            }
        }
        // E22b: re-compute team status + publish consolidated summary si aplica.
        if is_team_member {
            self.on_team_member_finished(&id);
        }
    }
}

fn split_role_prompt(raw: &str) -> (&str, &str) {
    match raw.trim().split_once(char::is_whitespace) {
        Some((role, rest)) => (role.trim(), rest.trim()),
        None => (raw.trim(), ""),
    }
}

fn format_agent_done(id: &str, body: &str) -> String {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        format!("**[agent {id}]** (sin output)")
    } else {
        format!("**[agent {id}]**\n\n{trimmed}")
    }
}

fn format_agent_list(reg: &crate::services::agents::AgentRegistry) -> String {
    let mut out = String::from("## Subagentes\n\n");
    out.push_str(&format!("Activos: {}/{MAX_CONCURRENT_AGENTS}\n\n", reg.active_count()));

    if reg.agents.is_empty() {
        out.push_str(
            "No hay subagentes registrados. Usa `/spawn <role> <prompt>` para lanzar uno.\n\n",
        );
        out.push_str("**Roles disponibles**: ");
        out.push_str(&AgentRole::canonical_names().join(", "));
        out.push('\n');
        return out;
    }

    out.push_str("| ID | Rol | Estado | Duracion | Prompt |\n");
    out.push_str("|----|-----|--------|----------|--------|\n");
    for info in reg.recent(AGENT_LIST_VIEW) {
        let dur =
            info.duration().map(|d| format!("{}s", d.as_secs())).unwrap_or_else(|| "-".to_string());
        out.push_str(&format!(
            "| `{}` | {} | {} | {} | {} |\n",
            info.id,
            info.role,
            info.status.label(),
            dur,
            info.short_prompt(40),
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::agents::AgentRegistry;

    #[test]
    fn split_role_prompt_basic() {
        let (role, prompt) = split_role_prompt("discovery find docs about hooks");
        assert_eq!(role, "discovery");
        assert_eq!(prompt, "find docs about hooks");
    }

    #[test]
    fn split_role_prompt_no_prompt() {
        let (role, prompt) = split_role_prompt("orchestrator");
        assert_eq!(role, "orchestrator");
        assert_eq!(prompt, "");
    }

    #[test]
    fn format_done_includes_id_and_body() {
        let s = format_agent_done("a1", "hello world");
        assert!(s.contains("agent a1"));
        assert!(s.contains("hello world"));
    }

    #[test]
    fn format_done_handles_empty_body() {
        let s = format_agent_done("a2", "   ");
        assert!(s.contains("sin output"));
    }

    #[test]
    fn format_list_empty_shows_help() {
        let reg = AgentRegistry::new();
        let s = format_agent_list(&reg);
        assert!(s.contains("No hay subagentes"));
        assert!(s.contains("orchestrator"));
    }

    #[test]
    fn format_list_renders_table() {
        let mut reg = AgentRegistry::new();
        let id = reg.allocate_id();
        let info = AgentInfo::new(id.clone(), &AgentRole::Discovery, "find x".into());
        reg.insert(info);
        let s = format_agent_list(&reg);
        assert!(s.contains("a1"));
        assert!(s.contains("discovery"));
    }
}
