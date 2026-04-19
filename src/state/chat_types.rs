// ── Chat supporting types ────────────────────────────────────────────────────
//
// Core types (ChatRole, ToolCallStatus, ToolCall, ChatMode) live in
// ingenieria-domain and are re-exported here for backward compatibility.

pub use ingenieria_domain::chat::{ChatMode, ChatRole, ToolCall, ToolCallStatus};

/// Referencia a un documento MCP inyectado como contexto en un turno del
/// chat (P2.6). Se popula cuando el usuario usa @mentions y el resolver
/// MCP trae contenido. Usado por `/audit` y el UI para mostrar citations.
#[derive(Debug, Clone, PartialEq)]
pub struct DocReference {
    /// URI ingenieria (ej: `ingenieria://skill/net/add-feature`).
    pub uri: String,
    /// Tipo del doc: "skill" | "agent" | "workflow" | "adr" | "policy" | "command".
    pub kind: String,
    /// Nombre del doc tal como el usuario lo tipeó.
    pub name: String,
    /// Tamaño en bytes del contenido inyectado al prompt.
    pub bytes: usize,
}

/// Chat message with UI-specific fields not present in the domain crate.
///
/// The domain `ingenieria_domain::chat` module holds the pure types; this
/// struct adds `cached_lines` (ratatui) and `structured` (services) fields
/// that tie it to the main binary.
#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub role: ChatRole,
    pub content: String,
    pub tool_calls: Vec<ToolCall>,
    pub tool_call_id: Option<String>,
    /// Cached rendered markdown lines. None while streaming, Some after finalized.
    /// Wrapped in Arc to avoid deep cloning on every render frame.
    pub cached_lines: Option<std::sync::Arc<Vec<ratatui::text::Line<'static>>>>,
    /// Output estructurado detectado en el contenido (E19). Se popula al
    /// finalizar el turno del assistant via `detect_structured_output`.
    pub structured: Option<crate::services::structured_output::StructuredOutput>,
    /// Documentos MCP citados en este turno (P2.6). Vacío si no se usaron
    /// @mentions o la resolución falló silenciosamente.
    pub context_refs: Vec<DocReference>,
    /// Bloque de razonamiento extendido (Extended Thinking). Acumulado durante
    /// streaming via ThinkingDelta; renderizado antes del body en italic/muted.
    pub thinking: Option<String>,
}

impl ChatMessage {
    pub fn new(role: ChatRole, content: String) -> Self {
        Self {
            role,
            content,
            tool_calls: Vec::new(),
            tool_call_id: None,
            cached_lines: None,
            structured: None,
            context_refs: Vec::new(),
            thinking: None,
        }
    }

    pub fn tool_result(tool_call_id: String, content: String) -> Self {
        Self {
            role: ChatRole::Tool,
            content,
            tool_calls: Vec::new(),
            tool_call_id: Some(tool_call_id),
            cached_lines: None,
            structured: None,
            context_refs: Vec::new(),
            thinking: None,
        }
    }

    /// Invalidate cached markdown (call when content changes).
    pub fn invalidate_cache(&mut self) {
        self.cached_lines = None;
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ChatStatus {
    LoadingContext,
    Ready,
    Streaming,
    ExecutingTools,
    Error(String),
}

// ChatMode is re-exported from ingenieria_domain::chat above.

/// Display mode para la lista de mensajes del chat (E33).
///
/// `Normal` es el render por defecto (markdown + tool expansion toggles normales).
/// `Brief` filtra mensajes verbose: trunca el texto del asistente a `BRIEF_MAX_LINES`
/// lineas y reemplaza tool results por un contador compacto.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum ChatDisplayMode {
    #[default]
    Normal,
    /// Modo compacto: solo resultados clave, texto verbose truncado.
    Brief,
}

impl ChatDisplayMode {
    /// Expuesto para tests; el toggle se habilitara de nuevo cuando se
    /// reintroduzca un entry point para cambiar entre Normal/Brief.
    #[cfg(test)]
    #[allow(dead_code)]
    pub fn label(&self) -> &'static str {
        match self {
            ChatDisplayMode::Normal => "normal",
            ChatDisplayMode::Brief => "brief",
        }
    }
}

