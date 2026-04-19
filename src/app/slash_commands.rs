use crate::state::{ChatMessage, ChatMode, ChatRole};

use super::App;

fn parse_command(cmd: &str) -> (&str, &str) {
    let parts: Vec<&str> = cmd.splitn(2, ' ').collect();
    (parts[0], parts.get(1).unwrap_or(&"").trim())
}

/// Slashes que migraron a la paleta `:` y ya no son validos como `/`.
/// Muestra hint redirigiendo al usuario en vez del generico "Comando desconocido".
fn is_migrated_to_palette(cmd: &str) -> bool {
    matches!(
        cmd,
        "/theme"
            | "/model"
            | "/permissions"
            | "/doctor"
            | "/audit"
            | "/mcp-status"
            | "/dashboard"
            | "/home"
            | "/init"
            | "/transcript"
            | "/history-search"
            | "/plugins"
            | "/autoskill"
            | "/skills"
            | "/commands"
            | "/adrs"
            | "/policies"
            | "/agents"
            | "/workflows"
            | "/sync"
            | "/health"
            | "/search"
            | "/config"
            | "/disconnect"
            | "/context"
    )
}

impl App {
    pub(crate) fn handle_slash_command(&mut self, cmd: &str) {
        let (command, arg) = parse_command(cmd);
        match command {
            // ── Sesion ─────────────────────────────────────────────────────
            "/clear" | "/exit" | "/history" | "/resume" | "/compact" | "/fork" | "/export"
            | "/undo" | "/redo" => self.exec_session_cmd(command, arg),
            // ── Contexto AI ───────────────────────────────────────────────
            "/diff" | "/files" | "/memory" | "/costs" | "/metrics" => {
                self.exec_context_cmd(command)
            }
            // ── Modo chat ─────────────────────────────────────────────────
            "/plan" => self.toggle_plan_mode(),
            // ── Output AI ─────────────────────────────────────────────────
            "/apply" => self.handle_apply_command(arg),
            "/blocks" => self.handle_blocks_command(),
            // ── Agents / teams / monitores ────────────────────────────────
            "/spawn" => self.handle_spawn_agent_command(arg),
            "/agent-list" => self.handle_agent_list_command(),
            "/agent-cancel" => self.handle_agent_cancel_command(arg),
            "/team-start" => self.handle_team_start_command(arg),
            "/team-list" => self.handle_team_list_command(),
            "/team-cancel" => self.handle_team_cancel_command(arg),
            "/team-mail" => self.handle_team_mail_command(arg),
            "/monitor" => self.handle_monitor_start_command(arg),
            "/monitor-list" => self.handle_monitor_list_command(),
            "/monitor-kill" => self.handle_monitor_kill_command(arg),
            "/monitor-show" => self.handle_monitor_show_command(arg),
            // ── Todos ─────────────────────────────────────────────────────
            "/todos" => self.handle_todos_command(),
            "/todo-add" => self.handle_todo_add_command(arg),
            "/todo-start" => self.handle_todo_start_command(arg),
            "/todo-done" => self.handle_todo_done_command(arg),
            "/todo-remove" => self.handle_todo_remove_command(arg),
            "/todo-clear" => self.handle_todo_clear_command(),
            // ── Memoria persistente ───────────────────────────────────────
            "/remember" => self.handle_remember_command(arg),
            "/forget" => self.handle_forget_command(arg),
            // ── Workflow ──────────────────────────────────────────────────
            "/workflow" => self.exec_workflow_cmd(arg),
            // ── Cron ──────────────────────────────────────────────────────
            "/cron-add" => self.handle_cron_add_command(arg),
            "/cron-list" => self.handle_cron_list_command(),
            "/cron-remove" => self.handle_cron_remove_command(arg),
            // ── Meta ──────────────────────────────────────────────────────
            "/continue" => self.handle_continue_command(),
            "/help" => self.exec_help_cmd(),
            // ── Slashes removidos: sugerir `:` ────────────────────────────
            s if is_migrated_to_palette(s) => self.notify_migrated(s),
            other => self.exec_dynamic_cmd(other, arg),
        }
    }

