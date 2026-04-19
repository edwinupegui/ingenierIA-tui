//! Slash commands para cron jobs (E23).
//!
//! Comandos:
//!   /cron-add notify "<expr>" <message>
//!   /cron-add spawn  "<expr>" <role> <prompt...>
//!   /cron-list
//!   /cron-remove <id>
//!
//! La expresion va entre comillas dobles para no chocar con el espacio que
//! separa cron fields. El handler de `Action::CronJobFired` materializa el
//! side-effect (notify o spawn de subagent).

use crate::services::agents::{
    spawn_agent_task, AgentCreds, AgentInfo, AgentRole, MAX_CONCURRENT_AGENTS,
};
use crate::services::cron::{
    crons_config_path, parse_expression, save_jobs, schedule_summary, CronAction, CronJob,
};
use crate::state::{ChatMessage, ChatRole};

use super::App;

/// Limite de longitud del mensaje notify (evita expresiones absurdas).
const MAX_NOTIFY_LEN: usize = 240;

impl App {
    pub(crate) fn handle_cron_add_command(&mut self, raw: &str) {
        let raw = raw.trim();
        if raw.is_empty() {
            self.notify("Uso: /cron-add <notify|spawn> \"<expr>\" <args>".to_string());
            return;
        }
        let (kind, rest) = match raw.split_once(char::is_whitespace) {
            Some((k, r)) => (k.trim().to_ascii_lowercase(), r.trim()),
            None => (raw.to_ascii_lowercase(), ""),
        };

        let Some((expr, body)) = parse_quoted_expression(rest) else {
            self.notify(
                "✗ Expresion debe ir entre comillas dobles. Ej: /cron-add notify \"0 0 9 * * *\" Standup"
                    .to_string(),
            );
            return;
        };

        if let Err(e) = parse_expression(&expr) {
            self.notify(format!("✗ Expresion cron invalida: {e}"));
            return;
        }

        let action = match kind.as_str() {
            "notify" => match build_notify_action(body) {
                Ok(a) => a,
                Err(e) => {
                    self.notify(format!("✗ {e}"));
                    return;
                }
            },
            "spawn" => match build_spawn_action(body) {
                Ok(a) => a,
                Err(e) => {
                    self.notify(format!("✗ {e}"));
                    return;
                }
            },
            other => {
                self.notify(format!("✗ Tipo desconocido: '{other}'. Usa: notify | spawn"));
                return;
            }
        };

        let id = self.state.crons.allocate_id();
        let job = CronJob::new(id.clone(), expr.clone(), action);
        self.state.crons.add(job);
        if let Err(e) = save_jobs(&self.state.crons.snapshot()) {
            self.notify(format!("⚠ Cron agregado pero falla persistencia: {e}"));
        }
        let summary = schedule_summary(&expr);
        self.notify(format!("✓ Cron {id} agregado — proxima: {summary}"));
    }

    pub(crate) fn handle_cron_list_command(&mut self) {
        let path = crons_config_path()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "(config dir no disponible)".into());
        let mut body = format_cron_list(&self.state.crons.snapshot());
        body.push_str(&format!("\n*Persistencia:* `{path}`\n"));
        let cached =
            crate::ui::widgets::markdown::render_markdown(&body, &self.state.active_theme.colors());
        let mut msg = ChatMessage::new(ChatRole::Assistant, body);
        msg.cached_lines = Some(std::sync::Arc::new(cached));
        self.state.chat.messages.push(msg);
        self.state.chat.scroll_offset = u16::MAX;
    }

    pub(crate) fn handle_cron_remove_command(&mut self, arg: &str) {
        let id = arg.trim();
        if id.is_empty() {
            self.notify("Uso: /cron-remove <id>".to_string());
            return;
        }
        if !self.state.crons.remove(id) {
            self.notify(format!("✗ Cron no encontrado: {id}"));
            return;
        }
        if let Err(e) = save_jobs(&self.state.crons.snapshot()) {
            self.notify(format!("⚠ Cron removido pero falla persistencia: {e}"));
        } else {
            self.notify(format!("✓ Cron {id} removido"));
        }
    }

    pub(crate) fn handle_cron_fired(&mut self, id: String) {
        let Some(job) = self.state.crons.get_clone(&id) else {
            tracing::warn!(id, "CronJobFired sin job correspondiente");
            return;
        };
        self.state.crons.record_fired(&id, chrono::Utc::now());
        // Persistir el fire_count actualizado, best-effort.
        if let Err(e) = save_jobs(&self.state.crons.snapshot()) {
            tracing::warn!(error = %e, "fallo persistir cron fire_count");
        }

        match job.action.clone() {
            CronAction::Notify { message } => {
                self.notify(format!("⏰ [cron {id}] {message}"));
            }
            CronAction::Spawn { role, prompt } => self.fire_cron_spawn(&id, role, prompt),
        }
    }

    fn fire_cron_spawn(&mut self, cron_id: &str, role: String, prompt: String) {
        if self.state.agents.active_count() >= MAX_CONCURRENT_AGENTS {
            self.notify(format!("⚠ [cron {cron_id}] pool subagent lleno — spawn omitido"));
            return;
        }
        let role = AgentRole::from_name(&role);
        let id = self.state.agents.allocate_id();
        let mut info = AgentInfo::new(id.clone(), &role, prompt.clone());
        // E24: intentar worktree aislado (degradacion silenciosa fuera de git).
        if let Ok(Some(handle)) = self.state.worktree_manager.create(&id) {
            info.worktree = Some(handle);
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
            role,
            prompt,
            creds,
            self.state.model.clone(),
            cancel,
            self.tx.clone(),
        );
        self.notify(format!("⏰ [cron {cron_id}] → agent {id} lanzado"));
    }
}

