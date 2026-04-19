//! Retry manager con exponential backoff para errores transitorios de la API.
//!
//! Clasifica errores por status code y decide si reintentar, con cuanto delay,
//! o rendirse. La decision final de *fallback* a otro modelo se delega en
//! `model_fallback::ModelFallbackChain`.
//!
//! Status codes reintentables por defecto: 408, 429, 500, 502, 503, 504, 529.
//! (429 = rate limit, 5xx = server-side, 529 = overload en Anthropic).

use std::time::Duration;

/// Configuracion del retry con valores por defecto razonables.
#[derive(Debug, Clone)]
pub struct RetryConfig {
    pub max_retries: u32,
    pub base_delay: Duration,
    pub max_delay: Duration,
    pub backoff_factor: f64,
    pub retryable_status_codes: &'static [u16],
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            base_delay: Duration::from_millis(500),
            max_delay: Duration::from_secs(30),
            backoff_factor: 2.0,
            retryable_status_codes: &[408, 429, 500, 502, 503, 504, 529],
        }
    }
}

/// Decision tomada por el `RetryManager` despues de un error.
#[derive(Debug, Clone, PartialEq)]
pub enum RetryDecision {
    /// Reintentar despues de `delay`. `attempt` es 1-indexed (1 = primer retry).
    Retry { attempt: u32, delay: Duration, reason: String },
    /// Rendirse. El error no es recuperable o se agotaron los intentos.
    GiveUp { reason: String },
}

/// Gestor de retry que mantiene el contador de intentos a lo largo de una request.
pub struct RetryManager {
    config: RetryConfig,
    attempt: u32,
}

impl RetryManager {
    pub fn new(config: RetryConfig) -> Self {
        Self { config, attempt: 0 }
    }

    /// Clasifica un error y decide la siguiente accion.
    pub fn on_error(&mut self, error_message: &str) -> RetryDecision {
        let status = extract_status_code(error_message);
        let is_retryable = status.is_some_and(|s| self.config.retryable_status_codes.contains(&s))
            || is_transient_network_error(error_message);

        if !is_retryable {
            return RetryDecision::GiveUp {
                reason: format!("Error no recuperable: {}", short_summary(error_message)),
            };
        }

        if self.attempt >= self.config.max_retries {
            return RetryDecision::GiveUp {
                reason: format!(
                    "Agotados {} reintentos ({})",
                    self.config.max_retries,
                    short_summary(error_message)
                ),
            };
        }

        self.attempt += 1;
        let delay = extract_retry_after(error_message)
            .unwrap_or_else(|| compute_delay(&self.config, self.attempt));
        let reason = retry_reason(status, error_message);
        RetryDecision::Retry { attempt: self.attempt, delay, reason }
    }

    /// Resetea el contador de intentos (al iniciar una nueva request del usuario).
    #[cfg_attr(not(test), allow(dead_code, reason = "API publica para reutilizar el manager"))]
    pub fn reset(&mut self) {
        self.attempt = 0;
    }

    pub fn config(&self) -> &RetryConfig {
        &self.config
    }
}

/// Calcula el delay para el intento `attempt` con backoff exponencial y cap.
fn compute_delay(config: &RetryConfig, attempt: u32) -> Duration {
    let base = config.base_delay.as_millis() as f64;
    let multiplier = config.backoff_factor.powi(attempt.saturating_sub(1) as i32);
    let millis = (base * multiplier).min(config.max_delay.as_millis() as f64);
    Duration::from_millis(millis as u64)
}

/// Extrae el valor de `retry-after` o `retry-after-ms` del texto del error.
///
/// El formato esperado es `[retry-after=N]` (segundos) o `[retry-after-ms=N]`
/// (milisegundos), embebido por `ClaudeProvider` antes de lanzar el error.
pub fn extract_retry_after(error: &str) -> Option<Duration> {
    if let Some(start) = error.find("[retry-after-ms=") {
        let rest = &error[start + "[retry-after-ms=".len()..];
        let end = rest.find(']')?;
        let ms: u64 = rest[..end].parse().ok()?;
        return Some(Duration::from_millis(ms.min(config_max_delay_ms())));
    }
    if let Some(start) = error.find("[retry-after=") {
        let rest = &error[start + "[retry-after=".len()..];
        let end = rest.find(']')?;
        let secs: u64 = rest[..end].parse().ok()?;
        return Some(Duration::from_secs(secs.min(config_max_delay_ms() / 1000)));
    }
    None
}

/// Techo para valores de Retry-After: reutiliza el max_delay del config default.
fn config_max_delay_ms() -> u64 {
    RetryConfig::default().max_delay.as_millis() as u64
}

/// Descripcion corta para mostrar en el spinner/notificacion.
fn retry_reason(status: Option<u16>, error: &str) -> String {
    match status {
        Some(429) => "rate limited".into(),
        Some(503) => "servicio no disponible".into(),
        Some(502 | 504) => "gateway timeout".into(),
        Some(529) => "sobrecarga del modelo".into(),
        Some(408) => "request timeout".into(),
        Some(500) => "error del servidor".into(),
        Some(code) => format!("HTTP {code}"),
        None => short_summary(error),
    }
}

/// Extrae un status code HTTP del texto del error. Busca el primer numero de
/// 3 digitos entre 100-599.
pub fn extract_status_code(error: &str) -> Option<u16> {
    let bytes = error.as_bytes();
    let mut i = 0;
    while i + 3 <= bytes.len() {
        if bytes[i].is_ascii_digit()
            && bytes[i + 1].is_ascii_digit()
            && bytes[i + 2].is_ascii_digit()
        {
            let is_boundary_start = i == 0 || !bytes[i - 1].is_ascii_digit();
            let is_boundary_end = i + 3 == bytes.len() || !bytes[i + 3].is_ascii_digit();
            if is_boundary_start && is_boundary_end {
                let code: u16 = (bytes[i] - b'0') as u16 * 100
                    + (bytes[i + 1] - b'0') as u16 * 10
                    + (bytes[i + 2] - b'0') as u16;
                if (100..=599).contains(&code) {
                    return Some(code);
                }
            }
        }
        i += 1;
    }
    None
}