    /// P3.7: reintentar/continuar la última ronda del chat sin nuevo user
    /// prompt. Útil cuando el AI se queda colgado después de ejecutar tools
    /// (PostToolStallDetector detecta este caso) o cuando el stream falló
    /// por error recuperable.
    fn handle_continue_command(&mut self) {
        if self.state.chat.messages.is_empty() {
            self.notify("Sin mensajes previos para continuar".to_string());
            return;
        }
        if matches!(
            self.state.chat.status,
            crate::state::ChatStatus::Streaming | crate::state::ChatStatus::ExecutingTools
        ) {
            self.notify("Chat ya está activo — espera al estado Ready".to_string());
            return;
        }
        self.state.chat.status = crate::state::ChatStatus::Streaming;
        self.state.chat.scroll_offset = u16::MAX;
        self.notify("▶ Reintentando último turno...".to_string());
        self.spawn_chat_completion();
    }

    fn exec_session_cmd(&mut self, command: &str, arg: &str) {
        match command {
            "/clear" => {
                self.state.chat = crate::state::ChatState::new();
                self.notify("Chat limpiado".to_string());
            }
            "/exit" => {
                self.state.chat = crate::state::ChatState::new();
                self.state.screen = crate::state::AppScreen::Splash;
                self.state.input.clear();
            }
            "/history" => self.handle_sessions_command(),
            "/resume" => self.handle_resume_command(),
            "/fork" => self.handle_fork_command(arg),
            "/export" => self.handle_export_command(arg),
            "/compact" => self.handle_compact_command(arg),
            "/undo" => self.handle_undo_command(),
            "/redo" => self.handle_redo_command(),
            _ => {}
        }
    }

    /// Notifica al usuario que un slash removido ahora vive en la paleta `:`.
    fn notify_migrated(&mut self, cmd: &str) {
        let name = cmd.trim_start_matches('/');
        self.notify(format!("{cmd} se movio a la paleta. Pulsa `:` y busca `{name}`."));
    }

    /// `/workflow <name>` — carga un workflow ingenierIA en el chat.
    fn exec_workflow_cmd(&mut self, arg: &str) {
        if !self.is_mcp_online() {
            self.notify("✗ MCP offline — /workflow no disponible".to_string());
        } else if arg.is_empty() {
            self.notify("Uso: /workflow <nombre>".to_string());
        } else {
            self.spawn_load_workflow(arg.to_string());
            self.notify(format!("Cargando workflow {arg}..."));
        }
    }

    fn exec_context_cmd(&mut self, command: &str) {
        match command {
            "/diff" => self.inject_git_diff(),
            "/files" => self.inject_recent_files(),
            "/memory" => {
                let breakdown = self.state.chat.memory_breakdown();
                let cached = crate::ui::widgets::markdown::render_markdown(
                    &breakdown,
                    &self.state.active_theme.colors(),
                );
                let mut msg = ChatMessage::new(ChatRole::Assistant, breakdown);
                msg.cached_lines = Some(std::sync::Arc::new(cached));
                self.state.chat.messages.push(msg);
                self.state.chat.scroll_offset = u16::MAX;
            }
            "/costs" => self.handle_costs_command(),
            "/metrics" => self.handle_metrics_command(),
            _ => {}
        }
    }

    /// `/metrics` — muestra aggregates de performance del chat (E34).
    fn handle_metrics_command(&mut self) {
        let agg = self.state.chat.metrics.aggregates();
        let body = crate::services::chat::metrics::format_session_summary(&agg);
        let cached =
            crate::ui::widgets::markdown::render_markdown(&body, &self.state.active_theme.colors());
        let mut msg = ChatMessage::new(ChatRole::Assistant, body);
        msg.cached_lines = Some(std::sync::Arc::new(cached));
        self.state.chat.messages.push(msg);
        self.state.chat.scroll_offset = u16::MAX;
    }

