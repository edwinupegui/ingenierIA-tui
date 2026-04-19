//! Slash commands para teams multi-agente (E22b).
//!
//! Comandos:
//!   /team-start <template> <goal>  — lanza un team (fullstack|research|plan-exec|docs-research)
//!   /team-list                     — tabla de teams activos/recientes
//!   /team-cancel <id>              — cancel cooperativo de todos los miembros
//!
//! El handler de `Action::AgentResult` (en `agents_handler.rs`) recomputa el
//! team status cada vez que un miembro termina; cuando todos terminan se
//! publica un resumen consolidado.

use crate::services::agents::{
    member_prompt, spawn_agent_task, AgentCreds, AgentInfo, TeamInfo, TeamTemplate,
    MAX_CONCURRENT_AGENTS, MAX_CONCURRENT_TEAMS,
};
use crate::state::{ChatMessage, ChatRole};

use super::App;

/// Limite de caracteres para el goal de un team — UX.
const MAX_GOAL_LEN: usize = 2_000;

impl App {
    pub(crate) fn handle_team_start_command(&mut self, raw: &str) {
        let (template_token, goal) = split_template_goal(raw);
        if template_token.is_empty() || goal.is_empty() {
            self.notify(
                "Uso: /team-start <template> <goal>. Templates: fullstack, research, \
                 plan-exec, docs-research"
                    .to_string(),
            );
            return;
        }
        if goal.len() > MAX_GOAL_LEN {
            self.notify(format!(
                "✗ Goal demasiado largo ({} chars). Limite: {MAX_GOAL_LEN}",
                goal.len()
            ));
            return;
        }
        let Some(template) = TeamTemplate::from_name(template_token) else {
            self.notify(format!(
                "✗ Template desconocido: '{template_token}'. Disponibles: {}",
                TeamTemplate::canonical_names().join(", ")
            ));
            return;
        };
        if self.state.teams.active_count() >= MAX_CONCURRENT_TEAMS {
            self.notify(format!(
                "✗ Pool de teams lleno: {}/{MAX_CONCURRENT_TEAMS}. Espera o /team-cancel",
                self.state.teams.active_count(),
            ));
            return;
        }
        let roles = template.roles();
        if self.state.agents.active_count() + roles.len() > MAX_CONCURRENT_AGENTS {
            self.notify(format!(
                "✗ No hay capacidad para {} agentes ({} activos, max {MAX_CONCURRENT_AGENTS}). \
                 Cancela agentes activos primero.",
                roles.len(),
                self.state.agents.active_count(),
            ));
            return;
        }

        self.spawn_team_members(template, goal);
    }

    fn spawn_team_members(&mut self, template: TeamTemplate, goal: &str) {
        let team_id = self.state.teams.allocate_id();
        let roles = template.roles();
        let mut member_ids = Vec::with_capacity(roles.len());

        let creds = AgentCreds {
            claude_key: crate::app::wizard::load_claude_api_key(),
            copilot_auth: crate::services::copilot::load_saved_auth(),
            mock: self.mock_provider,
        };

        for (idx, role) in roles.iter().enumerate() {
            let is_leader = idx == 0;
            let prompt = member_prompt(role, goal, is_leader);
            let agent_id = self.state.agents.allocate_id();
            let mut info = AgentInfo::new(agent_id.clone(), role, prompt.clone());
            // E24: worktree aislado para cada miembro del team.
            if let Ok(Some(handle)) = self.state.worktree_manager.create(&agent_id) {
                info.worktree = Some(handle);
            }
            let cancel = info.cancel.clone();
            self.state.agents.insert(info);
            spawn_agent_task(
                agent_id.clone(),
                role.clone(),
                prompt,
                creds.clone(),
                self.state.model.clone(),
                cancel,
                self.tx.clone(),
            );
            member_ids.push(agent_id);
        }

        let team =
            TeamInfo::new(team_id.clone(), template.clone(), goal.to_string(), member_ids.clone());
        self.state.teams.insert(team);
        self.notify(format!(
            "⇒ Team {team_id} ({}) lanzado con {} miembros: {}",
            template.label(),
            member_ids.len(),
            member_ids.join(", ")
        ));
    }