/// Lineas maximas renderizadas por mensaje del asistente en modo `Brief`.
/// Lo que sobre se colapsa en una linea resumen `… [+N líneas]`.
pub const BRIEF_MAX_LINES: usize = 12;

// ChatMode::as_str and from_str_lossy live in ingenieria_domain::chat.

/// Modo de operación del agente — controla si las herramientas se ejecutan
/// automáticamente, requieren aprobación, o solo generan planes.
///
/// Ciclable con Shift+Tab: Ask → Auto → Plan → Ask.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum AgentMode {
    /// Pide aprobación para herramientas no-safe (comportamiento por defecto).
    #[default]
    Ask,
    /// Ejecuta TODAS las herramientas sin preguntar.
    Auto,
    /// Solo lectura — genera planes sin ejecutar cambios. Activa ChatMode::Planning.
    Plan,
}

impl AgentMode {
    pub fn label(&self) -> &'static str {
        match self {
            AgentMode::Ask => "ASK",
            AgentMode::Auto => "AUTO",
            AgentMode::Plan => "PLAN",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            AgentMode::Ask => "?",
            AgentMode::Auto => "⚡",
            AgentMode::Plan => "◈",
        }
    }

    pub fn next(&self) -> AgentMode {
        match self {
            AgentMode::Ask => AgentMode::Auto,
            AgentMode::Auto => AgentMode::Plan,
            AgentMode::Plan => AgentMode::Ask,
        }
    }
}


/// Maximum tool call rounds before stopping (prevents infinite loops).
pub const MAX_TOOL_ROUNDS: u8 = 10;

// ── Cost tracking ────────────────────────────────────────────────────────────

// CostState fue movido a `state/cost.rs` para mantener este archivo bajo el
// limite de 400 LOC y para soportar pricing multi-modelo + prompt caching.
pub use super::cost::CostState;

/// A tool call awaiting user approval.
#[derive(Debug, Clone)]
pub struct PendingToolApproval {
    pub tool_call_id: String,
    pub tool_name: String,
    pub arguments: String,
    pub permission: String,
    /// Pre-computed display label "[permission] " — evita format!() en el render loop.
    pub permission_label: String,
    /// Main reason from the enforcer (shown as subtitle in modal).
    pub reason: Option<String>,
    /// Detailed reasons from the bash validator pipeline.
    pub validator_reasons: Vec<String>,
    /// Selection flag para aprobar/denegar subsets cuando hay múltiples
    /// tool calls pendientes en la misma ronda. Toggle con Space en el modal.
    pub selected: bool,
}

/// A skill selected from the doc picker, pending user input.
#[derive(Debug, Clone)]
pub struct SelectedSkill {
    pub name: String,
}

/// Estado UI de una elicitation pendiente (E18).
///
/// No implementa `Clone` porque contiene un `ElicitationResponder` de un
/// solo uso. El modal se renderiza mientras este campo sea `Some`; el handler
/// de teclado hace `take()` cuando el usuario responde.
#[derive(Debug)]
pub struct PendingElicitation {
    pub request: crate::services::mcp::elicitation::ElicitationRequest,
    pub responder: crate::services::mcp::elicitation::ElicitationResponder,
    /// Buffer de texto (solo para `ElicitationField::Text`).
    pub text_buffer: String,
    /// Cursor para Select/MultiSelect. Ignorado en Text/Confirm.
    pub cursor: usize,
    /// Indices seleccionados para MultiSelect.
    pub multi_selected: std::collections::BTreeSet<usize>,
}

impl PendingElicitation {
    pub fn new(
        request: crate::services::mcp::elicitation::ElicitationRequest,
        responder: crate::services::mcp::elicitation::ElicitationResponder,
    ) -> Self {
        Self {
            request,
            responder,
            text_buffer: String::new(),
            cursor: 0,
            multi_selected: std::collections::BTreeSet::new(),
        }
    }
}

