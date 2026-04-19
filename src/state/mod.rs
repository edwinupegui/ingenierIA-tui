#[cfg(feature = "autoskill")]
pub mod autoskill_picker;
pub mod cache_state;
pub mod chat_state;
pub mod chat_types;
mod command_state;
pub mod cost;
mod dashboard_state;
mod event_state;
pub mod history_search;
mod init_state;
pub mod input_undo;
pub mod mention_picker;
pub mod message_queue;
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "E08 spec — consumed when render loop adopts Selector-based caching"
    )
)]
pub mod selectors;
pub mod transcript;
mod wizard_state;

// Re-export all public types so existing `use crate::state::*` imports keep working.
pub use cache_state::CacheLayer;
pub use chat_state::ChatState;
pub use chat_types::{
    AgentMode, ChatDisplayMode, ChatMessage, ChatMode, ChatRole, ChatStatus, DocPickerState,
    DocReference, PendingToolApproval, SelectedSkill, SlashAutocomplete, ToolCall, ToolCallStatus,
    BRIEF_MAX_LINES, MAX_TOOL_ROUNDS,
};
pub use command_state::{CommandState, ModelPickerState, ThemePickerState};
pub use cost::CostState;
pub use dashboard_state::{DashboardState, SearchState};
pub use event_state::{system_time_str, ActiveSession, TimedEvent};
pub use init_state::{InitState, InitStep};
#[cfg_attr(
    not(test),
    allow(unused_imports, reason = "Re-export consumido por tests del handler")
)]
pub use transcript::TranscriptView;
pub use wizard_state::{
    UrlValidation, WizardModelPhase, WizardState, WizardStep, WIZARD_PROVIDERS, WIZARD_ROLES,
};

// Re-export toast types directly so callers use `state::ToastLevel::Success` etc.
// The ToastState struct is accessed via `state.toasts`.

use crate::domain::{doctor::DoctorReport, health::HealthStatus};
use crate::services::onboarding::{OnboardingState, PlatformHints, Tip};
use crate::ui::theme::ThemeVariant;

// ── Toasts ──────────────────────────────────────────────────────────────────

pub use ingenieria_domain::toast::ToastLevel;

#[derive(Debug, Clone)]
pub struct Toast {
    pub message: String,
    pub level: ToastLevel,
    pub created_at: u64,
}

/// Max toasts visibles simultaneamente.
const MAX_TOASTS: usize = 4;
/// Ticks antes de auto-dismiss (3s a 4Hz = 12 ticks).
const TOAST_LIFETIME: u64 = 12;

#[derive(Debug, Default)]
pub struct ToastState {
    pub toasts: Vec<Toast>,
}

impl ToastState {
    pub fn push(&mut self, message: String, level: ToastLevel, tick: u64) {
        self.toasts.push(Toast { message, level, created_at: tick });
        // Mantener solo los mas recientes
        if self.toasts.len() > MAX_TOASTS * 2 {
            self.toasts.drain(..self.toasts.len() - MAX_TOASTS);
        }
    }

    /// Remueve toasts expirados y retorna los visibles.
    pub fn tick(&mut self, current_tick: u64) {
        self.toasts.retain(|t| current_tick.saturating_sub(t.created_at) < TOAST_LIFETIME);
    }

    pub fn visible(&self) -> impl Iterator<Item = &Toast> {
        self.toasts.iter().rev().take(MAX_TOASTS)
    }

    pub fn is_empty(&self) -> bool {
        self.toasts.is_empty()
    }
}

// ── Factory UI ───────────────────────────────────────────────────────────────

pub use ingenieria_domain::factory::UiFactory;

// ── Server status ────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum ServerStatus {
    Unknown,
    Online(HealthStatus),
    Offline(String),
}

impl ServerStatus {
    pub fn docs_total(&self) -> Option<u32> {
        if let ServerStatus::Online(h) = self {
            Some(h.docs.total)
        } else {
            None
        }
    }
}

// ── App mode ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum AppMode {
    Normal,
    Search,
    Command,
    ModelPicker,
    ThemePicker,
    #[cfg(feature = "autoskill")]
    AutoskillPicker,
}

// ── App screen ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum AppScreen {
    Wizard,
    Splash,
    Dashboard,
    Init,
    Chat,
}

// ── UI Panel Flags ───────────────────────────────────────────────────────────

#[derive(Default)]
pub struct UiPanelFlags {
    pub show_tool_monitor: bool,
    pub show_enforcement: bool,
    pub show_agents: bool,
    pub show_cost_panel: bool,
    pub show_notifications: bool,
    pub show_sessions: bool,
    pub show_doctor: bool,
}

