use crate::domain::{
    doctor::DoctorReport,
    document::{DocumentDetail, DocumentSummary},
    event::IngenieriaEvent,
    health::HealthStatus,
    search::SearchResultItem,
};

/// Wrapper that redacts its value in Debug output to prevent token leaks.
pub struct Redacted(pub String);

impl std::fmt::Debug for Redacted {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("[REDACTED]")
    }
}

#[derive(Debug)]
pub enum Action {
    // ── Teclado ──────────────────────────────────────────────────────────────
    KeyChar(char),
    KeyBackspace,
    KeyEnter,
    KeyTab,
    KeyBackTab,
    KeyEsc,
    KeyCtrlC,
    KeyUp,
    KeyDown,
    KeyLeft,
    KeyRight,
    KeyHome,
    KeyEnd,
    KeyDelete,
    KeyShiftEnter,
    Paste(String),
    PreviewScrollUp,
    PreviewScrollDown,
    /// Ctrl+↑ en Chat: salta al user message anterior (nav_user_cursor -1)
    /// y desplaza scroll para dejarlo al tope del viewport.
    ChatNavPrev,
    /// Ctrl+↓ en Chat: salta al siguiente user message.
    ChatNavNext,
    /// Deshace el ultimo cambio del input (Ctrl+Z) — E40.
    InputUndo,
    /// Rehace el ultimo cambio deshecho del input (Ctrl+Y) — E40.
    InputRedo,

    // ── Mouse ─────────────────────────────────────────────────────────────
    MouseScrollUp(u16),   // column x
    MouseScrollDown(u16), // column x

    // ── Tick 250ms ──────────────────────────────────────────────────────────
    Tick,

    // ── Red: health ─────────────────────────────────────────────────────────
    HealthUpdated(HealthStatus),
    HealthFetchFailed(String),
    ServerEvent(IngenieriaEvent),
    SseDisconnected,

    // ── Red: documentos ─────────────────────────────────────────────────────
    DocumentsLoaded(Vec<DocumentSummary>),
    DocumentsLoadFailed(String),
    DocumentLoaded(DocumentDetail),
    DocumentLoadFailed(String),

    /// Document loaded for chat injection (from doc picker).
    ChatDocLoaded(DocumentDetail),
    ChatDocLoadFailed(String),

    // ── Red: búsqueda ───────────────────────────────────────────────────────
    SearchResultsReceived(Vec<SearchResultItem>),
    SearchFailed(String),

    // ── Wizard: validación URL ──────────────────────────────────────────────
    WizardUrlValid,
    WizardUrlInvalid(String),

    // ── Init: inicializar proyecto ────────────────────────────────────────
    InitDetected(crate::services::init::ProjectType, String),
    InitComplete(Vec<crate::services::init::InitFileResult>),
    InitFailed(String),

    // ── Copilot: OAuth device flow ───────────────────────────────────────
    CopilotDeviceCode {
        user_code: String,
        verification_uri: String,
        device_code: Redacted,
    },
    CopilotDeviceCodeFailed(String),
    CopilotAuthSuccess(Redacted), // oauth_token — redacted in Debug
    CopilotAuthFailed(String),
    CopilotModelsLoaded(Vec<crate::services::copilot::CopilotModel>),
    CopilotModelsFailed(String),

    // ── Chat ─────────────────────────────────────────────────────────────
    ChatContextLoaded(Vec<crate::state::ChatMessage>),
    ChatContextFailed(String),
    ChatStreamDelta(String),
    /// Delta de bloque de pensamiento extendido (Extended Thinking).
    ChatThinkingDelta(String),
    /// Toggle visibilidad del sidebar derecho (ctrl+b).
    ChatToggleSidebar,
    ChatTokenUsage {
        input_tokens: u32,
        output_tokens: u32,
        cache_creation_input_tokens: u32,
        cache_read_input_tokens: u32,
    },
    ChatStreamDone,
    /// API respondio con stop_reason=max_tokens: respuesta truncada por limite de tokens.
    ChatStreamTruncated,
    /// Fallo estructurado del stream (E13). Reemplaza `ChatStreamError(String)`.
    ChatStreamFailure(crate::domain::failure::StructuredFailure),
    /// Usuario aborto el turn actual (Esc durante Streaming/ExecutingTools).
    /// Abortea el task del provider y limpia el estado a Ready.
    ChatStreamAbort,
    // ── Focus ──────────────────────────────────────────────────────────────
    FocusGained,
    FocusLost,

