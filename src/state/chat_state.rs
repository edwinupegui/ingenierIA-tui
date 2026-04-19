// ── ChatState ────────────────────────────────────────────────────────────────

use super::chat_types::{
    AgentMode, ChatDisplayMode, ChatMessage, ChatMode, ChatRole, ChatStatus, CostState,
    DocPickerState, PendingElicitation, PendingToolApproval, SelectedSkill, SlashAutocomplete,
};
use super::message_queue::MessageQueue;
use super::transcript::TranscriptView;

pub struct ChatState {
    pub messages: Vec<ChatMessage>,
    pub input: String,
    pub status: ChatStatus,
    pub scroll_offset: u16,
    pub context_loaded: bool,
    /// Number of consecutive tool execution rounds in current turn.
    pub tool_rounds: u8,
    /// Unique session ID for history persistence.
    pub session_id: String,
    /// Tool calls waiting for user approval before execution.
    pub pending_approvals: Vec<PendingToolApproval>,
    /// Detected code blocks from the last assistant message.
    pub code_blocks: Vec<crate::services::codeblocks::CodeBlock>,
    /// Currently selected code block index (for apply action).
    pub code_block_cursor: usize,
    /// Input history for up/down navigation.
    pub input_history: Vec<String>,
    /// Current position in input history (-1 = current input).
    pub history_cursor: Option<usize>,
    /// Saved current input when navigating history.
    pub input_draft: String,
    /// Accumulated cost tracking for this session.
    pub cost: CostState,
    /// Max context window in tokens (model-dependent, default 200K).
    pub max_context: u32,
    /// Chat operating mode (normal or planning).
    pub mode: ChatMode,
    /// Modo de operación del agente (Ask / Auto / Plan). Ciclable con Shift+Tab.
    pub agent_mode: AgentMode,
    /// Whether tool call details (args + results) are expanded.
    pub tools_expanded: bool,
    /// Slash command autocomplete state.
    pub slash_autocomplete: SlashAutocomplete,
    /// MCP document picker state.
    pub doc_picker: DocPickerState,
    /// User argument pending for workflow load.
    pub pending_workflow_arg: Option<String>,
    /// Skill selected from doc picker, waiting for user prompt.
    pub selected_skill: Option<SelectedSkill>,
    /// Last computed max scroll (total_wrapped_lines - visible_height), set by render.
    pub last_max_scroll: std::cell::Cell<u16>,
    /// Elapsed seconds since streaming started (updated by StreamHeartbeat).
    pub stream_elapsed_secs: u16,
    /// Whether the stream is stalled (no delta for 15s+).
    pub stream_stalled: bool,
    /// Multi-message queue: text typed while streaming, drained when AI finishes.
    pub message_queue: MessageQueue,
    /// Stored references for large pastes (>5KB), expanded on send.
    pub pasted_refs: Vec<crate::services::paste_handler::PastedRef>,
    /// Auto-incrementing ID for paste references.
    pub next_paste_id: usize,
    /// Tick at which user last scrolled up (for grace period).
    pub scroll_anchor_tick: Option<u64>,
    /// Indice del primer mensaje NO persistido en JSONL. auto_save_history
    /// hace append de `messages[persisted_msg_count..]` y avanza este contador.
    /// Se resetea a 0 en `/clear` y se pone a `messages.len()` tras `/resume`.
    pub persisted_msg_count: usize,
    /// Ultima estrategia de compactacion aplicada. `None` si aun no se compacto.
    pub last_compaction: Option<crate::services::compactor::CompactionStrategy>,
    /// Elicitation pendiente (modal abierto) — E18.
    pub pending_elicitation: Option<PendingElicitation>,
    /// Display mode para el chat (Normal o Brief) — E33.
    pub display_mode: ChatDisplayMode,
    /// Transcript overlay (Ctrl+O) — E33.
    pub transcript: TranscriptView,
    /// Performance metrics (TTFT, OTPS, turn duration) — E34.
    pub metrics: crate::services::chat::metrics::ChatMetrics,
    /// Modal de busqueda de historial (Ctrl+R) — E30b.
    pub history_search: Option<crate::state::history_search::HistorySearch>,
    /// Lista de todos asociada al chat (E12). Se alimenta desde `/plan` al
    /// aprobar, o manualmente via `/todo-add`, `/todo-done`, `/todo-clear`.
    pub todos: crate::domain::todos::TodoList,
    /// Ultimo plan parseado desde el modo planning (E12). `None` mientras no
    /// haya un plan candidato o tras aprobarlo/descartarlo.
    pub pending_plan: Option<crate::domain::todos::Plan>,
    /// Stack de undo/redo del input (E40). Se alimenta en cada mutacion del
    /// buffer `input` (char push, backspace, paste, history nav, draft load).
    pub input_undo: crate::state::input_undo::InputUndoStack,
    /// Contenido del draft que se conoce persistido en disco (E40).
    /// Evita I/O redundante: solo se vuelve a escribir cuando difiere.
    pub persisted_draft: String,
    /// Sidebar derecho visible (toggle con ctrl+b). Default true.
    pub sidebar_visible: bool,
    /// Message navigator expandido (muestra previews) vs compacto (ticks).
    pub nav_expanded: bool,
    /// Índice del user message seleccionado en el navigator (cursor lógico).
    /// Indexa sobre la lista filtrada de `ChatRole::User`, no sobre `messages`.
    pub nav_user_cursor: Option<usize>,
    /// Offsets (en líneas visuales) de cada user message, cacheados por
    /// `render_messages`. Consumido por el handler de `[`/`]` para calcular
    /// scroll_offset al saltar entre turnos sin reconstruir heights.
    pub last_user_offsets: std::cell::RefCell<Vec<u16>>,
    /// Picker de `@` mentions. Modal sobre el input que inserta referencias
    /// a documentos (skills/agents/etc.) como contexto para la AI.
    pub mention_picker: crate::state::mention_picker::MentionPicker,
    /// Cursor del modal de aprobación multi-tool (P1.3). Indexa sobre
    /// `pending_approvals`. Reset a 0 cuando la lista se vacía o se reconstruye.
    pub approval_cursor: usize,
    /// Instant en que entramos al estado `ExecutingTools` con pendientes. `None`
    /// mientras no hay approval activa. Usado por el tick handler para decidir
    /// auto-approve / auto-deny por timeout según PermissionMode.
    pub approval_started_at: Option<std::time::Instant>,
    /// Timeout del modal de aprobación en ms. Default 60s; puede ajustarse
    /// desde `/permissions` (E17) o /config a futuro.
    pub approval_timeout_ms: u64,
    /// Stack de `(popped_messages, draft_before_undo)` por cada /undo
    /// consecutivo. /redo pop-ea del final y restaura. Se limpia al enviar
    /// un nuevo mensaje (para no restaurar turns stale que ya no aplican).
    pub undo_redo_stack: Vec<(Vec<ChatMessage>, String)>,
}