// ── AppState (root) ──────────────────────────────────────────────────────────

pub struct AppState {
    pub screen: AppScreen,
    pub mode: AppMode,
    pub factory: UiFactory,
    pub input: String,
    /// Slash autocomplete state for the splash screen input.
    pub splash_autocomplete: SlashAutocomplete,
    /// Pending doc picker to open once documents finish loading.
    pub pending_picker: Option<(String, String)>,
    pub server_status: ServerStatus,
    pub tick_count: u64,
    /// Dirty flag for deferred config persistence.
    pub config_dirty: bool,

    // Sub-states by domain
    pub dashboard: DashboardState,
    pub search: SearchState,
    pub command: CommandState,
    pub chat: ChatState,
    pub wizard: WizardState,
    pub init: InitState,
    pub model_picker: ModelPickerState,
    pub theme_picker: Option<ThemePickerState>,
    #[cfg(feature = "autoskill")]
    pub autoskill_picker: Option<autoskill_picker::AutoskillPickerState>,

    // UI
    pub panels: UiPanelFlags,
    pub toasts: ToastState,
    pub keybindings: crate::config::Keybindings,

    // Events
    pub events: Vec<TimedEvent>,
    pub sessions: Vec<ActiveSession>,
    pub tool_events: Vec<crate::domain::tool_event::ToolEvent>,
    pub hook_events: Vec<crate::domain::hook_event::HookEvent>,
    pub tool_monitor_filter: ToolMonitorFilter,

    // Config
    pub developer: String,
    pub model: String,
    /// Directorio de trabajo al iniciar (cacheado para el sidebar, no cambia).
    pub working_dir: String,
    /// Rama git activa al iniciar (cacheado para el sidebar).
    pub git_branch: Option<String>,
    /// Provider AI activo ("anthropic", "github-copilot", "mock"). Copiado
    /// desde `config.provider` al arrancar; se persiste en `SessionMeta`.
    pub provider: String,
    pub detected_factory: Option<String>,
    #[cfg(feature = "autoskill")]
    pub pending_external_skills: Vec<String>,

    // Caches
    pub caches: CacheLayer,
    /// MCP tool definitions discovered via `tools/list`.
    #[cfg(feature = "mcp")]
    pub mcp_tools: Vec<crate::services::mcp::McpToolInfo>,

    // Doctor
    pub doctor_report: Option<DoctorReport>,

    // Theming
    pub active_theme: ThemeVariant,

    // SubAgents (E22a)
    pub agents: crate::services::agents::AgentRegistry,

    // Multi-agent teams (E22b) — composicion de subagentes hacia un goal comun
    pub teams: crate::services::agents::TeamRegistry,

    // Worktree isolation (E24) — git worktrees por subagente
    pub worktree_manager: crate::services::worktree::WorktreeManager,

    // Cron jobs (E23)
    pub crons: crate::services::cron::CronRegistry,

    // Process monitors (E26) — builds, tests y procesos largos en background
    pub monitors: crate::services::monitor::MonitorRegistry,
    /// Active monitor output panel overlay (E26 completion). `None` = cerrado.
    pub monitor_panel: Option<MonitorPanelState>,

    // LSP Integration (E25) — diagnosticos del language server
    pub lsp: LspState,

    // Plugins (E28) — extensibilidad via trait Plugin
    pub plugins: crate::services::plugins::PluginRegistry,

    // Onboarding (E39)
    pub onboarding: OnboardingState,
    pub platform_hints: PlatformHints,
    /// Tip activo para esta sesion (pickeado una vez al startup, aplica a todos
    /// los screens con scope `Any` o al screen actual con scope especifico).
    pub current_tip: Option<Tip>,

    /// Tick en el que expira la ventana de "doble Ctrl+C para salir". Cuando
    /// es `Some(t)` y `tick_count <= t`, el segundo Ctrl+C confirma la salida.
    /// Cada tick = 250ms; ventana estandar de 8 ticks = 2 segundos.
    pub quit_armed_until: Option<u64>,
}

/// Filter mode for the tool monitor overlay.
#[derive(Debug, Clone, Default, PartialEq)]
pub enum ToolMonitorFilter {
    /// Show all events.
    #[default]
    All,
    /// Only successful completions.
    Ok,
    /// Only errors.
    Errors,
}

impl ToolMonitorFilter {
    /// Cycle to the next filter mode (All → Ok → Errors → All).
    pub fn next(&self) -> Self {
        match self {
            ToolMonitorFilter::All => ToolMonitorFilter::Ok,
            ToolMonitorFilter::Ok => ToolMonitorFilter::Errors,
            ToolMonitorFilter::Errors => ToolMonitorFilter::All,
        }
    }

