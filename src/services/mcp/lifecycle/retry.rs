//! Exponential backoff con cap para reconexion de servers MCP.
//!
//! Secuencia (sin jitter): 2s, 4s, 8s, 16s, 32s, 60s (cap). El manager llama
//! [`next_delay`] con el contador de intentos post-fallo (>=1) y agenda el
//! reintento en segundo plano.

use std::time::Duration;

/// Cap maximo entre reintentos (1 min).
const MAX_DELAY_SECS: u64 = 60;
/// Delay base al primer fallo.
const BASE_DELAY_SECS: u64 = 2;

/// Calcula el delay para el siguiente reintento segun `attempts` (numero de
/// fallos acumulados, >=1). Nunca retorna Duration::ZERO.
pub fn next_delay(attempts: u32) -> Duration {
    let attempts = attempts.max(1);
    // 2 ^ (attempts - 1) * BASE, con saturacion para evitar overflow.
    let shift = (attempts - 1).min(16); // 2^16 = 65536, suficiente
    let raw = BASE_DELAY_SECS.saturating_mul(1u64 << shift);
    let clamped = raw.min(MAX_DELAY_SECS);
    Duration::from_secs(clamped)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_attempt_is_base() {
        assert_eq!(next_delay(1), Duration::from_secs(2));
    }

    #[test]
    fn doubles_each_attempt() {
        assert_eq!(next_delay(2), Duration::from_secs(4));
        assert_eq!(next_delay(3), Duration::from_secs(8));
        assert_eq!(next_delay(4), Duration::from_secs(16));
        assert_eq!(next_delay(5), Duration::from_secs(32));
    }

    #[test]
    fn capped_at_60s() {
        assert_eq!(next_delay(6), Duration::from_secs(60));
        assert_eq!(next_delay(10), Duration::from_secs(60));
        assert_eq!(next_delay(100), Duration::from_secs(60));
    }

    #[test]
    fn zero_attempts_treated_as_one() {
        assert_eq!(next_delay(0), Duration::from_secs(2));
    }

    #[test]
    fn no_overflow_on_huge_attempts() {
        // Previously could overflow if shift were unbounded.
        let d = next_delay(u32::MAX);
        assert_eq!(d, Duration::from_secs(60));
    }
}