/// Detecta errores de red transitorios en el texto del error (sin status code).
fn is_transient_network_error(error: &str) -> bool {
    let lc = error.to_lowercase();
    lc.contains("timeout")
        || lc.contains("timed out")
        || lc.contains("connection reset")
        || lc.contains("connection closed")
        || lc.contains("dns error")
        || lc.contains("temporarily unavailable")
        || lc.contains("empty response")
}

/// Recorta el error a una linea corta para mostrar al usuario.
fn short_summary(error: &str) -> String {
    let first_line = error.lines().next().unwrap_or(error);
    let trimmed = first_line.trim();
    if trimmed.chars().count() > 80 {
        let truncated: String = trimmed.chars().take(77).collect();
        format!("{truncated}...")
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_common_status_codes() {
        assert_eq!(extract_status_code("Claude API error: 429 Too Many Requests"), Some(429));
        assert_eq!(extract_status_code("Copilot chat failed: 503 Unavailable"), Some(503));
        assert_eq!(extract_status_code("HTTP 500 internal"), Some(500));
        assert_eq!(extract_status_code("error 529 overloaded"), Some(529));
        assert_eq!(extract_status_code("no status here"), None);
        // Rechaza numeros pegados a mas digitos (no son HTTP).
        assert_eq!(extract_status_code("code 1234"), None);
    }

    #[test]
    fn rate_limit_triggers_retry() {
        let mut mgr = RetryManager::new(RetryConfig::default());
        let decision = mgr.on_error("Claude API error: 429 rate limit");
        match decision {
            RetryDecision::Retry { attempt, reason, .. } => {
                assert_eq!(attempt, 1);
                assert!(reason.contains("rate limited"));
            }
            _ => panic!("expected Retry"),
        }
    }

    #[test]
    fn four_hundred_is_not_retryable() {
        let mut mgr = RetryManager::new(RetryConfig::default());
        let decision = mgr.on_error("Claude API error: 400 bad request");
        assert!(matches!(decision, RetryDecision::GiveUp { .. }));
    }

    #[test]
    fn max_retries_gives_up() {
        let mut mgr = RetryManager::new(RetryConfig::default());
        for _ in 0..3 {
            assert!(matches!(mgr.on_error("503 unavailable"), RetryDecision::Retry { .. }));
        }
        assert!(matches!(mgr.on_error("503 unavailable"), RetryDecision::GiveUp { .. }));
    }

    #[test]
    fn backoff_is_exponential_with_cap() {
        let config = RetryConfig::default();
        let d1 = compute_delay(&config, 1);
        let d2 = compute_delay(&config, 2);
        let d3 = compute_delay(&config, 3);
        assert_eq!(d1, Duration::from_millis(500));
        assert_eq!(d2, Duration::from_millis(1000));
        assert_eq!(d3, Duration::from_millis(2000));
        // Cap a max_delay
        let d_huge = compute_delay(&config, 20);
        assert!(d_huge <= config.max_delay);
    }

    #[test]
    fn network_timeout_treated_as_transient() {
        let mut mgr = RetryManager::new(RetryConfig::default());
        let decision = mgr.on_error("request timed out after 60s");
        assert!(matches!(decision, RetryDecision::Retry { .. }));
    }

    #[test]
    fn empty_response_is_retryable() {
        let mut mgr = RetryManager::new(RetryConfig::default());
        let decision = mgr.on_error("empty response: el provider no emitio contenido");
        assert!(matches!(decision, RetryDecision::Retry { .. }));
    }

    #[test]
    fn retry_after_secs_overrides_backoff() {
        let error = "Claude API error: 429 [retry-after=10] rate limited";
        let delay = extract_retry_after(error).expect("should parse retry-after");
        assert_eq!(delay, Duration::from_secs(10));

        let mut mgr = RetryManager::new(RetryConfig::default());
        match mgr.on_error(error) {
            RetryDecision::Retry { delay, .. } => {
                assert_eq!(delay, Duration::from_secs(10), "should use header, not backoff");
            }
            _ => panic!("expected Retry"),
        }
    }

    #[test]
    fn retry_after_ms_parsed_correctly() {
        let error = "Claude API error: 429 [retry-after-ms=5000]";
        let delay = extract_retry_after(error).expect("should parse retry-after-ms");
        assert_eq!(delay, Duration::from_millis(5000));
    }

    #[test]
    fn retry_after_absent_uses_backoff() {
        let error = "Claude API error: 429 no header";
        assert!(extract_retry_after(error).is_none());
        let mut mgr = RetryManager::new(RetryConfig::default());
        match mgr.on_error(error) {
            RetryDecision::Retry { delay, .. } => {
                assert_eq!(delay, Duration::from_millis(500), "first attempt = base_delay");
            }
            _ => panic!("expected Retry"),
        }
    }

    #[test]
    fn reset_zeroes_counter() {
        let mut mgr = RetryManager::new(RetryConfig::default());
        let _ = mgr.on_error("503");
        let _ = mgr.on_error("503");
        mgr.reset();
        let decision = mgr.on_error("503");
        match decision {
            RetryDecision::Retry { attempt, .. } => assert_eq!(attempt, 1),
            _ => panic!("expected Retry after reset"),
        }
    }
}