    pub fn label(&self) -> &str {
        match self {
            ToolMonitorFilter::All => "all",
            ToolMonitorFilter::Ok => "ok",
            ToolMonitorFilter::Errors => "errors",
        }
    }
}

// ── Monitor panel state (E26 completion) ─────────────────────────────────────

/// Estado del overlay de output de un monitor.
#[derive(Debug, Clone)]
pub struct MonitorPanelState {
    /// ID del monitor que se esta mostrando.
    pub monitor_id: String,
    /// Offset de scroll (0 = bottom, crece hacia arriba).
    pub scroll_offset: u16,
    /// Si true, auto-scroll al bottom cuando llegan nuevas lineas.
    pub follow: bool,
}

impl MonitorPanelState {
    pub fn new(monitor_id: String) -> Self {
        Self { monitor_id, scroll_offset: 0, follow: true }
    }
}

// ── LSP state (E25) ─────────────────────────────────────────────────────────

/// Estado del cliente LSP. Vive en AppState.lsp.
#[derive(Debug, Default)]
pub struct LspState {
    /// Nombre del server conectado (e.g. "rust-analyzer"). `None` si no hay.
    pub server_name: Option<String>,
    /// true si el handshake `initialize` completo.
    pub connected: bool,
    /// Diagnosticos acumulados por URI. Se sobreescriben por URI cada vez que
    /// llega un `publishDiagnostics`.
    pub diagnostics: Vec<crate::services::lsp::LspDiagnostic>,
    /// Error si el server fallo al arrancar.
    pub error: Option<String>,
    /// Flag cooperativo de shutdown para el client task.
    pub shutdown: std::sync::Arc<std::sync::atomic::AtomicBool>,
    /// Channel para enviar didOpen/didChange al client task.
    pub cmd_tx: Option<tokio::sync::mpsc::Sender<crate::services::lsp::LspCommand>>,
    /// Archivos que ya recibieron didOpen (para no enviar duplicados).
    pub opened_uris: std::collections::HashSet<String>,
    /// URIs recién modificados (via code block apply). Cleared after diagnostics arrive.
    pub pending_validation: std::collections::HashSet<String>,
}

impl LspState {
    /// Reemplaza diagnosticos para un URI dado. El LSP envia el set completo
    /// por archivo cada vez — no deltas.
    pub fn update_diagnostics(&mut self, uri: &str, new: Vec<crate::services::lsp::LspDiagnostic>) {
        self.diagnostics.retain(|d| d.uri != uri);
        self.diagnostics.extend(new);
        // Cap total para evitar bloat en monorepos.
        if self.diagnostics.len() > 500 {
            self.diagnostics.drain(..self.diagnostics.len() - 500);
        }
    }

    #[allow(dead_code, reason = "consumido por panel de status bar y /lsp-status en Sprint 12")]
    pub fn error_count(&self) -> usize {
        self.diagnostics
            .iter()
            .filter(|d| d.severity == crate::services::lsp::Severity::Error)
            .count()
    }

    #[allow(dead_code, reason = "consumido por panel de status bar en Sprint 12")]
    pub fn warning_count(&self) -> usize {
        self.diagnostics
            .iter()
            .filter(|d| d.severity == crate::services::lsp::Severity::Warning)
            .count()
    }

    /// Notify the LSP server that a file was changed. Sends didOpen first
    /// if this is the first time we touch this URI, then didChange.
    pub fn notify_file_changed(&mut self, path: &str, content: &str) {
        let Some(tx) = &self.cmd_tx else {
            return;
        };
        if !self.connected {
            return;
        }
        let uri = format!("file://{path}");
        let lang_id = detect_language_id(path);

        if !self.opened_uris.contains(&uri) {
            self.opened_uris.insert(uri.clone());
            let _ = tx.try_send(crate::services::lsp::LspCommand::DidOpen {
                uri: uri.clone(),
                language_id: lang_id.to_string(),
                version: 1,
                text: content.to_string(),
            });
        } else {
            let version = 2;
            let _ = tx.try_send(crate::services::lsp::LspCommand::DidChange {
                uri: uri.clone(),
                version,
                text: content.to_string(),
            });
        }
        // Mark for post-apply validation — diagnostics arriving for this URI
        // will trigger a toast.
        self.pending_validation.insert(uri);
    }
}

