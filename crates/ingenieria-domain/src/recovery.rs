//! Recovery recipes (E42) — escenarios de fallo conocidos con recipes
//! estructuradas para que el TUI reaccione de forma predecible.
//!
//! Los tipos de este modulo son solo dominio: definen **que** escenarios
//! existen y **que** pasos secuenciales aplica cada recipe. La ejecucion
//! concreta vive en `services/recovery_engine.rs` (que decide cuando
//! notificar, reintentar, o escalar al humano).
//!
//! Referencia: `claw-code rust/crates/runtime/src/recovery_recipes.rs` (631 LOC).
//! Se adopta un subset enfocado en los fallos que el TUI ya taxonomiza en
//! `domain::failure::FailureCategory` + escenarios del file watcher.

use std::time::Duration;

use super::failure::{FailureCategory, StructuredFailure};

/// Escenarios de fallo con recovery definido. Derivado de `FailureCategory`
/// pero mas especifico: aqui importa **como** se recupera, no como se
/// categoriza para el usuario.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(
    dead_code,
    reason = "SessionCorrupted y WorkerCrash son API publica para futuros handlers (session resume + worker watchdog)"
)]
pub enum FailureScenario {
    /// Provider no responde (429, 5xx, timeout).
    ProviderTimeout,
    /// Provider devolvio 429 Too Many Requests explicito.
    ProviderRateLimit,
    /// Credenciales expiradas/invalidas (401/403).
    ProviderAuthExpired,
    /// MCP server no conecto (stdio/SSE/WS handshake fallo).
    McpHandshakeFailure,
    /// Tool call MCP no respondio dentro del timeout.
    McpToolTimeout,
    /// Config file invalido (JSON malformado, schema incorrecto).
    ConfigInvalid,
    /// Session JSONL corrupto (parse fallo al resume).
    SessionCorrupted,
    /// Worker mpsc cayo inesperadamente — se pasa el nombre.
    WorkerCrash(String),
    /// No hay espacio en disco para logs/sesiones.
    DiskFull,
}

/// Paso individual de una recipe. Las recipes son `Vec<RecoveryStep>` que
/// el engine ejecuta en orden hasta que una tiene exito o se agota.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code, reason = "variantes consumidas por engine + futuros handlers")]
pub enum RecoveryStep {
    /// Reintentar con exponential backoff.
    RetryWithBackoff { max_retries: u32, base_delay: Duration },
    /// Reiniciar un worker por nombre.
    RestartWorker(String),
    /// Recargar la config desde disco.
    ReloadConfig,
    /// Cambiar al modelo de fallback.
    SwitchToFallback,
    /// Mostrar toast al usuario con el mensaje dado.
    NotifyUser(String),
    /// Requiere intervencion manual del humano.
    EscalateToHuman,
}

/// Politica de escalado si la recipe se agota.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(
    dead_code,
    reason = "Abort reservado para escenarios que detendran operacion en curso (ej. DiskFull durante escritura)"
)]
pub enum EscalationPolicy {
    /// Registrar fallo y continuar funcionando (degraded).
    LogAndContinue,
    /// Mostrar toast bloqueante al usuario.
    AlertHuman,
    /// Detener la operacion actual.
    Abort,
}

/// Numero maximo de retries encadenados por recipe para evitar loops.
pub const MAX_RECIPE_RETRIES: u32 = 3;

/// Mapeo scenario → pasos de la recipe.
///
/// La recipe es determinista y sin IO: retornar solo la "intencion", no
/// ejecuta nada. El engine se encarga de ejecutar con `tokio::spawn`.
pub fn recipe_for(scenario: &FailureScenario) -> Vec<RecoveryStep> {
    match scenario {
        FailureScenario::ProviderTimeout => vec![
            RecoveryStep::RetryWithBackoff {
                max_retries: MAX_RECIPE_RETRIES,
                base_delay: Duration::from_secs(2),
            },
            RecoveryStep::NotifyUser("Reintentando conexion con el provider…".into()),
            RecoveryStep::SwitchToFallback,
        ],
        FailureScenario::ProviderRateLimit => vec![
            RecoveryStep::RetryWithBackoff {
                max_retries: MAX_RECIPE_RETRIES,
                base_delay: Duration::from_secs(10),
            },
            RecoveryStep::NotifyUser("Rate limit alcanzado, esperando ventana…".into()),
        ],
        FailureScenario::ProviderAuthExpired => vec![
            RecoveryStep::NotifyUser("Credenciales expiradas — re-autenticar con /config".into()),
            RecoveryStep::EscalateToHuman,
        ],
        FailureScenario::McpHandshakeFailure => vec![
            RecoveryStep::RetryWithBackoff { max_retries: 2, base_delay: Duration::from_secs(5) },
            RecoveryStep::NotifyUser(
                "MCP server no disponible, continuando sin tools externas".into(),
            ),
        ],
        FailureScenario::McpToolTimeout => {
            vec![RecoveryStep::NotifyUser("Tool MCP tardo demasiado — abortando ronda".into())]
        }
        FailureScenario::ConfigInvalid => vec![
            RecoveryStep::NotifyUser(
                "Config invalida — usando defaults; revisa ~/.config/ingenieria-tui/".into(),
            ),
            RecoveryStep::ReloadConfig,
        ],
        FailureScenario::SessionCorrupted => vec![RecoveryStep::NotifyUser(
            "Session corrupto — iniciando nueva sesion (el archivo sigue en disco)".into(),
        )],
        FailureScenario::WorkerCrash(name) => vec![
            RecoveryStep::RestartWorker(name.clone()),
            RecoveryStep::NotifyUser(format!("Worker '{name}' reiniciado")),
        ],
        FailureScenario::DiskFull => vec![
            RecoveryStep::NotifyUser("Disco lleno — deteniendo escrituras a log y sesiones".into()),
            RecoveryStep::EscalateToHuman,
        ],
    }
}