    fn exec_help_cmd(&mut self) {
        let help = "## Comandos del chat\n\n\
            Solo los comandos de **conversacion** viven en `/`. Para configuracion, \
            diagnostico o navegacion pulsa `:` (shift+:) y busca.\n\n\
            ### Sesion\n\
            - `/clear` — Limpia el chat\n\
            - `/exit` — Vuelve a splash\n\
            - `/history` — Conversaciones guardadas\n\
            - `/resume` — Retoma ultima conversacion\n\
            - `/fork <label>` — Ramifica la sesion actual\n\
            - `/export [path]` — Exporta la sesion a JSONL\n\
            - `/compact [strategy]` — Compacta mensajes (aggressive|balanced|conservative)\n\
            - `/undo` / `/redo` — Deshace/rehace el ultimo turn\n\
            - `/continue` — Reintenta el ultimo turno del AI\n\n\
            ### Contexto AI\n\
            - `/diff` — Inyecta git diff\n\
            - `/files` — Inyecta archivos recientes\n\
            - `/memory` — Uso de contexto tokens\n\
            - `/costs` — Costos de la sesion\n\
            - `/metrics` — TTFT / OTPS / duracion\n\n\
            ### Modo chat\n\
            - `/plan` — Activa modo planificacion\n\n\
            ### Output AI\n\
            - `/apply [n]` — Aplicar code block\n\
            - `/blocks` — Listar code blocks\n\n\
            ### Agents / teams / monitores\n\
            - `/spawn <role> <prompt>`\n\
            - `/agent-list` · `/agent-cancel <id>`\n\
            - `/team-start <template> <goal>` · `/team-list` · `/team-cancel <id>` · `/team-mail <id>`\n\
            - `/monitor <cmd>` · `/monitor-list` · `/monitor-kill <id>` · `/monitor-show <id>`\n\n\
            ### Todos\n\
            - `/todos` — listar\n\
            - `/todo-add <titulo>` · `/todo-start <id>` · `/todo-done <id>` · `/todo-remove <id>` · `/todo-clear`\n\n\
            ### Memoria persistente\n\
            - `/remember <type> <file>: <body>`\n\
            - `/forget <file>`\n\n\
            ### Workflows\n\
            - `/workflow <name>` — Ejecutar workflow\n\n\
            _Para `theme`, `model`, `doctor`, `autoskill`, `audit`, `mcp-status`, \
            exploradores, cron, plugins, permissions, init, transcript, \
            dashboard, home, history-search ... pulsa `:`._";
        let cached =
            crate::ui::widgets::markdown::render_markdown(help, &self.state.active_theme.colors());
        let mut msg = ChatMessage::new(ChatRole::Assistant, help.to_string());
        msg.cached_lines = Some(std::sync::Arc::new(cached));
        self.state.chat.messages.push(msg);
        self.state.chat.scroll_offset = u16::MAX;
    }

    fn exec_dynamic_cmd(&mut self, other: &str, arg: &str) {
        let cmd_name = other.trim_start_matches('/');

        let is_selected_skill =
            self.state.chat.selected_skill.as_ref().is_some_and(|s| s.name == cmd_name);

        if is_selected_skill {
            self.state.chat.selected_skill.take();
            if !self.is_mcp_online() {
                self.notify(format!("✗ MCP offline — /{cmd_name} no disponible"));
            } else {
                let user_arg = if arg.is_empty() { None } else { Some(arg.to_string()) };
                self.state.chat.pending_workflow_arg = user_arg;
                self.spawn_load_workflow(cmd_name.to_string());
                self.notify(format!("Cargando skill {cmd_name}..."));
            }
        } else {
            self.notify(format!("Comando desconocido: {other}. Usa /help"));
        }
    }

    /// Aplica un theme (con invalidacion de caches y persistencia opcional).
    pub(crate) fn apply_theme(&mut self, variant: crate::ui::theme::ThemeVariant, persist: bool) {
        self.state.active_theme = variant;
        self.state.invalidate_markdown_caches();
        if persist {
            crate::config::persist_theme(crate::state::theme_variant_to_str(variant));
            self.notify(format!("✓ Theme: {}", variant.label()));
        }
    }

    /// Abre el modal selector de themes (live preview + persistencia al Enter).
    pub(crate) fn open_theme_picker(&mut self) {
        let original = self.state.active_theme;
        self.state.theme_picker = Some(crate::state::ThemePickerState::new(original));
        self.state.mode = crate::state::AppMode::ThemePicker;
        self.mark_onboarding_step(crate::services::onboarding::ChecklistStep::Personalize);
    }

    /// Aplica en vivo el tema marcado por el cursor del picker. No persiste.
    pub(crate) fn apply_theme_preview(&mut self) {
        let Some(picker) = self.state.theme_picker.as_ref() else {
            return;
        };
        let Some(variant) = picker.selected() else {
            return;
        };
        if self.state.active_theme == variant {
            return;
        }
        self.apply_theme(variant, false);
    }