impl ChatState {
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
            input: String::new(),
            status: ChatStatus::LoadingContext,
            scroll_offset: 0,
            context_loaded: false,
            tool_rounds: 0,
            session_id: crate::services::session::generate_session_id(),
            pending_approvals: Vec::new(),
            code_blocks: Vec::new(),
            code_block_cursor: 0,
            input_history: Vec::new(),
            history_cursor: None,
            input_draft: String::new(),
            cost: CostState::default(),
            max_context: 200_000,
            mode: ChatMode::default(),
            agent_mode: AgentMode::default(),
            tools_expanded: false,
            slash_autocomplete: SlashAutocomplete::default(),
            doc_picker: DocPickerState::default(),
            pending_workflow_arg: None,
            selected_skill: None,
            last_max_scroll: std::cell::Cell::new(0),
            stream_elapsed_secs: 0,
            stream_stalled: false,
            message_queue: MessageQueue::new(),
            pasted_refs: Vec::new(),
            next_paste_id: 1,
            scroll_anchor_tick: None,
            persisted_msg_count: 0,
            last_compaction: None,
            pending_elicitation: None,
            display_mode: ChatDisplayMode::default(),
            transcript: TranscriptView::default(),
            metrics: crate::services::chat::metrics::ChatMetrics::new(),
            history_search: None,
            todos: crate::domain::todos::TodoList::new(),
            pending_plan: None,
            input_undo: crate::state::input_undo::InputUndoStack::new(),
            persisted_draft: String::new(),
            sidebar_visible: true,
            nav_expanded: false,
            nav_user_cursor: None,
            last_user_offsets: std::cell::RefCell::new(Vec::new()),
            mention_picker: crate::state::mention_picker::MentionPicker::new(),
            approval_cursor: 0,
            approval_started_at: None,
            approval_timeout_ms: 60_000,
            undo_redo_stack: Vec::new(),
        }
    }

    /// Mueve el cursor del navigator al user message siguiente/previo y
    /// alinea `scroll_offset` para que el turno seleccionado quede visible.
    ///
    /// `delta` positivo avanza hacia el turno más reciente, negativo
    /// retrocede. Usa `last_user_offsets` (poblado por el último render);
    /// si la cache está vacía no mueve nada.
    pub fn nav_move(&mut self, delta: i32) {
        let offsets = self.last_user_offsets.borrow();
        if offsets.is_empty() {
            return;
        }
        let max = offsets.len() - 1;
        let current = self.nav_user_cursor.unwrap_or(max);
        let next = (current as i32 + delta).clamp(0, max as i32) as usize;
        let target_offset = offsets[next];
        drop(offsets);
        self.nav_user_cursor = Some(next);
        self.scroll_offset = target_offset;
    }

    /// Scroll up by `n` lines. Resolves u16::MAX (auto-bottom) before subtracting.
    /// Sets scroll anchor to prevent auto-scroll during streaming.
    pub fn scroll_up(&mut self, n: u16) {
        let max = self.last_max_scroll.get();
        let current = if self.scroll_offset == u16::MAX { max } else { self.scroll_offset };
        self.scroll_offset = current.saturating_sub(n);
    }

    /// Scroll down by `n` lines. Snaps to u16::MAX (auto-bottom) when reaching max.
    pub fn scroll_down(&mut self, n: u16) {
        if self.scroll_offset == u16::MAX {
            return; // already at bottom
        }
        let max = self.last_max_scroll.get();
        let new_val = self.scroll_offset.saturating_add(n);
        if new_val >= max {
            self.scroll_offset = u16::MAX;
        } else {
            self.scroll_offset = new_val;
        }
    }

    /// Force scroll to bottom and clear any manual-scroll anchor. Llamar antes
    /// de push de mensajes nuevos (user/tool result/assistant continuation)
    /// para garantizar que el viewport siga el contenido. Equivale a lo que
    /// opencode-dev hace en `App.tsx::scrollToBottom` al append.
    pub fn snap_to_bottom(&mut self) {
        self.scroll_offset = u16::MAX;
        self.scroll_anchor_tick = None;
    }

    /// Save current input to history and clear.
    pub fn push_to_history(&mut self) {
        const MAX_INPUT_HISTORY: usize = 200;
        if !self.input.trim().is_empty() {
            self.input_history.push(self.input.clone());
            if self.input_history.len() > MAX_INPUT_HISTORY {
                self.input_history.drain(..self.input_history.len() - MAX_INPUT_HISTORY);
            }
        }
        self.history_cursor = None;
        self.input_draft.clear();
    }

    /// Navigate to previous input in history.
    pub fn history_up(&mut self) {
        if self.input_history.is_empty() {
            return;
        }
        match self.history_cursor {
            None => {
                self.input_draft = self.input.clone();
                let idx = self.input_history.len() - 1;
                self.history_cursor = Some(idx);
                self.input = self.input_history[idx].clone();
            }
            Some(idx) if idx > 0 => {
                let new_idx = idx - 1;
                self.history_cursor = Some(new_idx);
                self.input = self.input_history[new_idx].clone();
            }
            _ => {}
        }
    }

    /// Navigate to next input in history (or back to draft).
    pub fn history_down(&mut self) {
        if let Some(idx) = self.history_cursor {
            if idx + 1 < self.input_history.len() {
                let new_idx = idx + 1;
                self.history_cursor = Some(new_idx);
                self.input = self.input_history[new_idx].clone();
            } else {
                self.history_cursor = None;
                self.input = self.input_draft.clone();
            }
        }
    }

    /// Estimate total context tokens from all messages (chars / 4 heuristic).
    pub fn estimated_context_tokens(&self) -> u32 {
        self.messages.iter().map(|m| (m.content.len() as u32) / 4).sum()
    }

    /// Context usage as percentage of max window.
    pub fn context_percent(&self) -> f64 {
        if self.max_context == 0 {
            return 0.0;
        }
        (self.estimated_context_tokens() as f64 / self.max_context as f64) * 100.0
    }

    /// Auto-compact threshold derivado de la estrategia `Balanced` (default, 80%).
    /// Usar en handler_events para decidir si disparar `compact_with_strategy(Balanced)`.
    pub fn needs_auto_compaction(&self) -> bool {
        let cfg = crate::services::compactor::CompactionStrategy::Balanced.config();
        self.context_percent() >= cfg.trigger_percent
    }

    /// Compacta con una estrategia explicita. Retorna el outcome completo.
    pub fn compact_with_strategy(
        &mut self,
        strategy: crate::services::compactor::CompactionStrategy,
    ) -> crate::services::compactor::CompactionOutcome {
        let outcome = crate::services::compactor::compact(&self.messages, strategy);
        if outcome.removed_count > 0 {
            self.messages = outcome.messages.clone();
            self.last_compaction = Some(strategy);
            self.scroll_offset = u16::MAX;
        }
        outcome
    }

    /// Generate a context breakdown for /memory command.
    pub fn memory_breakdown(&self) -> String {
        let mut system_tokens = 0u32;
        let mut user_tokens = 0u32;
        let mut assistant_tokens = 0u32;
        let mut tool_tokens = 0u32;
        let mut user_count = 0u32;
        let mut assistant_count = 0u32;
        let mut tool_count = 0u32;

        for m in &self.messages {
            let toks = (m.content.len() as u32) / 4;
            match m.role {
                ChatRole::System => system_tokens += toks,
                ChatRole::User => {
                    user_tokens += toks;
                    user_count += 1;
                }
                ChatRole::Assistant => {
                    assistant_tokens += toks;
                    assistant_count += 1;
                }
                ChatRole::Tool => {
                    tool_tokens += toks;
                    tool_count += 1;
                }
            }
        }

        let total = system_tokens + user_tokens + assistant_tokens + tool_tokens;
        let pct = self.context_percent();
        let max_k = self.max_context / 1000;

        format!(
            "## Contexto cargado\n\n\
             ```\n\
             Sistema            {:>6} tok\n\
             Usuario ({:>2} msgs)  {:>6} tok\n\
             Asistente ({:>2} msgs) {:>5} tok\n\
             Tools ({:>2} results)  {:>5} tok\n\
             ─────────────────────────\n\
             Total: {:>6} / {}K tok ({:.0}%)\n\
             ```\n",
            system_tokens,
            user_count,
            user_tokens,
            assistant_count,
            assistant_tokens,
            tool_count,
            tool_tokens,
            total,
            max_k,
            pct,
        )
    }
}
