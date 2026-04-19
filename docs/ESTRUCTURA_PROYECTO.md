# Estructura del Proyecto

## Estructura actual

```
sc-ingenieria-tui/
|
|-- crates/                                # Workspace crates (79 archivos Rust)
|   |-- ingenieria-domain/src/ (18)          # Tipos puros del dominio (zero deps internas)
|   |-- ingenieria-ui/src/ (22)              # Theme system, design system, a11y, primitives
|   |-- ingenieria-api/src/ (7)              # API types, pricing, metrics, retry
|   |-- ingenieria-runtime/src/ (17)         # Session, audit, memory, permissions, config validation
|   +-- ingenieria-tools/src/ (15)           # Bash safety, hooks, MCP protocol types
|
|-- src/
|   |-- main.rs                            # Entry point: bootstrap, event loop, worker spawn
|   |-- actions.rs                         # Enum Action con 100+ variantes
|   |-- config.rs                          # Carga de config (env, .mcp.json, ~/.config)
|   |
|   |-- app/ (33 archivos)                 # Reducer: unico lugar que muta AppState
|   |   |-- mod.rs                         # App struct, dispatch handle(), plugin hooks
|   |   |-- keys.rs, keys_chat.rs,         # Keybindings por screen (global, chat, wizard, splash)
|   |   |   keys_wizard.rs, keys_splash.rs
|   |   |-- handler_actions.rs             # Handlers: dashboard, init, focus, documents
|   |   |-- handler_events.rs              # Handlers: server events, health, sync
|   |   |-- slash_commands.rs              # Dispatcher `/` chat-only (42 slashes; ver docs/COMANDOS.md)
|   |   |-- spawners.rs, spawners_chat.rs  # Spawn de tasks async (docs, chat, tools)
|   |   |-- chat_tools.rs                  # Tool call execution, approval workflows
|   |   |-- chat_history.rs                # Chat context loading, history
|   |   |-- chat_codeblocks.rs             # Code block application + LSP notifications
|   |   |-- commands.rs                    # `:` palette dispatcher (26 ops/config/nav ids)
|   |   |-- quit_handler.rs                # Double Ctrl+C escalation (armar/salir)
|   |   |-- autoskill_handler.rs           # Modal Autoskill (feature `autoskill`)
|   |   |-- wizard.rs                      # Wizard screen flow
|   |   |-- agents_handler.rs              # Sub-agent spawning (E22a)
|   |   |-- team_handler.rs                # Multi-agent teams (E22b)
|   |   |-- cron_handler.rs                # Cron job execution (E23)
|   |   |-- lsp_handler.rs                 # Language server integration (E25)
|   |   |-- monitor_handler.rs             # Process monitor management (E26)
|   |   |-- bridge_handler.rs              # IDE bridge HTTP handler (E27)
|   |   |-- plugin_handler.rs              # Plugin lifecycle (E28)
|   |   |-- elicitation_handler.rs         # MCP elicitation requests (E18)
|   |   |-- hooks_handler.rs               # Hook execution (E16)
|   |   |-- config_tool_handler.rs         # ConfigTool AI-driven changes (E20)
|   |   |-- input_dx_handler.rs            # Input DX: undo/redo (E40)
|   |   |-- memory_commands.rs             # CLAUDE.md memory system
|   |   |-- todos_handler.rs               # /todo command parsing
|   |   |-- transcript_handler.rs          # Transcript overlay (`:transcript`)
|   |   |-- history_bridge.rs              # History persistence bridge
|   |   |-- history_search_handler.rs      # Input history search (`:history-search`)
|   |   +-- tests.rs                       # Tests del reducer
|   |
|   |-- state/ (15 archivos)              # AppState y sub-states
|   |   |-- mod.rs                         # AppState master struct (35+ campos)
|   |   |-- chat_state.rs                  # ChatState: mensajes, scroll, stream
|   |   |-- chat_types.rs                  # ChatMessage, ChatStatus, ToolCall, ChatMode
|   |   |-- cost.rs                        # Token usage tracking, cost calculation
|   |   |-- dashboard_state.rs             # DashboardState, SearchState, sidebar
|   |   |-- wizard_state.rs                # WizardState: URL, name, role, provider
|   |   |-- command_state.rs               # CommandState, ModelPickerState, ThemePickerState, PALETTE_COMMANDS (26)
|   |   |-- autoskill_picker.rs            # AutoskillItem + AutoskillPickerState (feature `autoskill`)
|   |   |-- cache_state.rs                 # CacheLayer: docs, searches, suggestions
|   |   |-- init_state.rs                  # InitState: project type, results
|   |   |-- event_state.rs                 # TimedEvent, ActiveSession
|   |   |-- transcript.rs                  # TranscriptView
|   |   |-- history_search.rs              # History search state (`:history-search`)
|   |   |-- input_undo.rs                  # Input undo/redo stack (E40)
|   |   |-- message_queue.rs               # Message queue para async dispatch
|   |   +-- selectors.rs                   # Cache selector patterns (E08)
|   |
|   |-- domain/                            # Re-export de ingenieria-domain
|   |   +-- mod.rs
|   |
|   |-- services/ (13 subdirectorios + 20 archivos sueltos)
|   |   |-- agents/ (5)                    # AgentRegistry, spawner, roles, teams (E22a/b)
|   |   |-- auth/ (2)                      # Copilot OAuth device flow
|   |   |-- bridge/ (3)                    # IDE Bridge HTTP server (E27)
|   |   |-- chat/ (8)                      # ChatProvider trait + Claude/Copilot/Mock impls
|   |   |-- compactor/ (4)                 # Context compaction (E19)
|   |   |-- cron/ (5)                      # Cron scheduler: jobs, registry, store (E23)
|   |   |-- hooks/ (2)                     # Event hooks configurables (E16)
|   |   |-- lsp/ (6)                       # LSP client, diagnostics (E25)
|   |   |-- mcp/ (16)                      # MCP client, transports, lifecycle manager
|   |   |   |-- lifecycle/ (5)             #   config, manager, state, retry
|   |   |   +-- transports/ (4)            #   WebSocket, Stdio, SSE
|   |   |-- onboarding/ (4)                # Checklist + tips (E39)
|   |   |-- permissions/ (2)               # Tool permission enforcer (E17)
|   |   |-- structured_output/ (2)         # JSON extraction (E19)
|   |   |-- tools/ (4)                     # Tool registry, FS read/write, config tool (E20)
|   |   |-- autoskill_map.rs               # Tech detection + skill mapping
|   |   |-- cache.rs, doc_cache.rs         # Document caching
|   |   |-- context.rs                     # Chat context building (system prompts)
|   |   |-- doctor.rs                      # Diagnostic self-check
|   |   |-- ingenieria_client.rs             # HTTP REST client
|   |   |-- features.rs                    # Feature flag queries
|   |   |-- monitor.rs                     # Process monitoring (E26)
|   |   |-- plugins.rs                     # Plugin system (E28)
|   |   |-- recovery_engine.rs             # Crash recovery recipes (E42)
|   |   |-- worktree.rs                    # Git worktree isolation (E24)
|   |   +-- ... (otros: copilot, history, sync, uri, etc.)
|   |
|   |-- ui/ (50 archivos)                 # Render puro, &AppState inmutable
|   |   |-- mod.rs                         # render() dispatch por screen
|   |   |-- chat.rs, chat_render.rs        # Chat screen + message rendering
|   |   |-- chat_tools.rs                  # Tool call display + approval UI
|   |   |-- dashboard.rs                   # Dashboard: header, sidebar, preview
|   |   |-- wizard.rs, wizard_auth.rs,     # Wizard screens
|   |   |   wizard_model.rs
|   |   |-- init.rs                        # Init screen
|   |   |-- diff_render.rs, diff_syntax.rs,# Diff rendering
|   |   |   diff_word.rs
|   |   |-- tool_display.rs               # Tool call rendering
|   |   |-- markdown/ (5)                  # Markdown pipeline, cache, streaming
|   |   |-- virtual_scroll.rs             # Virtual scrolling
|   |   |-- hyperlinks.rs                 # Hyperlink detection
|   |   |-- msg_height.rs                 # Message height calculation
|   |   +-- widgets/ (30+ archivos)        # Componentes reutilizables
|   |       |-- agent_panel.rs             # Sub-agent status (E22a)
|   |       |-- autoskill_picker.rs        # Modal Autoskill (feature `autoskill`)
|   |       |-- command_palette.rs         # `:` palette con headers inline compactos
|   |       |-- cost_panel.rs              # Token cost breakdown
|   |       |-- elicitation_modal.rs       # MCP elicitation UI (E18)
|   |       |-- model_picker.rs            # Selector de modelos AI (`:model`)
|   |       |-- monitor_panel.rs           # Process monitor output (E26)
|   |       |-- onboarding_checklist.rs    # Onboarding checklist (E39)
|   |       |-- permission_modal.rs        # Tool approval modal
|   |       |-- theme_picker.rs            # ThemePicker modal con live preview
|   |       |-- transcript_modal.rs        # Transcript overlay (`:transcript`)
|   |       |-- toasts.rs                  # Toast notifications
|   |       |-- tip_card.rs                # Tip display card
|   |       +-- ... (otros: gauge, hints, markdown, etc.)
|   |
|   |-- workers/ (11 archivos)            # Tasks async de larga duracion
|   |   |-- keyboard.rs                    # Input event polling (4 modifier bindings; ver docs/KEYBINDINGS.md)
|   |   |-- tick.rs                        # 250ms heartbeat
|   |   |-- health.rs                      # Health check loop
|   |   |-- sse.rs                         # SSE streaming + reconnect
|   |   |-- hook_events.rs                 # Hook event polling (E16)
|   |   |-- tool_events.rs                # Tool event polling
|   |   |-- cron_worker.rs                # Cron job firing (E23)
|   |   |-- file_watcher.rs               # Config/CLAUDE.md/.env watching (E42)
|   |   |-- process_monitor.rs            # Build/test output capture (E26)
|   |   |-- lifecycle.rs                   # Worker lifecycle framework (E08)
|   |   +-- mod.rs                         # Re-exports
|   |
|   |-- registries/                        # Singletons globales (OnceLock)
|   |   +-- mod.rs
|   |
|   +-- utils/                             # Helpers de texto
|       |-- mod.rs
|       +-- text/                          # truncate, visible_width
|
|-- .agents/skills/                        # Skill definitions (archivos reales)
|   +-- rust-best-practices/               # Skill con 9 reference chapters
|
|-- .claude/skills/                        # Symlinks a .agents/skills/
|   +-- rust-best-practices -> ../../.agents/skills/rust-best-practices
|
|-- docs/                                  # Documentacion del proyecto
|   |-- ARQUITECTURA_Y_ESTANDARES.md
|   |-- AUDITORIA.md                       # Checklist para auditar calidad
|   |-- COMANDOS.md                        # Split `/` (42) vs `:` (26) canonico
|   |-- ERROR_HANDLING.md
|   |-- ESTRUCTURA_PROYECTO.md             # (este archivo)
|   |-- FLUJOS_Y_OBJETIVOS.md
|   |-- KEYBINDINGS.md                     # Atajos de teclado + double Ctrl+C
|   |-- LINEAMIENTOS_VISUALES.md           # Theme system (6 variantes + TokyoNight default)
|   +-- OWNERSHIP_Y_PERFORMANCE.md
|-- installers/                            # Scripts de instaladores (macOS, Windows)
|-- scripts/                               # Scripts de instalacion
|-- CLAUDE.md                              # Instrucciones para desarrollo con IA
|-- Cargo.toml                             # Workspace root manifest
+-- Cargo.lock
```

