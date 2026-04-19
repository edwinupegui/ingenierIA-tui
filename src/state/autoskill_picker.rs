//! Estado del modal `Autoskill` (feature `autoskill`).
//!
//! Lista las skills recomendadas segun el stack detectado en el proyecto y
//! permite instalar las no-instaladas en batch. Se abre desde la paleta `:` +
//! `autoskill` y se alimenta con el resultado de `spawn_autoskill_scan`.

/// Una skill recomendada por el escaneo de stack.
#[derive(Debug, Clone)]
pub struct AutoskillItem {
    /// Path completo en el catalogo (`owner/repo/skill-name`).
    pub path: String,
    /// Nombre legible (ultimo segmento del path).
    pub name: String,
    /// Techs / combos que sugirieron esta skill.
    pub sources: Vec<String>,
    /// Ya esta instalada localmente (no se puede toggle).
    pub installed: bool,
    /// Marcada por el usuario para instalar (solo aplicable si `!installed`).
    pub selected: bool,
}

/// Estado del picker — una instancia mientras dura el modal abierto.
#[derive(Debug, Clone, Default)]
pub struct AutoskillPickerState {
    pub items: Vec<AutoskillItem>,
    pub cursor: usize,
    pub loading: bool,
    pub error: Option<String>,
    /// Resumen del stack detectado (ej: "Nest (TypeScript + Jest)").
    pub project_summary: String,
}

impl AutoskillPickerState {
    pub fn loading() -> Self {
        Self { loading: true, ..Self::default() }
    }

    pub fn move_up(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    pub fn move_down(&mut self) {
        if !self.items.is_empty() {
            self.cursor = (self.cursor + 1).min(self.items.len() - 1);
        }
    }

    /// Alterna la seleccion del item en `cursor`. No-op si la skill ya esta
    /// instalada.
    pub fn toggle_current(&mut self) {
        if let Some(item) = self.items.get_mut(self.cursor) {
            if !item.installed {
                item.selected = !item.selected;
            }
        }
    }

    /// Paths de skills seleccionadas para instalar (excluye las ya instaladas).
    pub fn selected_paths(&self) -> Vec<String> {
        self.items.iter().filter(|i| i.selected && !i.installed).map(|i| i.path.clone()).collect()
    }

    pub fn installed_count(&self) -> usize {
        self.items.iter().filter(|i| i.installed).count()
    }

    pub fn pending_count(&self) -> usize {
        self.items.iter().filter(|i| !i.installed).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_item(name: &str, installed: bool) -> AutoskillItem {
        AutoskillItem {
            path: format!("owner/repo/{name}"),
            name: name.to_string(),
            sources: vec!["nest".to_string()],
            installed,
            selected: false,
        }
    }

    #[test]
    fn toggle_only_affects_uninstalled() {
        let mut state = AutoskillPickerState {
            items: vec![sample_item("a", true), sample_item("b", false)],
            ..Default::default()
        };
        state.cursor = 0;
        state.toggle_current();
        assert!(!state.items[0].selected, "installed skill no debe togglearse");

        state.cursor = 1;
        state.toggle_current();
        assert!(state.items[1].selected, "uninstalled skill si se toggle");
    }

    #[test]
    fn selected_paths_excludes_installed() {
        let mut a = sample_item("a", true);
        a.selected = true; // defensive: aunque este installed+selected
        let mut b = sample_item("b", false);
        b.selected = true;
        let state = AutoskillPickerState { items: vec![a, b], ..Default::default() };
        assert_eq!(state.selected_paths(), vec!["owner/repo/b".to_string()]);
    }

    #[test]
    fn move_down_clamps_to_last_item() {
        let mut state = AutoskillPickerState {
            items: vec![sample_item("a", false), sample_item("b", false)],
            ..Default::default()
        };
        state.move_down();
        state.move_down();
        state.move_down();
        assert_eq!(state.cursor, 1);
    }
}