    /// Periodic heartbeat during streaming (total elapsed seconds).
    StreamHeartbeat(u16),
    /// Streaming has been slow (no delta for 15s+). Carries total elapsed seconds.
    StreamWarning(u16),
    /// Streaming timed out (no delta for 60s). UI should offer retry.
    StreamTimeout,
    /// Retry automatico programado tras un error transitorio.
    ChatRetryScheduled {
        attempt: u32,
        max_attempts: u32,
        delay_secs: u16,
        reason: String,
    },
    /// Se sugiere usar un modelo de fallback tras varios fallos consecutivos.
    ChatFallbackSuggested {
        previous_model: String,
        suggested_model: String,
    },
    /// Stall detectado post-tool; se lanza un nudge silencioso.
    ChatPostToolStall {
        nudge_number: u32,
    },
    ChatToolCall {
        id: String,
        name: String,
        arguments: String,
    },
    ChatToolResult {
        tool_call_id: String,
        content: String,
    },

    // ── Sync tracking ───────────────────────────────────────────────────────
    SyncResult {
        updated_uris: Vec<String>,
        server_last_update: String,
    },
    SyncFailed(String),

    // ── Workflow ─────────────────────────────────────────────────────────
    WorkflowLoaded {
        workflow_name: String,
        content: String,
    },
    WorkflowFailed(String),

    // ── Compliance ────────────────────────────────────────────────────────
    ComplianceResult(String),
    ComplianceFailed(String),

    // ── Planning mode ────────────────────────────────────────────────────
    PlanApprove,
    PlanEdit,
    PlanReject,

    // ── Persistent permissions ───────────────────────────────────────────
    AlwaysAllowTool(String),
    AlwaysDenyTool(String),

    // ── Code blocks ──────────────────────────────────────────────────────
    CodeBlockApplied {
        msg: String,
        /// Absolute path of the written file (for LSP didChange).
        path: Option<String>,
        /// Content written (for LSP didChange).
        content: Option<String>,
    },

    // ── History ──────────────────────────────────────────────────────────
    HistoryList(Vec<crate::services::history::HistoryEntry>),
    HistoryLoaded(crate::services::history::SavedConversation),

    // ── Skill Discovery ──────────────────────────────────────────────────
    ProjectTypeDetected(crate::services::init::ProjectType),
    /// Extended autoskill scan completed (tech detection + skill suggestions).
    #[cfg(feature = "autoskill")]
    AutoSkillScanDone(crate::services::autoskill_map::AutoSkillScan),
    /// External skills installation completed.
    #[cfg(feature = "autoskill")]
    SkillInstallDone(crate::services::skill_installer::InstallSummary),

    // ── Tool Monitor (F.1) ──────────────────────────────────────────────
    ToolEventReceived(crate::domain::tool_event::ToolEvent),

    // ── Enforcement Dashboard (F.2) ─────────────────────────────────────
    HookEventReceived(crate::domain::hook_event::HookEvent),

    // ── Hooks configurables (E16) ───────────────────────────────────────
    HookExecuted(crate::services::hooks::HookOutcome),

    // ── MCP tools ────────────────────────────────────────────────────────────
    #[cfg(feature = "mcp")]
    McpToolsDiscovered(Vec<crate::services::mcp::McpToolInfo>),