    pub(crate) fn exec_doctor_cmd(&mut self) {
        if self.state.panels.show_doctor {
            self.state.panels.show_doctor = false;
            return;
        }
        self.spawn_doctor_checks();
        self.notify("Ejecutando diagnostico...".to_string());
    }

    /// Muestra info del transport MCP actual + tools descubiertos.
    /// Incluye (E17b) el snapshot del lifecycle manager multi-server con
    /// degraded-mode y conteo de tools por server.
    pub(crate) fn handle_mcp_status_command(&mut self) {
        let online = self.is_mcp_online();
        let server = self.client.base_url();
        #[cfg(feature = "mcp")]
        let mcp_tools_count = self.state.mcp_tools.len();
        #[cfg(not(feature = "mcp"))]
        let mcp_tools_count = 0usize;

        let mut msg = String::from("## MCP Status\n\n");
        msg.push_str("### Server ingenierIA (legacy)\n\n");
        msg.push_str(&format!("- **URL**: `{server}`\n"));
        msg.push_str(&format!("- **Status**: {}\n", if online { "✓ online" } else { "✗ offline" }));
        msg.push_str("- **Transport**: SSE (default) — stdio + WebSocket disponibles\n");
        msg.push_str(&format!("- **Tools descubiertos**: {mcp_tools_count}\n\n"));

        #[cfg(feature = "mcp")]
        if mcp_tools_count > 0 {
            msg.push_str("#### Tools disponibles\n\n");
            for tool in self.state.mcp_tools.iter().take(20) {
                let desc = tool.description.as_deref().unwrap_or("(sin descripcion)");
                msg.push_str(&format!("- `{}` — {desc}\n", tool.name));
            }
            msg.push('\n');
        }

        #[cfg(feature = "mcp")]
        self.append_lifecycle_section(&mut msg);

        let cached =
            crate::ui::widgets::markdown::render_markdown(&msg, &self.state.active_theme.colors());
        let mut chat_msg = crate::state::ChatMessage::new(crate::state::ChatRole::Assistant, msg);
        chat_msg.cached_lines = Some(std::sync::Arc::new(cached));
        self.state.chat.messages.push(chat_msg);
        self.state.chat.scroll_offset = u16::MAX;
    }