---

## Reglas de organizacion

### Tamano maximo de archivos

| Tipo de archivo | Maximo lineas | Accion si se excede |
|----------------|--------------|-------------------|
| Handler (`app/*.rs`) | 400 | Extraer subfunciones o dividir |
| Screen (`ui/*.rs`) | 400 | Extraer widgets a `widgets/` |
| Service (`services/*.rs`) | 300 | Dividir por responsabilidad |
| Widget (`widgets/*.rs`) | 200 | Simplificar |
| State (`state/*.rs`) | 200 | Dividir por screen |
| Worker (`workers/*.rs`) | 100 | Son simples por diseno |
| Domain (`domain/*.rs`, `crates/ingenieria-domain/`) | 100 | Son solo tipos |
| `main.rs` | 80 | Solo bootstrap |
| `actions.rs` | 150 | Solo enum + variantes |

> Estos limites aplican a **codigo nuevo**. Los archivos existentes que exceden se documentan como excepciones abajo.

### Excepciones conocidas (backlog de refactoring)

| Archivo | Actual | Limite | Categoria | Razon / Plan |
|---------|--------|--------|-----------|-------------|
| `services/autoskill_map.rs` | 837 | 300 | Service | Mapa de deteccion data-heavy; candidato a code-gen o split por factory |
| `app/tests.rs` | 829 | 400 | Test | Suite creciente; dividir por dominio de test |
| `app/mod.rs` | 710 | 400 | Handler | Dispatcher central; extraer mas handlers a archivos dedicados |
| `state/chat_types.rs` | 624 | 200 | State | Tipos del chat + SLASH_COMMANDS (42); mover tipos a ingenieria-domain y el array a `state/slash_commands.rs` |
| `app/keys.rs` | 580 | 400 | Handler | Keybindings extensos + pickers; posible split por screen |
| `ui/chat_render.rs` | 579 | 400 | Screen | Rendering complejo; extraer sub-widgets |
| `state/mod.rs` | 512 | 200 | State | AppState grande; dividir sub-states a archivos propios |
| `app/slash_commands.rs` | 483 | 400 | Handler | Dispatcher + help markdown + hint migracion; candidato a macro o tabla declarativa |
| `ui/dashboard.rs` | 430 | 400 | Screen | Over-limit; extraer sidebar/preview widgets |
| `crates/ingenieria-domain/src/todos.rs` | 423 | 100 | Domain | Tipos de todos data-heavy (TodoItem, TodoStats, filters); dividir en sub-modules si crece mas |
| `crates/ingenieria-domain/src/failure.rs` | 325 | 100 | Domain | StructuredFailure + FailureKind variants extensas; candidato a split failure/kind |
| `crates/ingenieria-domain/src/recovery.rs` | 260 | 100 | Domain | RecoveryRecipe + RecoveryStep variants; companion de failure.rs |
| `crates/ingenieria-domain/src/plugin.rs` | 186 | 100 | Domain | PluginManifest + tipos de eventos; split si se agregan mas eventos |