/// Extrae la expresion entre comillas dobles y devuelve `(expr, resto)`.
fn parse_quoted_expression(text: &str) -> Option<(String, &str)> {
    let text = text.trim_start();
    let rest = text.strip_prefix('"')?;
    let end = rest.find('"')?;
    let expr = rest[..end].to_string();
    let after = rest[end + 1..].trim_start();
    Some((expr, after))
}

fn build_notify_action(body: &str) -> anyhow::Result<CronAction> {
    let msg = body.trim();
    if msg.is_empty() {
        anyhow::bail!("notify requiere un mensaje");
    }
    if msg.len() > MAX_NOTIFY_LEN {
        anyhow::bail!("mensaje demasiado largo (> {MAX_NOTIFY_LEN} chars)");
    }
    Ok(CronAction::Notify { message: msg.to_string() })
}

fn build_spawn_action(body: &str) -> anyhow::Result<CronAction> {
    let body = body.trim();
    let (role, prompt) = body
        .split_once(char::is_whitespace)
        .ok_or_else(|| anyhow::anyhow!("spawn requiere <role> <prompt>"))?;
    let prompt = prompt.trim();
    if prompt.is_empty() {
        anyhow::bail!("spawn requiere un prompt despues del role");
    }
    Ok(CronAction::Spawn { role: role.to_string(), prompt: prompt.to_string() })
}

fn format_cron_list(jobs: &[CronJob]) -> String {
    let mut out = String::from("## Cron Jobs\n\n");
    if jobs.is_empty() {
        out.push_str(
            "No hay cron jobs configurados. Ej:\n\n\
             ```\n/cron-add notify \"0 0 9 * * Mon-Fri\" Standup en 9am\n```\n",
        );
        return out;
    }
    out.push_str("| ID | Expresion | Accion | Fires | Proxima |\n");
    out.push_str("|----|-----------|--------|-------|---------|\n");
    for job in jobs {
        let proxima = if job.enabled {
            schedule_summary(&job.expression).split(',').next().unwrap_or("-").trim().to_string()
        } else {
            "(disabled)".to_string()
        };
        out.push_str(&format!(
            "| `{}` | `{}` | {} | {} | {proxima} |\n",
            job.id,
            job.expression,
            job.action.summary(40),
            job.fire_count,
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_quoted_basic() {
        let (expr, rest) = parse_quoted_expression(r#""0 * * * * *" hello world"#).unwrap();
        assert_eq!(expr, "0 * * * * *");
        assert_eq!(rest, "hello world");
    }

    #[test]
    fn parse_quoted_missing_close_returns_none() {
        assert!(parse_quoted_expression(r#""no close"#).is_none());
    }

    #[test]
    fn parse_quoted_no_quote_returns_none() {
        assert!(parse_quoted_expression("0 * * * * *").is_none());
    }

    #[test]
    fn build_notify_rejects_empty() {
        assert!(build_notify_action("   ").is_err());
    }

    #[test]
    fn build_notify_rejects_too_long() {
        let s = "x".repeat(MAX_NOTIFY_LEN + 1);
        assert!(build_notify_action(&s).is_err());
    }

    #[test]
    fn build_spawn_requires_prompt() {
        assert!(build_spawn_action("discovery").is_err());
        let act = build_spawn_action("discovery find docs").unwrap();
        match act {
            CronAction::Spawn { role, prompt } => {
                assert_eq!(role, "discovery");
                assert_eq!(prompt, "find docs");
            }
            _ => panic!("expected spawn"),
        }
    }

    #[test]
    fn format_list_empty_includes_help() {
        assert!(format_cron_list(&[]).contains("/cron-add"));
    }

    #[test]
    fn format_list_renders_table() {
        let job = CronJob::new(
            "c1".into(),
            "0 0 12 * * *".into(),
            CronAction::Notify { message: "lunch".into() },
        );
        let s = format_cron_list(&[job]);
        assert!(s.contains("c1"));
        assert!(s.contains("notify: lunch"));
    }
}