    // ── MCP Elicitation (E18) ───────────────────────────────────────────────
    #[allow(
        dead_code,
        reason = "Action consumida por bridge request_elicitation (E18) cuando el client MCP la invoque"
    )]
    ElicitationRequested {
        request: crate::services::mcp::elicitation::ElicitationRequest,
        responder: crate::services::mcp::elicitation::ElicitationResponder,
    },

    // ── Doctor ───────────────────────────────────────────────────────────────
    DoctorReportReady(DoctorReport),

    // ── SubAgents (E22a) ────────────────────────────────────────────────────
    /// Resultado final de un subagent (Done/Failed/Cancelled).
    AgentResult {
        id: String,
        status: crate::services::agents::AgentStatus,
        result: Option<String>,
    },

    // ── Cron (E23) ──────────────────────────────────────────────────────────
    /// El cron worker detecto que el job `id` debe disparar ahora.
    CronJobFired {
        id: String,
    },

    // ── Process Monitor (E26) ───────────────────────────────────────────────
    /// Nueva linea de output capturada por el monitor `id`.
    MonitorOutput {
        id: String,
        line: String,
        is_stderr: bool,
    },
    /// El proceso del monitor `id` termino (por exit natural o kill).
    MonitorFinished {
        id: String,
        exit_code: Option<i32>,
        error: Option<String>,
        killed: bool,
    },

    // ── ConfigTool (E20) ────────────────────────────────────────────────────
    /// El AI solicito cambiar un campo de configuracion. El handler valida,
    /// aplica el cambio y registra un entry en audit log.
    ApplyConfigChange {
        field: String,
        value: String,
    },

    /// El AI sincronizo la lista de todos via TodoWriteTool. Reemplaza
    /// `chat.todos` con la lista recibida preservando ids cuando coincide el
    /// titulo (continuidad en progreso del usuario).
    ApplyTodoWrite {
        items: Vec<crate::services::tools::todowrite::TodoInput>,
    },

    /// P2.4: la resolución async de @mentions terminó. El handler empuja el
    /// mensaje de usuario con el contenido augmentado + refs, y arranca la
    /// completion del provider. Si `refs` está vacío la resolución falló o
    /// no había mentions — igual se envía el texto original.
    ChatUserMessageResolved {
        augmented_text: String,
        refs: Vec<crate::state::DocReference>,
    },

    // ── LSP Integration (E25) ───────────────────────────────────────────────
    /// Diagnosticos recibidos via `textDocument/publishDiagnostics`.
    LspDiagnosticsReceived {
        uri: String,
        diagnostics: Vec<crate::services::lsp::LspDiagnostic>,
    },
    /// LSP server arranco exitosamente (handshake completo).
    LspServerStarted {
        name: String,
    },
    /// LSP server fallo al arrancar o durante el handshake.
    LspServerFailed {
        name: String,
        error: String,
    },

    // ── IDE Bridge (E27) ─────────────────────────────────────────────────────
    /// El IDE envio contexto adicional via HTTP.
    #[cfg(feature = "ide")]
    BridgeContextUpdate {
        kind: String,
        path: Option<String>,
        content: Option<String>,
    },
    /// El IDE respondio a un permiso de tool pendiente.
    #[cfg(feature = "ide")]
    BridgeToolApproval {
        tool_call_id: String,
        approved: bool,
    },

    // ── File watcher (E42) ──────────────────────────────────────────────────
    /// `~/.config/ingenieria-tui/config.json` fue modificado.
    ConfigChanged,
    /// `~/.config/ingenieria-tui/keybindings.json` fue modificado.
    KeybindingsChanged,
    /// `$CWD/CLAUDE.md` fue modificado.
    ClaudeMdChanged,
    /// `$CWD/.env` fue modificado.
    EnvChanged,

    // ── Recovery (E42) ──────────────────────────────────────────────────────
    /// Un recovery recipe se disparo para un escenario.
    #[allow(
        dead_code,
        reason = "Action consumida por recovery_engine en disparos async; placeholder para integracion completa"
    )]
    RecoveryRecipeDispatched {
        scenario_label: String,
        step_label: String,
    },

    // ── Control ─────────────────────────────────────────────────────────────
    #[cfg_attr(not(test), allow(dead_code))]
    Quit,
}

impl Action {
    /// Extrae el nombre del variant como `&str` para hooks de plugins.
    /// Parsea la representacion Debug (e.g. `"KeyChar('a')"` → `"KeyChar"`).
    /// Solo se llama cuando hay plugins registrados, asi que la allocacion
    /// es aceptable.
    pub fn tag(&self) -> String {
        let debug = format!("{self:?}");
        let end = debug.find(['(', ' ', '{']).unwrap_or(debug.len());
        debug[..end].to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tag_simple_variant() {
        assert_eq!(Action::Tick.tag(), "Tick");
        assert_eq!(Action::KeyEnter.tag(), "KeyEnter");
        assert_eq!(Action::KeyBackspace.tag(), "KeyBackspace");
    }

    #[test]
    fn tag_tuple_variant() {
        assert_eq!(Action::KeyChar('x').tag(), "KeyChar");
        assert_eq!(Action::ChatStreamDelta("hi".into()).tag(), "ChatStreamDelta");
    }

    #[test]
    fn tag_struct_variant() {
        let action =
            Action::ChatToolCall { id: "1".into(), name: "bash".into(), arguments: "{}".into() };
        assert_eq!(action.tag(), "ChatToolCall");
    }
}