/// Politica de escalado por escenario.
pub fn escalation_for(scenario: &FailureScenario) -> EscalationPolicy {
    match scenario {
        FailureScenario::ProviderTimeout
        | FailureScenario::ProviderRateLimit
        | FailureScenario::McpHandshakeFailure
        | FailureScenario::McpToolTimeout
        | FailureScenario::ConfigInvalid
        | FailureScenario::SessionCorrupted
        | FailureScenario::WorkerCrash(_) => EscalationPolicy::LogAndContinue,
        FailureScenario::ProviderAuthExpired | FailureScenario::DiskFull => {
            EscalationPolicy::AlertHuman
        }
    }
}

/// Heuristica que mapea un `StructuredFailure` (E13) a un escenario (E42)
/// cuando aplica. Retorna `None` si la categoria no encaja con ningun
/// recovery conocido (el caller solo muestra el hint generico de E13).
pub fn scenario_from_failure(failure: &StructuredFailure) -> Option<FailureScenario> {
    match failure.category {
        FailureCategory::PromptDelivery => {
            if failure.status_code == Some(429) {
                Some(FailureScenario::ProviderRateLimit)
            } else {
                Some(FailureScenario::ProviderTimeout)
            }
        }
        FailureCategory::StreamTimeout => Some(FailureScenario::ProviderTimeout),
        FailureCategory::ApiKeyInvalid => Some(FailureScenario::ProviderAuthExpired),
        FailureCategory::McpTimeout => Some(FailureScenario::McpToolTimeout),
        FailureCategory::McpError => Some(FailureScenario::McpHandshakeFailure),
        FailureCategory::ParseError => Some(FailureScenario::ConfigInvalid),
        FailureCategory::IoError => {
            if failure.message.to_lowercase().contains("no space") {
                Some(FailureScenario::DiskFull)
            } else {
                None
            }
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recipe_for_provider_timeout_has_retry_and_fallback() {
        let r = recipe_for(&FailureScenario::ProviderTimeout);
        assert!(matches!(r.first(), Some(RecoveryStep::RetryWithBackoff { .. })));
        assert!(r.iter().any(|s| matches!(s, RecoveryStep::SwitchToFallback)));
    }

    #[test]
    fn recipe_for_rate_limit_uses_longer_delay() {
        let r = recipe_for(&FailureScenario::ProviderRateLimit);
        match r.first() {
            Some(RecoveryStep::RetryWithBackoff { base_delay, .. }) => {
                assert!(base_delay.as_secs() >= 10);
            }
            other => panic!("expected RetryWithBackoff, got {other:?}"),
        }
    }

    #[test]
    fn recipe_for_worker_crash_includes_name() {
        let r = recipe_for(&FailureScenario::WorkerCrash("sse".into()));
        let contains_name =
            r.iter().any(|s| matches!(s, RecoveryStep::RestartWorker(n) if n == "sse"));
        assert!(contains_name);
    }

    #[test]
    fn recipe_for_auth_expired_escalates() {
        let r = recipe_for(&FailureScenario::ProviderAuthExpired);
        assert!(r.iter().any(|s| matches!(s, RecoveryStep::EscalateToHuman)));
    }

    #[test]
    fn escalation_for_disk_full_alerts_human() {
        assert_eq!(escalation_for(&FailureScenario::DiskFull), EscalationPolicy::AlertHuman);
    }

    #[test]
    fn escalation_for_provider_timeout_logs_and_continues() {
        assert_eq!(
            escalation_for(&FailureScenario::ProviderTimeout),
            EscalationPolicy::LogAndContinue
        );
    }

    #[test]
    fn scenario_from_failure_rate_limit_on_429() {
        let mut f = StructuredFailure::from_error("http 429");
        f.category = FailureCategory::PromptDelivery;
        f.status_code = Some(429);
        assert_eq!(scenario_from_failure(&f), Some(FailureScenario::ProviderRateLimit));
    }

    #[test]
    fn scenario_from_failure_api_key_invalid() {
        let f = StructuredFailure::new(FailureCategory::ApiKeyInvalid, "401");
        assert_eq!(scenario_from_failure(&f), Some(FailureScenario::ProviderAuthExpired));
    }

    #[test]
    fn scenario_from_failure_disk_full_detected_from_io_message() {
        let f = StructuredFailure::new(FailureCategory::IoError, "write failed: no space left");
        assert_eq!(scenario_from_failure(&f), Some(FailureScenario::DiskFull));
    }

    #[test]
    fn scenario_from_failure_trust_denied_has_no_recipe() {
        let f = StructuredFailure::new(FailureCategory::TrustDenied, "user said no");
        assert_eq!(scenario_from_failure(&f), None);
    }

    #[test]
    fn max_recipe_retries_is_bounded() {
        const { assert!(MAX_RECIPE_RETRIES <= 5, "no loops infinitos permitidos") }
    }
}
