//! Tip system con cooldown por sesion (E39).
//!
//! Catalogo estatico de tips contextuales, una por screen. El registry elige
//! deterministicamente un tip por sesion rotando por session_count, y respeta
//! un cooldown minimo para no spammear: maximo 1 tip cada `TIP_COOLDOWN_SESSIONS`
//! sesiones. Tips dismisseados explicitamente no vuelven a aparecer.

use serde::{Deserialize, Serialize};

/// Minimo de sesiones entre dos tips mostrados. Elegido para que el usuario
/// aprenda 1 cosa nueva cada 5 arranques sin sentir que el programa lo hostiga.
pub const TIP_COOLDOWN_SESSIONS: u32 = 5;

/// Contexto donde aplica el tip — el render consulta `state.screen` para
/// filtrar solo tips aplicables al screen actual.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TipScope {
    #[allow(
        dead_code,
        reason = "E39 — variante reservada para tips especificos del splash (catalog actual usa Any)"
    )]
    Splash,
    Dashboard,
    Chat,
    Any,
}

/// Tip individual. El `id` debe ser estable: se usa en la lista de dismissed.
#[derive(Debug, Clone, Copy)]
pub struct Tip {
    pub id: &'static str,
    pub scope: TipScope,
    pub text: &'static str,
}

/// Catalogo hardcoded. Si crece, extraer a JSON en XDG data dir.
pub const TIP_CATALOG: &[Tip] = &[
    Tip { id: "factory_tab", scope: TipScope::Any, text: "Usa Tab para cambiar entre factories." },
    Tip {
        id: "slash_plan",
        scope: TipScope::Chat,
        text: "Prueba /plan para que el asistente planifique antes de codear.",
    },
    Tip {
        id: "command_palette",
        scope: TipScope::Any,
        text: "Ctrl+K abre la paleta de comandos rapidamente.",
    },
    Tip {
        id: "transcript",
        scope: TipScope::Chat,
        text: "Ctrl+O abre el transcript completo con busqueda (Ctrl+F, n/N).",
    },
    Tip {
        id: "brief_mode",
        scope: TipScope::Chat,
        text: "/brief alterna modo compacto cuando la conversacion se vuelve larga.",
    },
    Tip {
        id: "dashboard_search",
        scope: TipScope::Dashboard,
        text: "Presiona / para buscar documentos, skills y workflows.",
    },
    Tip {
        id: "theme_switch",
        scope: TipScope::Any,
        text: "Ctrl+T cambia el tema (dark/light/high-contrast/solarized).",
    },
];

/// Estado persistente del tip system.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TipState {
    /// Session en la que se mostro el ultimo tip (para el cooldown).
    #[serde(default)]
    pub last_shown_session: u32,
    /// IDs que el usuario dismisseo explicitamente.
    #[serde(default)]
    pub dismissed: Vec<String>,
}

impl TipState {
    /// Elige un tip candidato para la sesion actual. Respeta cooldown y
    /// exclusiones. Retorna `None` si no hay tip disponible en este screen.
    ///
    /// La seleccion es deterministica en funcion de `session_count` para que
    /// el mismo usuario vea tips en un orden estable entre runs consecutivas
    /// (util para testing y para que un tip "se pegue" antes de rotar).
    pub fn pick<'a>(
        &self,
        session_count: u32,
        scope: TipScope,
        catalog: &'a [Tip],
    ) -> Option<&'a Tip> {
        if session_count.saturating_sub(self.last_shown_session) < TIP_COOLDOWN_SESSIONS
            && self.last_shown_session != 0
        {
            return None;
        }
        let eligible: Vec<&Tip> = catalog
            .iter()
            .filter(|t| t.scope == scope || t.scope == TipScope::Any)
            .filter(|t| !self.dismissed.iter().any(|d| d == t.id))
            .collect();
        if eligible.is_empty() {
            return None;
        }
        let idx = (session_count as usize) % eligible.len();
        Some(eligible[idx])
    }

    /// Marca que un tip fue mostrado en la sesion actual (resetea cooldown).
    pub fn mark_shown(&mut self, session_count: u32) {
        self.last_shown_session = session_count;
    }

    /// Dismiss permanente por id.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "E39 — dismiss expuesto pero aun sin handler de UI (iter futura)"
        )
    )]
    pub fn dismiss(&mut self, id: &str) {
        let id_owned = id.to_string();
        if !self.dismissed.iter().any(|d| d == &id_owned) {
            self.dismissed.push(id_owned);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tip(id: &'static str, scope: TipScope) -> Tip {
        Tip { id, scope, text: "x" }
    }

    #[test]
    fn pick_respects_cooldown() {
        let state = TipState { last_shown_session: 10, dismissed: vec![] };
        let cat = [tip("a", TipScope::Any)];
        assert!(state.pick(12, TipScope::Chat, &cat).is_none());
        assert!(state.pick(15, TipScope::Chat, &cat).is_some());
    }

    #[test]
    fn pick_filters_by_scope() {
        let state = TipState::default();
        let cat = [tip("a", TipScope::Chat), tip("b", TipScope::Dashboard)];
        assert_eq!(state.pick(1, TipScope::Dashboard, &cat).unwrap().id, "b");
    }

    #[test]
    fn pick_excludes_dismissed() {
        let state = TipState { last_shown_session: 0, dismissed: vec!["a".into()] };
        let cat = [tip("a", TipScope::Any), tip("b", TipScope::Any)];
        assert_eq!(state.pick(0, TipScope::Chat, &cat).unwrap().id, "b");
    }

    #[test]
    fn pick_returns_none_when_all_dismissed() {
        let state = TipState { last_shown_session: 0, dismissed: vec!["a".into()] };
        let cat = [tip("a", TipScope::Any)];
        assert!(state.pick(0, TipScope::Chat, &cat).is_none());
    }

    #[test]
    fn pick_any_scope_shows_everywhere() {
        let state = TipState::default();
        let cat = [tip("a", TipScope::Any)];
        assert!(state.pick(0, TipScope::Chat, &cat).is_some());
        assert!(state.pick(0, TipScope::Dashboard, &cat).is_some());
        assert!(state.pick(0, TipScope::Splash, &cat).is_some());
    }

    #[test]
    fn mark_shown_updates_cooldown() {
        let mut s = TipState::default();
        s.mark_shown(7);
        assert_eq!(s.last_shown_session, 7);
    }

    #[test]
    fn dismiss_is_idempotent() {
        let mut s = TipState::default();
        s.dismiss("tip_a");
        s.dismiss("tip_a");
        assert_eq!(s.dismissed.len(), 1);
    }

    #[test]
    fn catalog_ids_are_unique() {
        let mut ids: Vec<&str> = TIP_CATALOG.iter().map(|t| t.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), TIP_CATALOG.len());
    }
}