// ── Slash command autocomplete ──────────────────────────────────────────────

/// All available slash commands with descriptions.
///
/// Solo "cosas del chat": sesion, contexto AI, agents/teams/monitores, todos,
/// memoria, workflows ejecutables. La configuracion (theme, model, doctor,
/// autoskill, ...) vive en la paleta `:` (ver `PALETTE_COMMANDS`).
pub const SLASH_COMMANDS: &[(&str, &str)] = &[
    // ── Sesion ─────────────────────────────────────────────────────────────
    ("/clear", "Limpia el historial del chat y reinicia la conversacion desde cero"),
    ("/exit", "Cierra el chat y vuelve a la pantalla inicial de ingenierIA"),
    ("/resume", "Retoma la ultima sesion de chat exactamente donde la dejaste"),
    ("/history", "Muestra las sesiones de chat guardadas en el servidor para retomar"),
    ("/fork", "Ramifica la sesion actual con un label para explorar otro camino"),
    ("/export", "Exporta la sesion actual como archivo JSONL compartible"),
    ("/compact", "Compacta mensajes anteriores para liberar contexto sin perder hilo"),
    ("/undo", "Deshace el ultimo turn (pop user + assistant, restaura draft)"),
    ("/redo", "Rehace el ultimo /undo (solo si no enviaste un nuevo mensaje)"),
    ("/continue", "Reintenta el ultimo turno del AI sin enviar un nuevo prompt"),
    // ── Contexto AI ────────────────────────────────────────────────────────
    ("/diff", "Inyecta el git diff actual como contexto para que la AI lo analice"),
    ("/files", "Inyecta los archivos modificados recientemente como contexto para la AI"),
    ("/memory", "Muestra cuanto contexto se ha consumido y cuanto queda disponible"),
    ("/costs", "Muestra resumen de tokens consumidos, costos estimados y modelo activo"),
    ("/metrics", "Muestra metricas de performance: TTFT, OTPS y duracion por turno"),
    // ── Modo chat ──────────────────────────────────────────────────────────
    ("/plan", "Activa o desactiva el modo planning (la AI propone antes de ejecutar)"),
    // ── Output AI (code blocks) ────────────────────────────────────────────
    ("/apply", "Aplica un code block generado por la AI directamente al archivo destino"),
    ("/blocks", "Lista todos los bloques de codigo detectados en la conversacion actual"),
    // ── Agents / teams / monitores ─────────────────────────────────────────
    ("/spawn", "Lanza un subagent (orchestrator|discovery|architecture|...) con un prompt"),
    ("/agent-list", "Lista subagentes activos y recientes con su estado"),
    ("/agent-cancel", "Cancela un subagent activo por su id (cooperativo)"),
    ("/team-start", "Lanza un team de subagentes: /team-start <template> <goal>"),
    ("/team-list", "Muestra la tabla de teams activos y recientes"),
    ("/team-cancel", "Cancela todos los miembros de un team: /team-cancel <id>"),
    ("/team-mail", "Muestra el mailbox de un team: /team-mail <id>"),
    ("/monitor", "Lanza un proceso en background: /monitor <command>"),
    ("/monitor-list", "Lista procesos monitoreados activos y recientes con exit code"),
    ("/monitor-kill", "Mata un monitor activo: /monitor-kill <id>"),
    ("/monitor-show", "Muestra las ultimas lineas de un monitor: /monitor-show <id>"),
    // ── Todos ──────────────────────────────────────────────────────────────
    ("/todos", "Muestra la lista de todos de la sesion"),
    ("/todo-add", "Agrega un todo: /todo-add <titulo>"),
    ("/todo-start", "Marca un todo como en progreso: /todo-start <id>"),
    ("/todo-done", "Marca un todo como completado: /todo-done <id>"),
    ("/todo-remove", "Elimina un todo: /todo-remove <id>"),
    ("/todo-clear", "Vacia la lista de todos"),
    // ── Memoria persistente ────────────────────────────────────────────────
    ("/remember", "Guarda memoria: /remember <type> <file>: <body>"),
    ("/forget", "Elimina memoria: /forget <file>"),
    // ── Skills / workflows ejecutables ─────────────────────────────────────
    ("/workflow", "Carga un workflow ingenierIA por nombre para ejecucion guiada"),
    // ── Cron (requiere args inline) ────────────────────────────────────────
    ("/cron-add", "Agrega un cron job: /cron-add <notify|spawn> \"<expr>\" <args>"),
    ("/cron-list", "Lista los cron jobs configurados con su proxima ejecucion"),
    ("/cron-remove", "Elimina un cron job por su id (ej: c1)"),
    // ── Meta ───────────────────────────────────────────────────────────────
    ("/help", "Muestra la lista completa de comandos disponibles con descripcion"),
];

