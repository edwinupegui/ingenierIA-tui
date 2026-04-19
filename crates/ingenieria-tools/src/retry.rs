//! Retry policy para ejecución de tool calls.
//!
//! Un tool call puede fallar transitivamente (red caída mid-call, MCP
//! desconecta, timeout). La policy clasifica el resultado como retryable o
//! terminal y, si aplica, indica cuánto esperar antes de reintentar.
//!
//! El caller (p.ej. `chat_tools::execute_pending_tool_calls`) decide cuándo
//! invocar `decide()` sobre el String de resultado y cuándo hacer sleep +
//! reintentar. Este módulo no ejecuta el retry: solo provee la política.

use std::time::Duration;

#[derive(Debug, Clone, Copy)]
pub struct ToolRetryPolicy {
    pub max_attempts: u32,
    pub base_backoff: Duration,
}

impl Default for ToolRetryPolicy {
    fn default() -> Self {
        Self { max_attempts: 2, base_backoff: Duration::from_millis(500) }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetryDecision {
    Retry { delay: Duration, attempt: u32 },
    GiveUp,
    Ok,
}

impl ToolRetryPolicy {
    /// Decide la siguiente acción dada la salida del tool y los intentos
    /// hechos hasta ahora (0 en la primera llamada).
    pub fn decide(&self, result: &str, attempts: u32) -> RetryDecision {
        if !looks_like_error(result) {
            return RetryDecision::Ok;
        }
        if !is_retryable(result) || attempts + 1 >= self.max_attempts {
            return RetryDecision::GiveUp;
        }
        let multiplier = 1u32 << attempts.min(8);
        let delay = self.base_backoff.saturating_mul(multiplier);
        RetryDecision::Retry { delay, attempt: attempts + 1 }
    }
}

fn looks_like_error(s: &str) -> bool {
    let lower = s.trim_start().to_ascii_lowercase();
    lower.starts_with("error")
        || lower.starts_with("mcp error")
        || lower.starts_with("mcp connection failed")
}

fn is_retryable(s: &str) -> bool {
    let lower = s.to_ascii_lowercase();
    const RETRYABLE_PHRASES: &[&str] = &[
        "timeout",
        "timed out",
        "connection reset",
        "connection refused",
        "connection failed",
        "broken pipe",
        "mcp connection failed",
        "mcp error",
        "temporarily unavailable",
        "service unavailable",
        "502 bad gateway",
        "503",
        "504 gateway timeout",
    ];
    RETRYABLE_PHRASES.iter().any(|p| lower.contains(p))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn success_is_ok() {
        let p = ToolRetryPolicy::default();
        assert_eq!(p.decide("OK: escrito", 0), RetryDecision::Ok);
    }

    #[test]
    fn non_retryable_error_gives_up() {
        let p = ToolRetryPolicy::default();
        assert_eq!(p.decide("Error: FileNotFound", 0), RetryDecision::GiveUp);
    }

    #[test]
    fn retryable_first_attempt_retries() {
        let p = ToolRetryPolicy::default();
        match p.decide("Error: timeout", 0) {
            RetryDecision::Retry { attempt, delay } => {
                assert_eq!(attempt, 1);
                assert_eq!(delay, Duration::from_millis(500));
            }
            other => panic!("expected Retry, got {other:?}"),
        }
    }

    #[test]
    fn retryable_last_attempt_gives_up() {
        let p = ToolRetryPolicy::default();
        assert_eq!(p.decide("MCP connection failed", 1), RetryDecision::GiveUp);
    }

    #[test]
    fn backoff_grows_exponentially() {
        let p = ToolRetryPolicy { max_attempts: 10, base_backoff: Duration::from_millis(100) };
        match p.decide("Error: timeout", 2) {
            RetryDecision::Retry { delay, .. } => {
                assert_eq!(delay, Duration::from_millis(400));
            }
            _ => panic!(),
        }
    }

    #[test]
    fn mcp_errors_are_retryable() {
        assert!(is_retryable("MCP error: Timeout after 5s"));
        assert!(is_retryable("MCP connection failed"));
    }

    #[test]
    fn invalid_args_not_retryable() {
        assert!(!is_retryable("invalid arguments"));
    }
}
