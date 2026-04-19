//! Checklist progresivo de primera ejecucion (E39).
//!
//! 5 pasos que guian al usuario a descubrir la TUI. Persistente a traves de
//! sesiones: una vez marcado un paso como completado, se queda. El checklist
//! se auto-oculta despues de 4 visualizaciones con todos los pasos hechos o si
//! el usuario lo dismisea explicitamente.

use serde::{Deserialize, Serialize};

/// Numero maximo de veces que el checklist se muestra automaticamente una vez
/// completo. Despues pasa a estado dismissed.
pub const CHECKLIST_MAX_VIEWS: u32 = 4;

/// Ticks de espera antes de auto-dismiss al completar todos los pasos (3s a 4Hz).
const CHECKLIST_DISMISS_TICKS: u64 = 12;

/// Identificador estable de cada paso (serializable). El orden en el array
/// `ALL_STEPS` determina el orden visual.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChecklistStep {
    ConfigureServer,
    SelectFactory,
    FirstChat,
    ExploreDashboard,
    Personalize,
}

impl ChecklistStep {
    /// Lista canonica en orden de presentacion.
    pub const ALL: [ChecklistStep; 5] = [
        ChecklistStep::ConfigureServer,
        ChecklistStep::SelectFactory,
        ChecklistStep::FirstChat,
        ChecklistStep::ExploreDashboard,
        ChecklistStep::Personalize,
    ];

    /// Titulo corto mostrado en el widget.
    pub fn label(&self) -> &'static str {
        match self {
            ChecklistStep::ConfigureServer => "Configurar proveedor AI",
            ChecklistStep::SelectFactory => "Seleccionar factory",
            ChecklistStep::FirstChat => "Enviar primer chat",
            ChecklistStep::ExploreDashboard => "Explorar dashboard",
            ChecklistStep::Personalize => "Personalizar tema",
        }
    }

    /// Hint de como completar este paso — se muestra a la derecha del label.
    pub fn hint(&self) -> &'static str {
        match self {
            ChecklistStep::ConfigureServer => "se completa tras el wizard",
            ChecklistStep::SelectFactory => "Tab cambia contexto (Net/Ang/Nest/All)",
            ChecklistStep::FirstChat => "escribe y Enter en la pantalla de chat",
            ChecklistStep::ExploreDashboard => "':' → dashboard en paleta de comandos",
            ChecklistStep::Personalize => "':' → theme en paleta de comandos",
        }
    }
}

/// Estado persistente del checklist.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ChecklistState {
    /// Pasos completados (persistidos como Vec para JSON simple).
    #[serde(default)]
    pub completed: Vec<ChecklistStep>,
    /// Numero de veces que el checklist lleno (todos los pasos) ha sido visto.
    /// Solo se incrementa cuando `completed == ALL` y el usuario visita splash.
    #[serde(default)]
    pub full_views: u32,
    /// Dismiss manual (permanente hasta `reset`).
    #[serde(default)]
    pub dismissed: bool,
    /// Tick en el que el checklist se auto-dimissea (transient, no se persiste).
    #[serde(skip)]
    pub dismiss_at_tick: Option<u64>,
}

impl ChecklistState {
    /// Retorna `true` si el paso ya esta marcado como completado.
    pub fn is_done(&self, step: ChecklistStep) -> bool {
        self.completed.contains(&step)
    }

    /// Marca un paso como completado (idempotente). Retorna `true` si fue una
    /// transicion (para que el llamador pueda emitir un toast).
    pub fn mark(&mut self, step: ChecklistStep) -> bool {
        if self.is_done(step) {
            return false;
        }
        self.completed.push(step);
        true
    }

    /// Numero de pasos completados.
    pub fn progress(&self) -> usize {
        self.completed.iter().filter(|s| ChecklistStep::ALL.contains(s)).count()
    }

    /// `true` si el checklist debe mostrarse en splash. Se oculta si el usuario
    /// lo dismisso o si ya tuvimos `CHECKLIST_MAX_VIEWS` visualizaciones completas.
    pub fn should_display(&self) -> bool {
        if self.dismissed {
            return false;
        }
        self.full_views < CHECKLIST_MAX_VIEWS
    }

    /// Registra una visualizacion del checklist estando completo. No-op si no
    /// esta completo (para que visualizaciones parciales no cuenten contra el cap).
    pub fn record_view(&mut self) {
        if self.progress() == ChecklistStep::ALL.len() {
            self.full_views = self.full_views.saturating_add(1);
        }
    }

    /// Inicia el countdown de auto-dismiss si todos los pasos estan completos.
    /// No-op si ya hay un countdown activo o el checklist ya fue dismissed.
    pub fn start_dismiss_countdown(&mut self, current_tick: u64) {
        if self.dismiss_at_tick.is_none()
            && !self.dismissed
            && self.progress() == ChecklistStep::ALL.len()
        {
            self.dismiss_at_tick = Some(current_tick + CHECKLIST_DISMISS_TICKS);
        }
    }

    /// Chequea si el countdown expiro y dismissea. Retorna `true` si se hizo
    /// dismiss (para que el caller persista).
    pub fn check_auto_dismiss(&mut self, current_tick: u64) -> bool {
        if let Some(deadline) = self.dismiss_at_tick {
            if current_tick >= deadline {
                self.dismissed = true;
                self.dismiss_at_tick = None;
                return true;
            }
        }
        false
    }

    /// Dismiss explicito (persistente).
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "E39 — expuesto para slash command futuro (/onboarding-skip) sin handler aun"
        )
    )]
    pub fn dismiss(&mut self) {
        self.dismissed = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_state_is_empty() {
        let s = ChecklistState::default();
        assert_eq!(s.progress(), 0);
        assert!(!s.dismissed);
        assert!(s.should_display());
    }

    #[test]
    fn mark_is_idempotent() {
        let mut s = ChecklistState::default();
        assert!(s.mark(ChecklistStep::SelectFactory));
        assert!(!s.mark(ChecklistStep::SelectFactory));
        assert_eq!(s.progress(), 1);
    }

    #[test]
    fn should_hide_after_max_views() {
        let mut s = ChecklistState::default();
        for step in ChecklistStep::ALL {
            s.mark(step);
        }
        for _ in 0..CHECKLIST_MAX_VIEWS {
            assert!(s.should_display());
            s.record_view();
        }
        assert!(!s.should_display());
    }

    #[test]
    fn record_view_noop_when_incomplete() {
        let mut s = ChecklistState::default();
        s.mark(ChecklistStep::ConfigureServer);
        s.record_view();
        assert_eq!(s.full_views, 0);
    }

    #[test]
    fn dismiss_hides_permanently() {
        let mut s = ChecklistState::default();
        s.dismiss();
        assert!(!s.should_display());
    }

    #[test]
    fn all_steps_have_distinct_labels() {
        let mut labels: Vec<&str> = ChecklistStep::ALL.iter().map(|s| s.label()).collect();
        labels.sort();
        labels.dedup();
        assert_eq!(labels.len(), ChecklistStep::ALL.len());
    }
}