### Cuando crear un nuevo archivo

- Cuando una responsabilidad nueva no encaja en ninguno existente
- Cuando un archivo supera su limite de lineas

### Cuando NO crear un nuevo archivo

- Para una sola funcion helper (ponerla como `fn` privada en el modulo)
- Para un solo struct (agregarlo al archivo mas cercano)

---

## Dependencias entre modulos

### Workspace crates

```
ingenieria-domain (cero deps internas)
  ^
ingenieria-ui          ingenieria-runtime         ingenieria-api
(dep: domain)        (dep: domain)            (standalone)
  ^                    ^
ingenieria-tools (dep: domain, runtime)
```

### Binary (`src/`)

```
main.rs
  +-- app/      (depende de: state, actions, services, workers)
  +-- state/    (depende de: domain)
  +-- actions/  (depende de: domain)
  +-- domain/   (re-export de ingenieria-domain, no depende de nada interno)
  +-- services/ (depende de: domain, config, workspace crates)
  +-- ui/       (depende de: state, domain, [tipos puros de services/])
  +-- workers/  (depende de: actions, services)
  +-- config    (depende de: domain)
```

**Regla critica**: `ui/` NUNCA importa **logica** de `services/`. Excepcion documentada: tipos de datos puros (structs/enums sin metodos) necesarios para render. Archivos con esta excepcion:
- `widgets/onboarding_checklist.rs` — importa `ChecklistStep`, `ChecklistState`
- `widgets/monitor_panel.rs` — importa `MonitorInfo`, `MonitorLine`
- `widgets/agent_panel.rs` — importa `AgentRegistry`, `AgentStatus`
- `widgets/tip_card.rs` — importa `Tip`
- `widgets/elicitation_modal.rs` — importa `ElicitationField`

