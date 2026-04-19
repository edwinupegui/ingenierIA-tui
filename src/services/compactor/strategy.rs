//! Estrategias de compactacion con thresholds configurables.
//!
//! Tres perfiles:
//! - `Aggressive`: compacta temprano (>60%), preserva solo 4 mensajes recientes.
//! - `Balanced`: default — compacta a 80%, preserva 10 recientes.
//! - `Conservative`: compacta tarde (>90%), preserva 20 recientes (rica historia).
//!
//! Ver E14 del roadmap. Referencia: `claw-code/rust/crates/runtime/src/compact.rs`.

/// Perfil de compactacion. Afecta cuando disparar (threshold) y cuanto mantener.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CompactionStrategy {
    /// Compacta temprano, preserva poco. Para sesiones muy largas o contextos chicos.
    Aggressive,
    /// Equilibrio default: compacta a 80%, preserva 10 mensajes.
    #[default]
    Balanced,
    /// Compacta tarde, preserva mucha historia. Para trabajo que requiere continuidad.
    Conservative,
}

impl CompactionStrategy {
    /// Label corto para UI.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Aggressive => "aggressive",
            Self::Balanced => "balanced",
            Self::Conservative => "conservative",
        }
    }

    /// Parse desde string (case-insensitive). `None` si no matchea.
    pub fn from_label(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "aggressive" | "agr" | "a" => Some(Self::Aggressive),
            "balanced" | "bal" | "b" => Some(Self::Balanced),
            "conservative" | "con" | "c" => Some(Self::Conservative),
            _ => None,
        }
    }

    /// Parametros derivados de la estrategia.
    pub fn config(&self) -> CompactionConfig {
        match self {
            Self::Aggressive => CompactionConfig {
                trigger_percent: 60.0,
                keep_recent: 4,
                summary_budget_chars: 500,
            },
            Self::Balanced => CompactionConfig {
                trigger_percent: 80.0,
                keep_recent: 10,
                summary_budget_chars: 1500,
            },
            Self::Conservative => CompactionConfig {
                trigger_percent: 90.0,
                keep_recent: 20,
                summary_budget_chars: 3000,
            },
        }
    }
}

/// Parametros resueltos a partir de la estrategia.
#[derive(Debug, Clone, Copy)]
pub struct CompactionConfig {
    /// % contexto a partir del cual auto-compact se dispara.
    pub trigger_percent: f64,
    /// Cantidad minima de mensajes recientes (no-system) a preservar.
    pub keep_recent: usize,
    /// Presupuesto de chars para el resumen (truncacion al final).
    pub summary_budget_chars: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_label_matches_variants_and_aliases() {
        assert_eq!(
            CompactionStrategy::from_label("aggressive"),
            Some(CompactionStrategy::Aggressive)
        );
        assert_eq!(CompactionStrategy::from_label("A"), Some(CompactionStrategy::Aggressive));
        assert_eq!(CompactionStrategy::from_label("bal"), Some(CompactionStrategy::Balanced));
        assert_eq!(CompactionStrategy::from_label("CON"), Some(CompactionStrategy::Conservative));
        assert_eq!(CompactionStrategy::from_label("wat"), None);
    }

    #[test]
    fn config_monotonic_by_strategy() {
        let agr = CompactionStrategy::Aggressive.config();
        let bal = CompactionStrategy::Balanced.config();
        let con = CompactionStrategy::Conservative.config();
        assert!(agr.trigger_percent < bal.trigger_percent);
        assert!(bal.trigger_percent < con.trigger_percent);
        assert!(agr.keep_recent < bal.keep_recent);
        assert!(bal.keep_recent < con.keep_recent);
    }

    #[test]
    fn default_is_balanced() {
        assert_eq!(CompactionStrategy::default(), CompactionStrategy::Balanced);
    }
}