    pub(crate) fn handle_team_list_command(&mut self) {
        let body = format_team_list(&self.state.teams);
        let cached =
            crate::ui::widgets::markdown::render_markdown(&body, &self.state.active_theme.colors());
        let mut msg = ChatMessage::new(ChatRole::Assistant, body);
        msg.cached_lines = Some(std::sync::Arc::new(cached));
        self.state.chat.messages.push(msg);
        self.state.chat.scroll_offset = u16::MAX;
    }

    pub(crate) fn handle_team_cancel_command(&mut self, arg: &str) {
        let id = arg.trim();
        if id.is_empty() {
            self.notify("Uso: /team-cancel <id>".to_string());
            return;
        }
        let Some(team) = self.state.teams.get(id) else {
            self.notify(format!("✗ Team no encontrado: {id}"));
            return;
        };
        let member_ids = team.member_ids.clone();
        for agent_id in &member_ids {
            self.state.agents.request_cancel(agent_id);
        }
        self.notify(format!("⊘ Cancel solicitado para team {id} ({} miembros)", member_ids.len()));
    }

    /// Llamado desde el handler de `AgentResult` cuando un agente termina —
    /// postea su resultado al mailbox del team y recomputa el status.
    pub(crate) fn on_team_member_finished(&mut self, agent_id: &str) {
        let Some(team_id) = self.state.teams.team_id_of_agent(agent_id) else {
            return;
        };
        // Post agent result to team mailbox (IPC real).
        if let Some(agent) = self.state.agents.get(agent_id) {
            let body = agent
                .result
                .clone()
                .unwrap_or_else(|| format!("[{}] (sin output)", agent.status.label()));
            self.state.teams.post_mail(
                &team_id,
                crate::services::agents::MailMessage {
                    from: agent_id.to_string(),
                    body,
                    timestamp: std::time::SystemTime::now(),
                },
            );
        }
        let agents = &self.state.agents;
        self.state.teams.recompute_status(&team_id, |id| agents.get(id).map(|a| a.status.clone()));
        let Some(team) = self.state.teams.get(&team_id).cloned() else {
            return;
        };
        if team.status.is_terminal() {
            self.publish_team_summary(&team);
        }
    }

    /// `/team-mail <id>` — show the mailbox of a team.
    pub(crate) fn handle_team_mail_command(&mut self, arg: &str) {
        let id = arg.trim();
        if id.is_empty() {
            self.notify("Uso: /team-mail <id>".to_string());
            return;
        }
        let Some(team) = self.state.teams.get(id).cloned() else {
            self.notify(format!("✗ Team no encontrado: {id}"));
            return;
        };
        let body = format_team_mailbox(&team);
        let cached =
            crate::ui::widgets::markdown::render_markdown(&body, &self.state.active_theme.colors());
        let mut msg = ChatMessage::new(ChatRole::Assistant, body);
        msg.cached_lines = Some(std::sync::Arc::new(cached));
        self.state.chat.messages.push(msg);
        self.state.chat.scroll_offset = u16::MAX;
    }

    fn publish_team_summary(&mut self, team: &TeamInfo) {
        let body = format_team_summary(team, &self.state.agents);
        let cached =
            crate::ui::widgets::markdown::render_markdown(&body, &self.state.active_theme.colors());
        let mut msg = ChatMessage::new(ChatRole::Assistant, body);
        msg.cached_lines = Some(std::sync::Arc::new(cached));
        self.state.chat.messages.push(msg);
        self.state.chat.scroll_offset = u16::MAX;
        self.notify(format!("✓ Team {} completo ({})", team.id, team.status.label()));
    }
}

fn split_template_goal(raw: &str) -> (&str, &str) {
    match raw.trim().split_once(char::is_whitespace) {
        Some((tpl, rest)) => (tpl.trim(), rest.trim()),
        None => (raw.trim(), ""),
    }
}

