// ── Command Palette ───────────────────────────────────────────────────────────

pub struct PaletteCmd {
    pub id: &'static str,
    pub description: &'static str,
    pub category: &'static str,
}

pub const PALETTE_COMMANDS: &[PaletteCmd] = &[
    // ── Sincronizacion ─────────────────────────────────────────────────────
    PaletteCmd {
        id: "sync",
        description: "Descarga documentos de todas las factories",
        category: "Sincronizacion",
    },
    // ── Estado / Diagnostico ───────────────────────────────────────────────
    PaletteCmd {
        id: "health",
        description: "Consulta /api/health y muestra estado del servidor MCP",
        category: "Estado",
    },
    PaletteCmd {
        id: "doctor",
        description: "Diagnostico completo: MCP, LSP, IDE bridge, features y disco",
        category: "Estado",
    },
    PaletteCmd {
        id: "mcp-status",
        description: "Muestra info del servidor MCP, transporte y tools descubiertos",
        category: "Estado",
    },
    PaletteCmd {
        id: "audit",
        description: "Lista el audit log de tools y fallos",
        category: "Estado",
    },
    // ── Contexto de ingenierIA ───────────────────────────────────────────────
    PaletteCmd {
        id: "context",
        description: "Cambia la factory activa (Net → Ang → Nest → All) y recarga docs",
        category: "Contexto",
    },
    // ── Navegacion ─────────────────────────────────────────────────────────
    PaletteCmd {
        id: "search",
        description: "Abre busqueda fuzzy sobre todos los documentos cargados",
        category: "Navegacion",
    },
    PaletteCmd { id: "dashboard", description: "Abre el dashboard", category: "Navegacion" },
    PaletteCmd {
        id: "home",
        description: "Vuelve a la pantalla inicial (splash)",
        category: "Navegacion",
    },
    PaletteCmd {
        id: "transcript",
        description: "Abre/cierra el transcript overlay del chat",
        category: "Navegacion",
    },
    // ── Exploradores ───────────────────────────────────────────────────────
    PaletteCmd {
        id: "skills",
        description: "Abre el explorador de skills de ingenierIA",
        category: "Exploradores",
    },
    PaletteCmd {
        id: "commands",
        description: "Abre el explorador de commands de ingenierIA",
        category: "Exploradores",
    },
    PaletteCmd {
        id: "adrs",
        description: "Abre el explorador de ADRs (Architecture Decision Records)",
        category: "Exploradores",
    },
    PaletteCmd {
        id: "policies",
        description: "Abre el explorador de policies de cumplimiento",
        category: "Exploradores",
    },
    PaletteCmd {
        id: "agents",
        description: "Abre el explorador de agents configurados en ingenierIA",
        category: "Exploradores",
    },
    PaletteCmd {
        id: "workflows",
        description: "Abre el explorador de workflows de ingenierIA",
        category: "Exploradores",
    },
    // ── Configuracion ──────────────────────────────────────────────────────
    PaletteCmd {
        id: "config",
        description: "Abre el wizard para configurar servidor, provider y modelo AI",
        category: "Configuracion",
    },
    PaletteCmd {
        id: "init",
        description: "Crea .ingenieria.json en el directorio actual para vincular proyecto",
        category: "Configuracion",
    },
    PaletteCmd {
        id: "disconnect",
        description: "Elimina token del provider AI actual y reabre el wizard",
        category: "Configuracion",
    },
    PaletteCmd {
        id: "model",
        description: "Cambia el modelo AI (ej: gpt-4o, claude-opus) sin reconectar",
        category: "Configuracion",
    },
    PaletteCmd {
        id: "permissions",
        description: "Cicla el modo de permisos (Standard → Permissive → Strict)",
        category: "Configuracion",
    },
    PaletteCmd {
        id: "theme",
        description: "Abre el selector de themes (tokyonight, solarized, gruvbox, ...)",
        category: "Configuracion",
    },
    PaletteCmd {
        id: "plugins",
        description: "Lista los plugins cargados",
        category: "Configuracion",
    },
    PaletteCmd {
        id: "plugins-reload",
        description: "Recarga los plugins desde disco",
        category: "Configuracion",
    },
    // ── Instalacion de stack ───────────────────────────────────────────────
    PaletteCmd {
        id: "autoskill",
        description: "Escanea el stack y abre un modal para instalar skills recomendadas",
        category: "Instalacion",
    },
    // ── Historial de input ─────────────────────────────────────────────────
    PaletteCmd {
        id: "history-search",
        description: "Busca en el historial de inputs anteriores del prompt",
        category: "Historial",
    },
];

/// A resolved command entry (static or dynamic) for display and execution.
pub struct ResolvedCmd {
    pub id: String,
    pub description: String,
    pub category: String,
}

pub struct CommandState {
    pub query: String,
    pub cursor: usize,
    /// Dynamic workflow commands loaded from server documents.
    pub dynamic_workflows: Vec<ResolvedCmd>,
    /// Recent chat inputs shown as "Historial" category in the palette.
    pub recent_history: Vec<String>,
}

impl CommandState {
    pub fn new() -> Self {
        Self {
            query: String::new(),
            cursor: 0,
            dynamic_workflows: Vec::new(),
            recent_history: Vec::new(),
        }
    }