/// Detect language ID from file extension (for LSP didOpen).
fn detect_language_id(path: &str) -> &'static str {
    match path.rsplit('.').next() {
        Some("rs") => "rust",
        Some("ts") | Some("tsx") => "typescript",
        Some("js") | Some("jsx") => "javascript",
        Some("py") => "python",
        Some("go") => "go",
        Some("java") => "java",
        Some("cs") => "csharp",
        Some("rb") => "ruby",
        Some("c") | Some("h") => "c",
        Some("cpp") | Some("hpp") | Some("cc") => "cpp",
        Some("json") => "json",
        Some("yaml") | Some("yml") => "yaml",
        Some("toml") => "toml",
        Some("md") => "markdown",
        Some("html") => "html",
        Some("css") => "css",
        _ => "plaintext",
    }
}

impl AppState {
    /// Crea un nuevo AppState. `provider` se toma desde `Config` y queda
    /// persistido en `SessionMeta` para que la sesion pueda reportar
    /// correctamente que provider la sirvio.
    pub fn new_with_provider(
        developer: &str,
        model: &str,
        provider: &str,
        default_factory: Option<&str>,
        theme: Option<&str>,
    ) -> Self {
        let active_theme = theme
            .and_then(parse_theme_variant)
            .unwrap_or_else(crate::ui::theme::detection::auto_detect_theme);
        Self {
            screen: AppScreen::Splash,
            mode: AppMode::Normal,
            factory: UiFactory::from_key(default_factory),
            input: String::new(),
            splash_autocomplete: SlashAutocomplete::default(),
            pending_picker: None,
            server_status: ServerStatus::Unknown,
            tick_count: 0,
            config_dirty: false,
            dashboard: DashboardState::new(),
            search: SearchState::new(),
            command: CommandState::new(),
            chat: ChatState::new(),
            wizard: WizardState::new("", model),
            init: InitState::new(),
            model_picker: ModelPickerState::new(),
            theme_picker: None,
            #[cfg(feature = "autoskill")]
            autoskill_picker: None,
            panels: UiPanelFlags::default(),
            toasts: ToastState::default(),
            keybindings: crate::config::load_keybindings(),
            events: Vec::new(),
            sessions: Vec::new(),
            tool_events: Vec::new(),
            hook_events: Vec::new(),
            tool_monitor_filter: ToolMonitorFilter::default(),
            developer: developer.to_string(),
            model: model.to_string(),
            working_dir: std::env::current_dir()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default(),
            git_branch: {
                let cwd = std::env::current_dir().unwrap_or_default();
                std::process::Command::new("git")
                    .args(["rev-parse", "--abbrev-ref", "HEAD"])
                    .current_dir(&cwd)
                    .output()
                    .ok()
                    .filter(|o| o.status.success())
                    .and_then(|o| {
                        let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
                        if s.is_empty() { None } else { Some(s) }
                    })
            },
            provider: provider.to_string(),
            detected_factory: None,
            #[cfg(feature = "autoskill")]
            pending_external_skills: Vec::new(),
            caches: CacheLayer::new(),
            #[cfg(feature = "mcp")]
            mcp_tools: Vec::new(),
            doctor_report: None,
            active_theme,
            agents: crate::services::agents::AgentRegistry::new(),
            teams: crate::services::agents::TeamRegistry::new(),
            worktree_manager: crate::services::worktree::WorktreeManager::new(),
            crons: crate::services::cron::CronRegistry::new(),
            monitors: crate::services::monitor::MonitorRegistry::new(),
            monitor_panel: None,
            lsp: LspState::default(),
            plugins: crate::services::plugins::PluginRegistry::default(),
            onboarding: OnboardingState::load(),
            platform_hints: PlatformHints::detect(),
            current_tip: None,
            quit_armed_until: None,
        }
    }

    /// Invalida todas las `cached_lines` de markdown. Se llama al cambiar
    /// `active_theme` para que el proximo render re-materialice las lineas
    /// con los colores del nuevo tema.
    pub fn invalidate_markdown_caches(&mut self) {
        self.dashboard.preview.cached_lines = None;
        for msg in &mut self.chat.messages {
            msg.cached_lines = None;
        }
    }
}

/// Parsea el nombre del tema desde string — case-insensitive + aliases.
pub fn parse_theme_variant(name: &str) -> Option<ThemeVariant> {
    let n = name.trim().to_lowercase();
    match n.as_str() {
        "hc" => return Some(ThemeVariant::HighContrast),
        "highcontrast" => return Some(ThemeVariant::HighContrast),
        "high-contrast" => return Some(ThemeVariant::HighContrast),
        "tokyo-night" | "tokyo night" | "dark" => return Some(ThemeVariant::TokyoNight),
        _ => {}
    }
    ThemeVariant::ALL.iter().copied().find(|v| v.slug() == n)
}

/// Devuelve el string canonico usado por `parse_theme_variant` para
/// persistir el tema en config.
pub fn theme_variant_to_str(v: ThemeVariant) -> &'static str {
    v.slug()
}