    /// Agrega la seccion del `McpLifecycleManager` (E17b) al mensaje markdown.
    #[cfg(feature = "mcp")]
    fn append_lifecycle_section(&self, msg: &mut String) {
        let snap = self.mcp_manager.snapshot();
        msg.push_str("### Servers extra (mcp-servers.json)\n\n");
        let path = crate::services::mcp::lifecycle::servers_config_path()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "(config dir no disponible)".to_string());
        msg.push_str(&format!("- **Config**: `{path}`\n"));
        if snap.servers.is_empty() {
            msg.push_str("- Sin servers configurados.\n");
            return;
        }
        let (ready, total) = snap.ready_ratio();
        let tools = snap.total_tools();
        let degraded = if snap.is_degraded() { " ⚠ **degraded**" } else { "" };
        msg.push_str(&format!(
            "- **Servers**: {ready}/{total} ready — {tools} tool(s){degraded}\n\n"
        ));
        msg.push_str("| Server | Transport | Estado | Tools |\n");
        msg.push_str("|--------|-----------|--------|-------|\n");
        for s in &snap.servers {
            let state_cell = match &s.state {
                crate::services::mcp::lifecycle::ServerState::Failed { reason, attempts } => {
                    format!("failed (×{attempts}): {reason}")
                }
                other => other.label().to_string(),
            };
            msg.push_str(&format!(
                "| `{}` | {} | {} | {} |\n",
                s.name,
                s.kind.label(),
                state_cell,
                s.state.tools_count(),
            ));
        }
    }

    /// Muestra estadisticas del audit log o exporta via `--export <path>`.
    pub(crate) fn handle_audit_command(&mut self, arg: &str) {
        let arg = arg.trim();
        if let Some(path_arg) = arg.strip_prefix("--export") {
            let path = path_arg.trim();
            let dest: std::path::PathBuf = if path.is_empty() {
                std::env::temp_dir().join("ingenieria-audit.json")
            } else {
                std::path::PathBuf::from(path)
            };
            match crate::services::audit::export_to(&dest) {
                Ok(count) => {
                    self.notify(format!("✓ {count} entries exportados a {}", dest.display()));
                }
                Err(e) => self.notify(format!("✗ Error en export audit: {e}")),
            }
            return;
        }

        let files = crate::services::audit::list_log_files();
        let mut msg = String::from("## Audit Log\n\n");
        if files.is_empty() {
            msg.push_str("_(audit log vacío — los eventos aparecen al ejecutar tools)_\n");
        } else {
            for path in files.iter().take(10) {
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("?");
                let lines = std::fs::read_to_string(path).map(|c| c.lines().count()).unwrap_or(0);
                msg.push_str(&format!("- `{name}` — {lines} entries\n"));
            }
        }

        // P2.6: citations de @mentions resueltas en este chat.
        let refs: Vec<&crate::state::DocReference> =
            self.state.chat.messages.iter().flat_map(|m| m.context_refs.iter()).collect();
        if !refs.is_empty() {
            msg.push_str("\n## Citations (@mentions resueltas)\n\n");
            for r in &refs {
                msg.push_str(&format!("- `{}` · {}B\n", r.uri, r.bytes));
            }
            msg.push_str(&format!("\n_{} referencia(s) en esta sesión_\n", refs.len()));
        }

        msg.push_str("\n`/audit --export [path]` exportar a JSON");

        let cached =
            crate::ui::widgets::markdown::render_markdown(&msg, &self.state.active_theme.colors());
        let mut chat_msg = crate::state::ChatMessage::new(crate::state::ChatRole::Assistant, msg);
        chat_msg.cached_lines = Some(std::sync::Arc::new(cached));
        self.state.chat.messages.push(chat_msg);
        self.state.chat.scroll_offset = u16::MAX;
    }

    /// `/compact [aggressive|balanced|conservative]` — compacta mensajes viejos.
    /// Sin argumento usa `balanced`. Alias cortos: a/b/c, agr/bal/con.
    fn handle_compact_command(&mut self, arg: &str) {
        use crate::services::compactor::CompactionStrategy;
        let arg = arg.trim();
        let strategy = if arg.is_empty() {
            CompactionStrategy::Balanced
        } else {
            match CompactionStrategy::from_label(arg) {
                Some(s) => s,
                None => {
                    self.notify(format!(
                        "Estrategia invalida: '{arg}'. Opciones: aggressive, balanced, conservative"
                    ));
                    return;
                }
            }
        };
        let outcome = self.state.chat.compact_with_strategy(strategy);
        if outcome.removed_count > 0 {
            self.notify(format!(
                "✓ {} mensajes compactados ({})",
                outcome.removed_count,
                strategy.label()
            ));
        } else {
            self.notify("Nada que compactar".to_string());
        }
    }

    fn toggle_plan_mode(&mut self) {
        if self.state.chat.mode == ChatMode::Planning {
            self.state.chat.mode = ChatMode::Normal;
            self.state.chat.agent_mode = crate::state::AgentMode::Ask;
            self.notify("Modo normal (Ask) activado".to_string());
        } else {
            self.activate_plan_mode();
        }
    }

    /// Activa el modo Plan: inyecta system prompt y setea ChatMode::Planning.
    /// Llamado tanto desde /plan como desde Shift+Tab → AgentMode::Plan.
    pub(crate) fn activate_plan_mode(&mut self) {
        self.state.chat.mode = ChatMode::Planning;
        self.state.chat.agent_mode = crate::state::AgentMode::Plan;
        self.state.chat.messages.push(ChatMessage::new(
            ChatRole::System,
            "MODO PLANNING: Genera un plan estructurado paso a paso. \
             Valida cada paso contra las policies y ADRs de ingenierIA. \
             NO ejecutes cambios, solo planifica. \
             Formato del plan:\n\
             ```\n\
             Plan: [titulo]\n\
             ├─ [1] Paso uno          ○ pending\n\
             ├─ [2] Paso dos          ○ pending\n\
             └─ [3] Paso tres         ○ pending\n\
             ```"
            .to_string(),
        ));
        self.notify("◈ Modo PLAN activado — solo lectura".to_string());
    }
}
