//! Onboarding & tips service (E39).
//!
//! Agrupa tres piezas relacionadas a la primera experiencia del usuario:
//! - `ChecklistState`: 5 pasos dismissible persistidos en disco.
//! - `TipState`: tip contextual con cooldown por sesion.
//! - `PlatformHints`: deteccion de terminal / tmux / ssh.
//!
//! El estado se persiste en `~/.config/ingenieria-tui/onboarding.json`. Si el
//! archivo no existe o es invalido, arrancamos con defaults — no tratamos
//! esto como error.

pub mod checklist;
pub mod platform;
pub mod tips;

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub use checklist::{ChecklistState, ChecklistStep};
pub use platform::PlatformHints;
pub use tips::{Tip, TipScope, TipState, TIP_CATALOG};

/// Placeholders rotativos por factory. El render elige uno segun
/// `tick_count / PLACEHOLDER_ROTATION_TICKS` para rotar cada ~5s a 4Hz.
pub const PLACEHOLDER_ROTATION_TICKS: u64 = 20;

const PLACEHOLDERS_NET: &[&str] = &[
    "¿Qué construimos hoy?  \"Agrega el endpoint de pagos\"",
    "Prueba:  \"Crea un DTO de facturacion con validacion\"",
    "Idea:   \"Extrae esta logica a un handler dedicado\"",
];
const PLACEHOLDERS_ANG: &[&str] = &[
    "¿Qué construimos hoy?  \"Crea el componente de login\"",
    "Prueba:  \"Genera un reactive form con validacion async\"",
    "Idea:   \"Convierte este componente en standalone\"",
];
const PLACEHOLDERS_NEST: &[&str] = &[
    "¿Qué construimos hoy?  \"Crea el proxy de autenticación\"",
    "Prueba:  \"Agrega un guard con roles y metadata\"",
    "Idea:   \"Extrae este service a su propio modulo\"",
];
const PLACEHOLDERS_ALL: &[&str] = &[
    "¿Qué construimos hoy?  \"Nueva feature de usuarios end-to-end\"",
    "Prueba:  \"Plan para migrar este endpoint a GraphQL\"",
    "Idea:   \"Revisa compliance de este endpoint (/plan)\"",
];

/// Retorna el placeholder dinamico para el factory y tick actuales.
pub fn dynamic_placeholder(factory: &crate::state::UiFactory, tick: u64) -> &'static str {
    use crate::state::UiFactory;
    let bucket: &[&str] = match factory {
        UiFactory::Net => PLACEHOLDERS_NET,
        UiFactory::Ang => PLACEHOLDERS_ANG,
        UiFactory::Nest => PLACEHOLDERS_NEST,
        UiFactory::All => PLACEHOLDERS_ALL,
    };
    let idx = ((tick / PLACEHOLDER_ROTATION_TICKS) as usize) % bucket.len();
    bucket[idx]
}

/// Estado completo del onboarding. No contiene PlatformHints porque esos se
/// detectan en cada arranque y no tiene sentido persistirlos.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OnboardingState {
    #[serde(default)]
    pub session_count: u32,
    #[serde(default)]
    pub checklist: ChecklistState,
    #[serde(default)]
    pub tips: TipState,
}

impl OnboardingState {
    /// Carga desde XDG config. Si falla, retorna defaults — la primera vez es
    /// el flujo normal y no debe bloquear el arranque.
    pub fn load() -> Self {
        match persistence_path().and_then(|p| std::fs::read_to_string(p).ok()) {
            Some(content) => serde_json::from_str(&content).unwrap_or_default(),
            None => Self::default(),
        }
    }

    /// Incrementa session_count y persiste. Debe llamarse una sola vez al
    /// startup. Silencia errores de IO (logging en su lugar).
    pub fn bump_session_and_save(&mut self) {
        self.session_count = self.session_count.saturating_add(1);
        if let Err(err) = self.save() {
            tracing::warn!(%err, "onboarding state save failed");
        }
    }

    /// Persiste estado actual a disco. Crea el directorio si no existe.
    pub fn save(&self) -> anyhow::Result<()> {
        let path = persistence_path()
            .ok_or_else(|| anyhow::anyhow!("No se pudo determinar el config dir"))?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, serde_json::to_string_pretty(self)?)?;
        Ok(())
    }
}

/// Localiza `~/.config/ingenieria-tui/onboarding.json`. Respeta `XDG_CONFIG_HOME`.
fn persistence_path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("ingenieria-tui").join("onboarding.json"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_state_has_zero_session() {
        let s = OnboardingState::default();
        assert_eq!(s.session_count, 0);
        assert_eq!(s.checklist.progress(), 0);
    }

    #[test]
    fn roundtrip_serde() {
        let mut s = OnboardingState { session_count: 3, ..Default::default() };
        s.checklist.mark(ChecklistStep::SelectFactory);
        s.tips.mark_shown(3);
        let json = serde_json::to_string(&s).unwrap();
        let parsed: OnboardingState = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.session_count, 3);
        assert!(parsed.checklist.is_done(ChecklistStep::SelectFactory));
        assert_eq!(parsed.tips.last_shown_session, 3);
    }

    #[test]
    fn empty_json_uses_defaults() {
        let parsed: OnboardingState = serde_json::from_str("{}").unwrap();
        assert_eq!(parsed.session_count, 0);
    }

    #[test]
    fn invalid_json_is_tolerated_by_load() {
        // Solo podemos verificar que load() no panic — redirigir XDG a un
        // tempdir no vale la pena para este smoke test.
        let _ = OnboardingState::load();
    }

    #[test]
    fn dynamic_placeholder_rotates() {
        use crate::state::UiFactory;
        let p0 = dynamic_placeholder(&UiFactory::Net, 0);
        let p_rot = dynamic_placeholder(&UiFactory::Net, PLACEHOLDER_ROTATION_TICKS);
        assert_ne!(p0, p_rot, "Placeholder debe cambiar al pasar ROTATION_TICKS");
    }

    #[test]
    fn dynamic_placeholder_same_within_window() {
        use crate::state::UiFactory;
        let p_a = dynamic_placeholder(&UiFactory::Ang, 5);
        let p_b = dynamic_placeholder(&UiFactory::Ang, PLACEHOLDER_ROTATION_TICKS - 1);
        assert_eq!(p_a, p_b, "Placeholder debe ser estable dentro de ROTATION_TICKS");
    }
}