fn format_team_list(reg: &crate::services::agents::TeamRegistry) -> String {
    let mut out = String::from("## Teams\n\n");
    out.push_str(&format!("Activos: {}/{MAX_CONCURRENT_TEAMS}\n\n", reg.active_count()));
    if reg.teams.is_empty() {
        out.push_str(
            "No hay teams registrados. Usa `/team-start <template> <goal>` para lanzar uno.\n\n",
        );
        out.push_str("**Templates**: ");
        out.push_str(&TeamTemplate::canonical_names().join(", "));
        out.push('\n');
        return out;
    }
    out.push_str("| ID | Template | Estado | Miembros | Duracion | Goal |\n");
    out.push_str("|----|----------|--------|----------|----------|------|\n");
    for team in reg.teams.iter().rev() {
        let dur =
            team.duration().map(|d| format!("{}s", d.as_secs())).unwrap_or_else(|| "-".to_string());
        let goal_short = if team.goal.len() <= 40 {
            team.goal.clone()
        } else {
            format!("{}…", &team.goal[..39])
        };
        out.push_str(&format!(
            "| `{}` | {} | {} | {} | {} | {} |\n",
            team.id,
            team.template.label(),
            team.status.label(),
            team.member_ids.join(","),
            dur,
            goal_short,
        ));
    }
    out
}

fn format_team_summary(team: &TeamInfo, agents: &crate::services::agents::AgentRegistry) -> String {
    let mut out = format!(
        "**[team {}]** · template `{}` · {}\n\n> {}\n\n",
        team.id,
        team.template.label(),
        team.status.label(),
        team.goal.replace('\n', " ")
    );
    for (idx, agent_id) in team.member_ids.iter().enumerate() {
        let is_leader = idx == 0;
        let Some(agent) = agents.get(agent_id) else {
            continue;
        };
        let tag = if is_leader { "LEADER" } else { "worker" };
        let header =
            format!("### {} · `{}` · {} ({})", tag, agent.id, agent.role, agent.status.label());
        out.push_str(&header);
        out.push_str("\n\n");
        match &agent.result {
            Some(r) if !r.trim().is_empty() => {
                out.push_str(r.trim());
                out.push_str("\n\n");
            }
            _ => {
                out.push_str("_(sin output)_\n\n");
            }
        }
    }
    out
}

fn format_team_mailbox(team: &TeamInfo) -> String {
    let mut out = format!("## Mailbox — Team {}\n\n", team.id);
    if team.mailbox.is_empty() {
        out.push_str("_(sin mensajes)_\n");
        return out;
    }
    out.push_str(&format!("{} mensaje(s)\n\n", team.mailbox.len()));
    for msg in &team.mailbox {
        let dur = msg
            .timestamp
            .duration_since(team.started_at)
            .map(|d| format!("+{}s", d.as_secs()))
            .unwrap_or_else(|_| "?".into());
        out.push_str(&format!("**[{}]** _{dur}_\n\n", msg.from));
        let body_preview = if msg.body.len() > 500 {
            format!("{}…", &msg.body[..497])
        } else {
            msg.body.clone()
        };
        out.push_str(&body_preview);
        out.push_str("\n\n---\n\n");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_template_goal_extracts_both() {
        let (t, g) = split_template_goal("fullstack implement feature X");
        assert_eq!(t, "fullstack");
        assert_eq!(g, "implement feature X");
    }

    #[test]
    fn split_template_goal_empty_goal() {
        let (t, g) = split_template_goal("fullstack");
        assert_eq!(t, "fullstack");
        assert_eq!(g, "");
    }

    #[test]
    fn format_team_list_empty_shows_hint() {
        let reg = crate::services::agents::TeamRegistry::new();
        let out = format_team_list(&reg);
        assert!(out.contains("No hay teams"));
        assert!(out.contains("fullstack"));
    }

    #[test]
    fn format_team_list_renders_row() {
        let mut reg = crate::services::agents::TeamRegistry::new();
        reg.insert(TeamInfo::new(
            "t1".into(),
            TeamTemplate::Research,
            "explore X".into(),
            vec!["a1".into(), "a2".into()],
        ));
        let out = format_team_list(&reg);
        assert!(out.contains("| `t1` |"));
        assert!(out.contains("research"));
        assert!(out.contains("a1,a2"));
    }
}
