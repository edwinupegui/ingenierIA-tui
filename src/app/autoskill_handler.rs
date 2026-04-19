//! Handlers del modal `Autoskill` (feature `autoskill`).
//!
//! Abre el picker con spinner, recibe el resultado del scan, toggle de
//! seleccion y disparo de instalacion batch. Reutiliza `spawn_autoskill_scan`
//! y `spawn_install_skills` existentes (ver `src/app/spawners.rs`).

use crate::services::autoskill_map::{self, AutoSkillScan};
use crate::state::autoskill_picker::{AutoskillItem, AutoskillPickerState};
use crate::state::AppMode;

use super::App;

impl App {
    /// Abre el modal con spinner y lanza el scan async.
    pub(crate) fn open_autoskill_picker(&mut self) {
        self.state.autoskill_picker = Some(AutoskillPickerState::loading());
        self.state.mode = AppMode::AutoskillPicker;
        self.spawn_autoskill_scan();
    }

    /// Cierra el modal y vuelve a `Normal`. No dispara instalacion.
    pub(crate) fn close_autoskill_picker(&mut self) {
        self.state.autoskill_picker = None;
        self.state.mode = AppMode::Normal;
    }

    /// Popula el picker con el scan. Si el modal NO esta abierto, retorna
    /// `false` para que el caller aplique el flujo legacy (markdown en chat).
    pub(crate) fn populate_autoskill_picker(&mut self, scan: &AutoSkillScan) -> bool {
        if self.state.autoskill_picker.is_none() {
            return false;
        }
        let dir = std::env::current_dir().unwrap_or_default();
        let external = autoskill_map::collect_external_skills(scan, &dir);
        let items: Vec<AutoskillItem> = external
            .into_iter()
            .map(|s| AutoskillItem {
                path: s.path,
                name: s.name,
                sources: s.sources,
                installed: s.installed,
                selected: false,
            })
            .collect();

        let summary = format_project_summary(scan);
        if let Some(picker) = self.state.autoskill_picker.as_mut() {
            picker.items = items;
            picker.cursor = 0;
            picker.loading = false;
            picker.project_summary = summary;
        }
        // Mantener pending_external_skills en sync para telemetria/compat.
        self.state.pending_external_skills = self
            .state
            .autoskill_picker
            .as_ref()
            .map(|p| p.items.iter().filter(|i| !i.installed).map(|i| i.path.clone()).collect())
            .unwrap_or_default();
        true
    }

    /// Toggle del item bajo cursor. Si no hay picker abierto, no-op.
    pub(crate) fn toggle_autoskill_current(&mut self) {
        if let Some(picker) = self.state.autoskill_picker.as_mut() {
            picker.toggle_current();
        }
    }

    /// Lanza la instalacion de las skills marcadas y cierra el modal.
    /// Si no hay ninguna marcada, notifica y deja el modal abierto.
    pub(crate) fn install_selected_autoskills(&mut self) {
        let Some(picker) = self.state.autoskill_picker.as_ref() else {
            return;
        };
        let paths = picker.selected_paths();
        if paths.is_empty() {
            self.notify("Marca al menos una skill con Space antes de instalar".to_string());
            return;
        }
        let count = paths.len();
        self.spawn_install_skills(paths);
        self.notify(format!("Instalando {count} skills..."));
        self.close_autoskill_picker();
    }
}

/// Resumen corto del stack detectado, mostrado en el header del modal.
fn format_project_summary(scan: &AutoSkillScan) -> String {
    let primary = scan.primary_factory.unwrap_or("--");
    if scan.techs.is_empty() {
        return format!("Factory: {primary} (sin techs detectadas)");
    }
    let techs: Vec<&str> = scan.techs.iter().take(4).map(|t| t.name).collect();
    let extra = scan.techs.len().saturating_sub(techs.len());
    if extra > 0 {
        format!("Factory: {primary} — {} + {extra} mas", techs.join(", "))
    } else {
        format!("Factory: {primary} — {}", techs.join(", "))
    }
}
