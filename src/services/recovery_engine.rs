//! Recovery engine (E42) — interpreta recipes y decide que Actions emitir.
//!
//! El engine es un **mapper** sin estado mutable: dado un
//! `StructuredFailure` (E13) o un `FailureScenario`, resuelve la recipe
//! desde `domain::recovery` y devuelve los toasts / acciones que el handler
//! principal debe disparar.
//!
//! Por que no tiene estado: el retry con backoff ya lo implementa
//! `services::chat::retry::RetryManager` (E13). Este engine complementa
//! con cobertura para escenarios no-chat (MCP handshake, config invalido,
//! worker crash, disk full) y sirve como fuente unica de "que le digo al
//! usuario cuando X falla".

use crate::domain::failure::StructuredFailure;
use crate::domain::recovery::{
    escalation_for, recipe_for, scenario_from_failure, EscalationPolicy, FailureScenario,
    RecoveryStep,
};
use crate::state::ToastLevel;

/// Plan resuelto por el engine para un escenario. El caller decide como
/// dispararlo (toasts, audit log, Actions).
#[derive(Debug, Clone)]
pub struct RecoveryPlan {
    pub scenario: FailureScenario,
    pub steps: Vec<RecoveryStep>,
    pub escalation: EscalationPolicy,
    /// Mensajes humanos listos para toast (derivado de `RecoveryStep::NotifyUser`).
    pub user_messages: Vec<String>,
    /// Label agregado para logs / audit.
    pub summary: String,
}

impl RecoveryPlan {
    /// Nivel de toast recomendado segun la escalation policy.
    pub fn toast_level(&self) -> ToastLevel {
        match self.escalation {
            EscalationPolicy::LogAndContinue => ToastLevel::Info,
            EscalationPolicy::AlertHuman => ToastLevel::Warning,
            EscalationPolicy::Abort => ToastLevel::Error,
        }
    }
}

/// Resuelve un plan a partir de un `StructuredFailure`. Retorna `None` si
/// el fallo no mapea a un escenario conocido — en ese caso el caller solo
/// muestra el recovery_hint generico de E13.
pub fn plan_for_failure(failure: &StructuredFailure) -> Option<RecoveryPlan> {
    let scenario = scenario_from_failure(failure)?;
    Some(plan_for_scenario(scenario))
}

/// Resuelve un plan dado un scenario explicito (p.ej. desde un worker crash).
pub fn plan_for_scenario(scenario: FailureScenario) -> RecoveryPlan {
    let steps = recipe_for(&scenario);
    let escalation = escalation_for(&scenario);
    let user_messages: Vec<String> = steps
        .iter()
        .filter_map(|s| match s {
            RecoveryStep::NotifyUser(msg) => Some(msg.clone()),
            _ => None,
        })
        .collect();
    let summary = format!("{scenario:?} → {} paso(s)", steps.len());
    RecoveryPlan { scenario, steps, escalation, user_messages, summary }
}

/// Dado un plan, reduce a un unico mensaje representativo para mostrar al
/// usuario (el primero de los notify, o una etiqueta si no hay ninguno).
pub fn headline_message(plan: &RecoveryPlan) -> String {
    plan.user_messages.first().cloned().unwrap_or_else(|| format!("⚠ Recovery: {}", plan.summary))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::failure::FailureCategory;

    #[test]
    fn plan_for_failure_none_when_no_scenario() {
        let f = StructuredFailure::new(FailureCategory::TrustDenied, "user denied");
        assert!(plan_for_failure(&f).is_none());
    }

    #[test]
    fn plan_for_failure_maps_api_key() {
        let f = StructuredFailure::new(FailureCategory::ApiKeyInvalid, "401");
        let plan = plan_for_failure(&f).expect("should map to ProviderAuthExpired");
        assert_eq!(plan.scenario, FailureScenario::ProviderAuthExpired);
        assert!(!plan.user_messages.is_empty());
    }

    #[test]
    fn toast_level_for_alert_is_warning() {
        let plan = plan_for_scenario(FailureScenario::ProviderAuthExpired);
        assert_eq!(plan.toast_level(), ToastLevel::Warning);
    }

    #[test]
    fn toast_level_for_log_is_info() {
        let plan = plan_for_scenario(FailureScenario::ProviderTimeout);
        assert_eq!(plan.toast_level(), ToastLevel::Info);
    }

    #[test]
    fn headline_message_uses_first_notify() {
        let plan = plan_for_scenario(FailureScenario::McpHandshakeFailure);
        let hl = headline_message(&plan);
        assert!(hl.to_lowercase().contains("mcp"));
    }

    #[test]
    fn headline_message_falls_back_when_no_notify() {
        // Escenario que (en practica) siempre trae notify, pero simulamos
        // construyendo un plan manual sin user_messages para probar fallback.
        let plan = RecoveryPlan {
            scenario: FailureScenario::DiskFull,
            steps: vec![RecoveryStep::EscalateToHuman],
            escalation: EscalationPolicy::AlertHuman,
            user_messages: Vec::new(),
            summary: "test".to_string(),
        };
        assert!(headline_message(&plan).starts_with("⚠ Recovery"));
    }

    #[test]
    fn plan_for_scenario_worker_crash_has_restart_step() {
        let plan = plan_for_scenario(FailureScenario::WorkerCrash("sse".into()));
        assert!(plan.steps.iter().any(|s| matches!(s, RecoveryStep::RestartWorker(_))));
    }
}