/// State for the slash command autocomplete popup.
#[derive(Debug, Clone, Default)]
pub struct SlashAutocomplete {
    pub visible: bool,
    pub cursor: usize,
    pub filtered: Vec<(usize, &'static str, &'static str)>, // (original_idx, cmd, desc)
    /// Query actual (sin `/`). Se usa para Tab→common_prefix separado del
    /// ranking fuzzy usado en `filtered`.
    pub query: String,
}

impl SlashAutocomplete {
    /// Update filtered list based on current input. Input should start with "/".
    ///
    /// Empty query → lista completa en orden original. Con query usa
    /// `nucleo_matcher` (mismo patron que `mention_picker`/`history_search`),
    /// scoreando contra `name + " " + desc` para que palabras del helper
    /// tambien matcheen (ej: "undo" → `/undo`, "deshacer" → tambien `/undo`).
    pub fn update(&mut self, input: &str) {
        let query = input.trim_start_matches('/');
        self.query = query.to_string();
        if query.is_empty() {
            self.filtered = SLASH_COMMANDS
                .iter()
                .enumerate()
                .map(|(i, (cmd, desc))| (i, *cmd, *desc))
                .collect();
        } else {
            let mut matcher = nucleo_matcher::Matcher::new(nucleo_matcher::Config::DEFAULT);
            let pattern = nucleo_matcher::pattern::Pattern::parse(
                query,
                nucleo_matcher::pattern::CaseMatching::Ignore,
                nucleo_matcher::pattern::Normalization::Smart,
            );
            let mut buf = Vec::new();
            let mut scored: Vec<(u32, usize, &'static str, &'static str)> = SLASH_COMMANDS
                .iter()
                .enumerate()
                .filter_map(|(i, (cmd, desc))| {
                    let haystack = format!("{} {desc}", cmd.trim_start_matches('/'));
                    let needle = nucleo_matcher::Utf32Str::new(&haystack, &mut buf);
                    pattern.score(needle, &mut matcher).map(|s| (s, i, *cmd, *desc))
                })
                .collect();
            // Score desc; tie-break por orden original (idx asc) para estabilidad.
            scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
            self.filtered = scored.into_iter().map(|(_, i, c, d)| (i, c, d)).collect();
        }

        self.visible = !self.filtered.is_empty();
        // Clamp cursor
        if self.cursor >= self.filtered.len() {
            self.cursor = self.filtered.len().saturating_sub(1);
        }
    }

    pub fn move_up(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
    }

    pub fn move_down(&mut self) {
        if self.cursor + 1 < self.filtered.len() {
            self.cursor += 1;
        }
    }