    pub fn filtered(&self) -> Vec<ResolvedCmd> {
        let query = self.query.to_lowercase();
        let statics = PALETTE_COMMANDS.iter().map(|c| ResolvedCmd {
            id: c.id.to_string(),
            description: c.description.to_string(),
            category: c.category.to_string(),
        });
        let dynamics = self.dynamic_workflows.iter().map(|c| ResolvedCmd {
            id: c.id.clone(),
            description: c.description.clone(),
            category: c.category.clone(),
        });
        let history = self.recent_history.iter().enumerate().map(|(i, h)| {
            let preview: String = h.chars().take(60).collect();
            ResolvedCmd {
                id: format!("history-{i}"),
                description: preview,
                category: "Historial".to_string(),
            }
        });
        statics
            .chain(dynamics)
            .chain(history)
            .filter(|c| {
                query.is_empty()
                    || c.id.contains(query.as_str())
                    || c.description.to_lowercase().contains(query.as_str())
            })
            .collect()
    }

    /// Update the cached dynamic workflow list from loaded documents.
    pub fn load_workflows(&mut self, docs: &[crate::domain::document::DocumentSummary]) {
        self.dynamic_workflows = docs
            .iter()
            .filter(|d| d.doc_type == "workflow" || d.doc_type == "skill")
            .map(|d| ResolvedCmd {
                id: format!("workflow {}", d.name),
                description: d.description.clone(),
                category: "Workflows".to_string(),
            })
            .collect();
    }

    pub fn move_up(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    pub fn move_down(&mut self, len: usize) {
        if len > 0 {
            self.cursor = (self.cursor + 1).min(len - 1);
        }
    }

    pub fn reset(&mut self) {
        self.query.clear();
        self.cursor = 0;
        self.recent_history.clear();
    }
}

// ── Theme Picker ─────────────────────────────────────────────────────────────

/// Modal picker al estilo opencode: lista de temas con search inline y live
/// preview. `original` guarda el theme al abrir para permitir revertir con Esc.
pub struct ThemePickerState {
    pub query: String,
    pub cursor: usize,
    pub original: crate::ui::theme::ThemeVariant,
}

impl ThemePickerState {
    pub fn new(original: crate::ui::theme::ThemeVariant) -> Self {
        let cursor =
            crate::ui::theme::ThemeVariant::ALL.iter().position(|v| *v == original).unwrap_or(0);
        Self { query: String::new(), cursor, original }
    }

    pub fn filtered(&self) -> Vec<crate::ui::theme::ThemeVariant> {
        let q = self.query.trim().to_lowercase();
        crate::ui::theme::ThemeVariant::ALL
            .iter()
            .copied()
            .filter(|v| {
                q.is_empty() || v.slug().contains(&q) || v.label().to_lowercase().contains(&q)
            })
            .collect()
    }

    pub fn move_up(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    pub fn move_down(&mut self, len: usize) {
        if len > 0 {
            self.cursor = (self.cursor + 1).min(len - 1);
        }
    }

    pub fn selected(&self) -> Option<crate::ui::theme::ThemeVariant> {
        self.filtered().get(self.cursor).copied()
    }
}

// ── Model Picker ─────────────────────────────────────────────────────────────

pub struct ModelPickerState {
    pub models: Vec<crate::services::copilot::CopilotModel>,
    pub cursor: usize,
    pub loading: bool,
    pub error: Option<String>,
}

impl ModelPickerState {
    pub fn new() -> Self {
        Self { models: Vec::new(), cursor: 0, loading: false, error: None }
    }

    pub fn move_up(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    pub fn move_down(&mut self) {
        if !self.models.is_empty() {
            self.cursor = (self.cursor + 1).min(self.models.len() - 1);
        }
    }

    pub fn selected_model_id(&self) -> Option<&str> {
        self.models.get(self.cursor).map(|m| m.id.as_str())
    }
}

#[cfg(test)]
mod palette_tests {
    use super::*;

    fn palette_ids() -> Vec<&'static str> {
        PALETTE_COMMANDS.iter().map(|c| c.id).collect()
    }

    /// Comandos que migraron desde `:` al slash `/` no deben aparecer aqui.
    #[test]
    fn chat_only_commands_excluded_from_palette() {
        let ids = palette_ids();
        for banned in ["clear", "resume", "diff", "files", "sessions", "history"] {
            assert!(!ids.contains(&banned), "`{banned}` debe vivir en `/`, no en la paleta");
        }
    }

    /// Ids que migraron del slash al palette deben estar.
    #[test]
    fn ops_commands_present_in_palette() {
        let ids = palette_ids();
        for expected in [
            "sync",
            "health",
            "doctor",
            "mcp-status",
            "audit",
            "context",
            "search",
            "dashboard",
            "home",
            "transcript",
            "skills",
            "commands",
            "adrs",
            "policies",
            "agents",
            "workflows",
            "config",
            "init",
            "disconnect",
            "model",
            "permissions",
            "theme",
            "plugins",
            "plugins-reload",
            "autoskill",
            "history-search",
        ] {
            assert!(ids.contains(&expected), "`{expected}` debe vivir en la paleta");
        }
    }

    /// Nombres renombrados: `change-model` → `model`, `reset-config` fuera.
    #[test]
    fn renamed_entries_only_exist_with_new_name() {
        let ids = palette_ids();
        assert!(!ids.contains(&"change-model"));
        assert!(!ids.contains(&"reset-config"));
        assert!(ids.contains(&"model"));
    }
}
