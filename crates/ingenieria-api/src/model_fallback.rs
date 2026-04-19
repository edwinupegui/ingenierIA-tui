//! Cadena de fallback entre modelos cuando el primario acumula fallos.
//!
//! Referencia: claude-code fallback logic. El patron es: despues de N fallos
//! consecutivos del modelo primario, sugerir un modelo alternativo (tipicamente
//! mas barato o mas estable). La confirmacion del usuario ocurre en la capa UI.

/// Cadena de fallbacks: primero el primario, despues alternativas ordenadas.
#[derive(Debug, Clone)]
pub struct ModelFallbackChain {
    primary: String,
    fallbacks: Vec<String>,
    consecutive_failures: u32,
    threshold: u32,
    current_index: usize,
}

impl ModelFallbackChain {
    /// Crea una cadena con un modelo primario y una lista ordenada de fallbacks.
    /// `threshold` es el numero de fallos consecutivos antes de sugerir fallback.
    pub fn new(primary: impl Into<String>, fallbacks: Vec<String>, threshold: u32) -> Self {
        Self {
            primary: primary.into(),
            fallbacks,
            consecutive_failures: 0,
            threshold: threshold.max(1),
            current_index: 0,
        }
    }

    /// Registra un fallo y devuelve `Some(next_model)` si se alcanzo el threshold
    /// y existe un fallback disponible. `None` si aun no se llega al threshold
    /// o si se agotaron los fallbacks.
    pub fn record_failure(&mut self) -> Option<&str> {
        self.consecutive_failures += 1;
        if self.consecutive_failures < self.threshold {
            return None;
        }
        // Consumir un fallback
        if self.current_index < self.fallbacks.len() {
            let next = &self.fallbacks[self.current_index];
            self.current_index += 1;
            self.consecutive_failures = 0;
            return Some(next.as_str());
        }
        None
    }

    /// Resetea el contador tras un exito.
    pub fn record_success(&mut self) {
        self.consecutive_failures = 0;
    }

    /// Modelo primario original (para reintentar en el siguiente turno).
    #[allow(dead_code)]
    pub fn primary(&self) -> &str {
        &self.primary
    }

    /// Numero de fallos consecutivos acumulados.
    #[allow(dead_code)]
    pub fn consecutive_failures(&self) -> u32 {
        self.consecutive_failures
    }

    /// `true` si todavia hay fallbacks disponibles.
    #[allow(dead_code)]
    pub fn has_fallback_available(&self) -> bool {
        self.current_index < self.fallbacks.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn below_threshold_no_fallback() {
        let mut chain = ModelFallbackChain::new(
            "claude-sonnet-4-20250514",
            vec!["claude-haiku-4-5-20251001".into()],
            3,
        );
        assert!(chain.record_failure().is_none());
        assert!(chain.record_failure().is_none());
        assert_eq!(chain.consecutive_failures(), 2);
    }

    #[test]
    fn threshold_triggers_fallback() {
        let mut chain = ModelFallbackChain::new(
            "claude-sonnet-4-20250514",
            vec!["claude-haiku-4-5-20251001".into()],
            3,
        );
        chain.record_failure();
        chain.record_failure();
        let fallback = chain.record_failure();
        assert_eq!(fallback, Some("claude-haiku-4-5-20251001"));
        // Tras devolver un fallback, el contador se resetea.
        assert_eq!(chain.consecutive_failures(), 0);
    }

    #[test]
    fn fallbacks_are_exhaustible() {
        let mut chain = ModelFallbackChain::new("a", vec!["b".into(), "c".into()], 1);
        assert_eq!(chain.record_failure(), Some("b"));
        assert_eq!(chain.record_failure(), Some("c"));
        assert_eq!(chain.record_failure(), None);
        assert!(!chain.has_fallback_available());
    }

    #[test]
    fn success_resets_counter() {
        let mut chain = ModelFallbackChain::new("a", vec!["b".into()], 3);
        chain.record_failure();
        chain.record_failure();
        chain.record_success();
        assert_eq!(chain.consecutive_failures(), 0);
        assert!(chain.record_failure().is_none());
    }

    #[test]
    fn threshold_zero_is_clamped_to_one() {
        let mut chain = ModelFallbackChain::new("a", vec!["b".into()], 0);
        assert_eq!(chain.record_failure(), Some("b"));
    }
}