    /// Returns the selected command string (e.g. "/clear").
    pub fn selected_command(&self) -> Option<&'static str> {
        self.filtered.get(self.cursor).map(|(_, cmd, _)| *cmd)
    }

    /// Longest common prefix among candidates cuyo `cmd` empieza por `query`
    /// (case-insensitive). Se usa para Tab-complete: la semantica de prefijo
    /// es la esperada por el usuario aunque `filtered` use ranking fuzzy.
    /// Returns `None` cuando no hay candidatos prefix-match o el prefijo es
    /// vacio.
    pub fn common_prefix(&self) -> Option<String> {
        if self.query.is_empty() {
            return None;
        }
        let q = self.query.to_lowercase();
        let mut iter = SLASH_COMMANDS
            .iter()
            .filter(|(cmd, _)| cmd.trim_start_matches('/').to_lowercase().starts_with(&q))
            .map(|(cmd, _)| *cmd);
        let first = iter.next()?;
        let mut prefix_len = first.len();
        for cmd in iter {
            prefix_len = common_prefix_len(&first[..prefix_len], cmd);
            if prefix_len == 0 {
                return None;
            }
        }
        if prefix_len == 0 {
            None
        } else {
            Some(first[..prefix_len].to_string())
        }
    }

    pub fn close(&mut self) {
        self.visible = false;
        self.cursor = 0;
        self.filtered.clear();
        self.query.clear();
    }
}

/// Byte-length of the common prefix between two ASCII-safe slash commands.
/// Case-sensitive because slash commands are defined in lowercase.
fn common_prefix_len(a: &str, b: &str) -> usize {
    a.bytes().zip(b.bytes()).take_while(|(x, y)| x == y).count()
}

// ── DocPicker ────────────────────────────────────────────────────────────────

use crate::domain::document::DocumentSummary;

/// State for the MCP document picker overlay (skills, commands, adrs, etc.).
#[derive(Debug, Clone, Default)]
pub struct DocPickerState {
    pub visible: bool,
    pub doc_type: String,
    pub label: String,
    pub items: Vec<DocumentSummary>,
    pub filtered: Vec<usize>,
    pub cursor: usize,
    pub query: String,
}

impl DocPickerState {
    /// Open the picker for a specific doc_type, pre-filtered by factory.
    pub fn open(
        doc_type: &str,
        label: &str,
        all_docs: &[DocumentSummary],
        factory_key: Option<&str>,
    ) -> Self {
        let items: Vec<DocumentSummary> = all_docs
            .iter()
            .filter(|d| d.doc_type == doc_type)
            .filter(|d| factory_key.is_none_or(|f| d.factory == f))
            .cloned()
            .collect();
        let filtered: Vec<usize> = (0..items.len()).collect();
        Self {
            visible: true,
            doc_type: doc_type.to_string(),
            label: label.to_string(),
            items,
            filtered,
            cursor: 0,
            query: String::new(),
        }
    }

    pub fn update_filter(&mut self) {
        let query = self.query.to_lowercase();
        self.filtered = self
            .items
            .iter()
            .enumerate()
            .filter(|(_, d)| {
                query.is_empty()
                    || d.name.to_lowercase().contains(&query)
                    || d.description.to_lowercase().contains(&query)
            })
            .map(|(i, _)| i)
            .collect();
        if self.cursor >= self.filtered.len() {
            self.cursor = self.filtered.len().saturating_sub(1);
        }
    }

    pub fn move_up(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
    }

    pub fn move_down(&mut self) {
        if self.cursor + 1 < self.filtered.len() {
            self.cursor += 1;
        }
    }

    /// Returns the selected document summary.
    pub fn selected(&self) -> Option<&DocumentSummary> {
        self.filtered.get(self.cursor).and_then(|&i| self.items.get(i))
    }

    pub fn close(&mut self) {
        *self = Self::default();
    }
}

#[cfg(test)]
mod slash_autocomplete_tests {
    use super::SlashAutocomplete;

    #[test]
    fn common_prefix_returns_none_when_empty() {
        let ac = SlashAutocomplete::default();
        assert!(ac.common_prefix().is_none());
    }

    #[test]
    fn common_prefix_single_match_returns_full_command() {
        let mut ac = SlashAutocomplete::default();
        ac.update("/exi");
        assert_eq!(ac.common_prefix().as_deref(), Some("/exit"));
    }