`domain/` (y `ingenieria-domain`) NUNCA depende de ningun otro modulo interno.

---

## Visibilidad

| Visibilidad | Cuando usarla |
|------------|--------------|
| `pub` | Solo si otro modulo fuera del directorio lo necesita |
| `pub(crate)` | Si otros modulos del crate lo necesitan |
| `pub(super)` | Si solo el modulo padre lo necesita |
| privado (default) | Todo lo demas |

---

## Workspace crates: donde van los tipos nuevos

| Tipo de dato | Crate destino | Ejemplo |
|-------------|--------------|---------|
| Domain types (structs del modelo) | `ingenieria-domain` | `ChatRole`, `DocumentSummary`, `ToolEvent` |
| UI primitives, theme tokens | `ingenieria-ui` | `ColorTheme`, `DesignTokens` |
| API types, pricing | `ingenieria-api` | `ModelPricing`, `RetryConfig` |
| Session, audit, memory | `ingenieria-runtime` | `SessionStore`, `AuditEntry` |
| Tool safety, hooks | `ingenieria-tools` | `BashValidator`, `HookConfig` |

`src/domain/mod.rs` y `src/ui/mod.rs` re-exportan desde workspace crates. Usar `crate::domain::*` y `crate::ui::theme::*` sigue funcionando.
