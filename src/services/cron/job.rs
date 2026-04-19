//! Tipos de cron job (E23).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Accion que dispara un cron al cumplirse su schedule. MVP Sprint 10:
/// notificacion + spawn de subagent. Sprint 11+ podria sumar workflows o
/// hooks externos.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CronAction {
    /// Toast informativo en el chat.
    Notify { message: String },
    /// Spawnea un subagent (E22a) con `role` y `prompt`.
    Spawn { role: String, prompt: String },
}

impl CronAction {
    #[allow(
        dead_code,
        reason = "consumido por widgets futuros (cron panel) y por logging diagnostico"
    )]
    pub fn label(&self) -> &'static str {
        match self {
            CronAction::Notify { .. } => "notify",
            CronAction::Spawn { .. } => "spawn",
        }
    }

    /// Resumen corto para tablas (≤ `width` chars).
    pub fn summary(&self, width: usize) -> String {
        let raw = match self {
            CronAction::Notify { message } => format!("notify: {message}"),
            CronAction::Spawn { role, prompt } => format!("spawn {role}: {prompt}"),
        };
        truncate_chars(&raw, width)
    }
}

fn truncate_chars(s: &str, width: usize) -> String {
    let trimmed = s.trim();
    if trimmed.chars().count() <= width {
        trimmed.to_string()
    } else {
        let head: String = trimmed.chars().take(width.saturating_sub(1)).collect();
        format!("{head}…")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CronJob {
    pub id: String,
    /// Expresion cron 6 o 7 campos (`sec min hour day month weekday [year]`).
    pub expression: String,
    pub action: CronAction,
    /// Si `false`, el scheduler lo ignora.
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    /// Cuantas veces se ha disparado.
    #[serde(default)]
    pub fire_count: u64,
    /// Ultimo timestamp UTC en el que el cron disparo.
    #[serde(default)]
    pub last_fired_at: Option<DateTime<Utc>>,
}

fn default_enabled() -> bool {
    true
}

impl CronJob {
    pub fn new(id: String, expression: String, action: CronAction) -> Self {
        Self { id, expression, action, enabled: true, fire_count: 0, last_fired_at: None }
    }

    /// Marca este job como disparado en `now` (avanza fire_count + last_fired_at).
    pub fn record_fired(&mut self, now: DateTime<Utc>) {
        self.fire_count = self.fire_count.saturating_add(1);
        self.last_fired_at = Some(now);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_summary_truncates() {
        let act = CronAction::Notify { message: "x".repeat(80) };
        assert!(act.summary(20).chars().count() <= 20);
    }

    #[test]
    fn action_label_matches_variant() {
        assert_eq!(CronAction::Notify { message: "m".into() }.label(), "notify");
        assert_eq!(CronAction::Spawn { role: "r".into(), prompt: "p".into() }.label(), "spawn");
    }

    #[test]
    fn record_fired_increments_count_and_sets_timestamp() {
        let mut job = CronJob::new(
            "c1".into(),
            "0 * * * * *".into(),
            CronAction::Notify { message: "hi".into() },
        );
        let now = Utc::now();
        job.record_fired(now);
        assert_eq!(job.fire_count, 1);
        assert_eq!(job.last_fired_at, Some(now));
    }

    #[test]
    fn job_serializes_without_optional_fields() {
        let job = CronJob::new(
            "c1".into(),
            "0 * * * * *".into(),
            CronAction::Notify { message: "ping".into() },
        );
        let s = serde_json::to_string(&job).unwrap();
        let restored: CronJob = serde_json::from_str(&s).unwrap();
        assert_eq!(restored.id, "c1");
        assert!(restored.enabled);
        assert_eq!(restored.fire_count, 0);
        assert!(restored.last_fired_at.is_none());
    }

    #[test]
    fn job_deserializes_with_default_enabled_true() {
        let raw =
            r#"{"id":"c1","expression":"0 * * * * *","action":{"kind":"notify","message":"hi"}}"#;
        let job: CronJob = serde_json::from_str(raw).unwrap();
        assert!(job.enabled);
    }
}