    #[test]
    fn common_prefix_multiple_matches_returns_shared_prefix() {
        let mut ac = SlashAutocomplete::default();
        // "/c" matches /clear, /compact, /costs, /continue
        ac.update("/c");
        let prefix = ac.common_prefix().expect("hay candidatos");
        assert!(prefix.starts_with("/c"));
        assert!(prefix.len() >= 2);
    }

    #[test]
    fn common_prefix_narrows_when_input_narrows() {
        let mut ac = SlashAutocomplete::default();
        ac.update("/todo-");
        // /todo-add, /todo-start, /todo-done, /todo-remove, /todo-clear
        let prefix = ac.common_prefix().expect("hay candidatos");
        assert_eq!(prefix, "/todo-");
    }

    #[test]
    fn fuzzy_matches_non_prefix_substring() {
        // `compt` no es prefijo de ningun comando, pero con fuzzy deberia
        // matchear /compact (subsecuencia c-o-m-p-a-c-t).
        let mut ac = SlashAutocomplete::default();
        ac.update("/compt");
        let cmds: Vec<&str> = ac.filtered.iter().map(|(_, c, _)| *c).collect();
        assert!(
            cmds.contains(&"/compact"),
            "/compact debe matchear con fuzzy `compt`, got: {cmds:?}"
        );
    }

    #[test]
    fn fuzzy_ranks_prefix_match_before_distant_match() {
        // `cl` deberia rankear `/clear` arriba de comandos con c + l distantes.
        let mut ac = SlashAutocomplete::default();
        ac.update("/cl");
        let first = ac.filtered.first().map(|(_, c, _)| *c);
        assert_eq!(first, Some("/clear"), "/clear debe ser el top match");
    }

    #[test]
    fn empty_query_lists_all_in_original_order() {
        let mut ac = SlashAutocomplete::default();
        ac.update("/");
        assert_eq!(ac.filtered.len(), super::SLASH_COMMANDS.len());
        assert_eq!(ac.filtered[0].1, super::SLASH_COMMANDS[0].0);
    }

    /// Los slashes de configuracion migraron a la paleta `:`. Este test defiende
    /// contra reintroducciones accidentales en el array de `/`.
    #[test]
    fn config_commands_removed_from_slash() {
        let all: Vec<&str> = super::SLASH_COMMANDS.iter().map(|(c, _)| *c).collect();
        for banned in [
            "/theme",
            "/model",
            "/permissions",
            "/doctor",
            "/audit",
            "/mcp-status",
            "/autoskill",
            "/compliance",
            "/features",
            "/hooks",
            "/lsp-status",
            "/lsp-diag",
            "/bridge-status",
            "/plugins",
            "/dashboard",
            "/home",
            "/init",
            "/transcript",
            "/history-search",
            "/skills",
            "/commands",
            "/adrs",
            "/policies",
            "/agents",
            "/workflows",
            "/go",
            "/load",
            "/fork-from",
            "/memories",
            "/brief",
            "/sessions",
            "/retry",
            "/todo-check",
            "/install-skills",
            "/detect",
        ] {
            assert!(
                !all.contains(&banned),
                "`{banned}` debe vivir en la paleta `:`, no en los slashes"
            );
        }
    }

    /// Comandos centrales del chat deben seguir disponibles en `/`.
    #[test]
    fn chat_core_commands_present_in_slash() {
        let all: Vec<&str> = super::SLASH_COMMANDS.iter().map(|(c, _)| *c).collect();
        for expected in [
            "/clear",
            "/exit",
            "/resume",
            "/history",
            "/fork",
            "/export",
            "/compact",
            "/undo",
            "/redo",
            "/continue",
            "/diff",
            "/files",
            "/memory",
            "/costs",
            "/metrics",
            "/plan",
            "/apply",
            "/blocks",
            "/spawn",
            "/monitor",
            "/todos",
            "/remember",
            "/forget",
            "/workflow",
            "/help",
        ] {
            assert!(all.contains(&expected), "`{expected}` debe vivir en `/`");
        }
    }
}
