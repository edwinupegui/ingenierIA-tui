# Flujos y Objetivos de ingenierIA TUI

## Que es ingenierIA TUI

ingenierIA TUI es el panel de control visual del ecosistema ingenierIA. Si ingenierIA MCP Server es el cerebro que organiza skills, politicas, ADRs y workflows, la TUI es los ojos y las manos del desarrollador. Le da visibilidad sobre todo lo que el servidor sabe y le permite interactuar sin salir de la terminal.

---

## El ecosistema

```
ingenierIA MCP Server (Node.js, puerto 3001)
    |
    |-- REST API (/api/health, /api/documents, /api/search)
    |-- SSE Stream (/api/events)
    |-- MCP Protocol (/claude/sse) [8 tools disponibles]
    |
    +-- ingenierIA TUI (Rust, monorepo con 5 workspace crates)
            |
            |-- Consume REST para documentos, busqueda, salud
            |-- Consume SSE para eventos en tiempo real
            |-- MCP Client integrado (tool calling directo al servidor)
            |-- Chat con GitHub Copilot, Claude API o Mock provider (streaming)
            |-- Herramientas locales de filesystem + config para el AI
            |-- SubAgents, Teams, Cron, LSP, Plugins, IDE Bridge
```

---

## Flujos principales

### 1. Primera vez: El Wizard

4 pasos: Servidor URL -> Identidad -> Proveedor de AI -> Rol.

- **Proveedor de AI**: GitHub Copilot (OAuth device flow) o Claude API (API key directa)
- **Rol**: Backend/.NET, Frontend/Angular o Full Stack — define el factory por defecto
- **Auto-deteccion**: Al terminar, detecta automaticamente el tech stack del proyecto actual (factory, sub-tecnologias)

Guarda en `~/.config/ingenieria-tui/config.json`.

### 2. El Splash

Pantalla de bienvenida con logo ASCII. Opciones: `Enter` (Dashboard), `c` (Chat), `i` (Init proyecto). Navegacion con `Esc`: alterna entre chat y dashboard (ya no cierra la app).

### 3. El Dashboard

Pantalla principal con tres zonas:

- **Header**: Estado del servidor (online/offline), tabs de factory (Net/Ang/All) con Tab, conteo de documentos, identidad, modelo AI, indicador de actividad, context gauge
- **Sidebar**: Arbol de categorias (Skills, ADRs, Policies, Workflows). Navegacion vim (`j`/`k`), expand/collapse (`Space`), seleccion (`Enter`). Busqueda difusa con `/`. Skills ordenados por factory detectado (stack prioritario en verde)
- **Preview**: Contenido markdown renderizado con syntax highlighting (Rust/TS/C#), bordes Unicode en code blocks, diff coloreado. Scroll con `PageUp/PageDown` o `Alt+↑/↓`. Copia con `y`

### 4. La busqueda

- **Difusa (< 3 chars)**: nucleo-matcher local, instantanea
- **Servidor (>= 3 chars)**: HTTP a `/api/search`, busca en contenido completo

### 5. El Command Palette

Se activa con `:` (shift+:). Solo configuracion / operaciones / navegacion —
26 entries agrupadas en: Sincronizacion, Estado/Diagnostico, Contexto de
ingenieria, Navegacion, Exploradores, Configuracion, Instalacion, Historial
del input.

La conversacion (turnos, contexto AI, agents, todos, memoria) vive en `/`
slash commands (42 entries). Ver [`COMANDOS.md`](./COMANDOS.md) para la
taxonomia canonica y [`KEYBINDINGS.md`](./KEYBINDINGS.md) para atajos.

### 6. El Chat

Interfaz de conversacion con AI (Copilot, Claude API o Mock) con streaming.

**Flujo**:
1. Carga documentos del servidor y construye system prompt con politicas, ADRs, resumenes
2. El usuario escribe y envia
3. Streaming de respuesta con markdown renderizado, spinners braille, thinking indicator
4. Si el modelo necesita contexto, hace tool calls (filesystem local + MCP tools del servidor)
5. Tool calls visibles expandibles con `t` (JSON + resultados)

**Herramientas disponibles**:
- Filesystem: `read_file`, `list_directory`, `search_files`, `write_file`, `create_file`
- Config: `config_tool` (E20) — modifica config no-sensible via AI
- MCP: `get_factory_context`, `get_workflow`, `validate_compliance`, `sync_project`, `search_documents`, `get_document`, `list_factories`, `get_agents`

**Features del chat**:
- Input multilinea + historial de mensajes previos
- Slash command autocomplete: escribir `/` muestra popup con los 42 slashes disponibles
- Session persistence (`/resume`, `/history`, `/fork`, `/export`, `/compact`)
- Context injection (`/diff`, `/files`, `/memory`, `/costs`, `/metrics`)
- Code block tools (`/blocks`, `/apply`)
- Planning mode (`/plan`) con validacion compliance automatica
- Agents/teams/monitors (`/spawn`, `/team-*`, `/monitor*`)
- Todos de sesion (`/todos`, `/todo-*`)
- Memoria persistente (`/remember`, `/forget`)
- Workflows ejecutables (`/workflow <name>`)
- Cron scheduling (`/cron-add`, `/cron-list`, `/cron-remove`)
- Cache offline de documentos (degraded mode cuando el servidor no responde)
- Filesystem tool sandbox con prevencion de path traversal
- Permission system: Standard/Permissive/Strict (cambio via `:permissions`)

La configuracion (theme, model, diagnostico, exploradores de skills/ADRs/
policies/agents/workflows) se accede por `:` palette.

### 7. AutoSkill: modal de instalacion de stack

Activable con `:autoskill` en command palette (feature `autoskill`). Reemplaza
los viejos slashes `/autoskill` y `/install-skills` con un modal unificado.

**Flujo**:
1. `open_autoskill_picker` abre el modal con spinner y lanza `spawn_autoskill_scan`.
2. El scan analiza el proyecto (`package.json`, `.csproj` hasta 2 niveles,
   config files, file extensions) y detecta 26+ tecnologias + combos.
3. `populate_autoskill_picker` cruza las recomendaciones con skills ya
   instaladas (`skills-lock.json`, `.agents/skills/`, `.claude/skills/`) y
   popula `items: Vec<AutoskillItem>` con flag `installed: bool`.
4. El modal renderiza una lista con checkboxes:
   - `[✓] <name>` gris — ya instalada, no-op.
   - `[ ] <name>` — nueva, toggle con `Space`.
   - `[x] <name>` cyan — marcada para instalar.
5. `Enter` → `install_selected_autoskills` → `spawn_install_skills(paths)` →
   cierra el modal; el resultado del install llega como `Action::SkillInstallDone`.
6. `Esc` cierra sin instalar.

Tambien persiste un atajo debug en el dashboard (`Shift+S`) que dispara el
scan sin abrir el modal — escribe el resumen como markdown al chat
(flujo legacy util para desarrolladores de la TUI).

**Archivos**:
- `src/state/autoskill_picker.rs` — `AutoskillItem`, `AutoskillPickerState`.
- `src/ui/widgets/autoskill_picker.rs` — render.
- `src/app/autoskill_handler.rs` — open/populate/toggle/install.
- `src/services/autoskill_map.rs` — detect + collect_external_skills (existente).

### 8. Inicializacion de proyecto

Detecta tipo de proyecto automaticamente, pregunta para que cliente (Claude Code, Copilot, ambos), crea `.mcp.json` y `CLAUDE.md`.

### 9. Eventos en tiempo real (SSE)

Conexion SSE persistente. Eventos: sync, session, reload, heartbeat. Reconexion automatica con backoff exponencial.

### 10. Overlays y paneles

Los atajos `Ctrl+X` para abrir overlays fueron podados; el acceso canonico
es `:` + id. Solo los overlays "dashboard-local" conservan letra suelta.

- **Tool Monitor** (dashboard `T`): Historial de tool calls con filtros.
- **Enforcement Dashboard** (dashboard `H`): Estado de compliance gates.
- **Agent Panel** (dashboard `K`): Sub-agentes activos.
- **Notification Center** (dashboard `N`): Historial de notificaciones.
- **Cost Panel**: Detalle de costos por sesion.
- **Sessions Panel**: Developers conectados.
- **Model Picker** (`:model`): Selector de modelos AI.
- **Theme Picker** (`:theme`): Selector de themes con live preview; `Esc` revierte.
- **Autoskill Picker** (`:autoskill`): Modal de install batch (feature `autoskill`).
- **Permission Modal**: Aprobacion/denegacion de tool calls con always allow/deny.
- **Elicitation Modal** (E18): Input interactivo solicitado por MCP tools.
- **Monitor Panel** (E26): Output en tiempo real de procesos background.
- **Onboarding Checklist** (E39): Checklist guiado para nuevos usuarios.
- **Tip Card**: Tips contextuales con cooldown por sesion.
- **Transcript Modal** (`:transcript`): Vista completa del transcript.
- **History Search** (`:history-search`): Busqueda en historial de inputs.
- **Doctor Panel** (`:doctor`): Diagnosticos del sistema (MCP Server, MCP Tools,
  Config, Features, LSP, IDE Bridge, Disk).
- **Todo Panel**: Lista de tareas de la sesion.

---

## Subsistemas avanzados

### 11. Hooks configurables (E16)

Ejecucion de comandos shell disparados por eventos del TUI. Hooks se definen en `hooks.json` con triggers por accion (PreToolUse, PostToolUse, PreCodeApply, OnFactorySwitch). Se ejecutan fire-and-forget en paralelo y reportan resultados como Actions.

**Archivos**: `services/hooks/` (mod.rs, runner.rs), `app/hooks_handler.rs`

### 12. Permission System (E17)

Pipeline de enforcement basado en politicas para ejecucion de tools y bash. Tres modos:

| Modo | Comportamiento |
|------|---------------|
| Standard | Pide aprobacion para tools peligrosos |
| Permissive | Aprueba todo automaticamente |
| Strict | Pide aprobacion para todo |

Reglas persistentes `always_allow` / `always_deny` por nombre de tool. UI via Permission Modal.

**Archivos**: `services/permissions/` (mod.rs, enforcer.rs), `crates/ingenieria-runtime/src/permissions/policy.rs`

### 13. Elicitation MCP (E18)

Protocolo request/response para input interactivo desde MCP servers. Tipos de input: text, select, confirm, multi-select. El servidor envia `ElicitationRequest`, la UI renderiza un modal, y el usuario responde o cancela via `ElicitationResponder`.

**Archivos**: `services/mcp/elicitation.rs`, `ui/widgets/elicitation_modal.rs`, `app/elicitation_handler.rs`

### 14. Compactor (E19)

Compresion inteligente de contexto del chat para gestionar token budgets. Preserva system messages y cola reciente, resume mensajes eliminados. Respeta boundaries de pares tool_use/tool_result. Estrategias: Balanced, Aggressive, Conservative.

**Archivos**: `services/compactor/` (mod.rs, boundary.rs, strategy.rs, summary.rs)

### 15. ConfigTool (E20)

Herramienta AI-accesible para modificar configuracion no-sensible (model, factory, permission_mode, theme). Blocklist de keywords sensibles (api_key, token, secret, url). Despacha cambios via `Action::ApplyConfigChange` con audit trail.

**Archivos**: `services/tools/config_tool.rs`, `app/config_tool_handler.rs`

### 16. Mock Provider (E21)

Provider de chat para testing offline que genera respuestas sinteticas, tool calls simulados y streaming predecible. Permite desarrollo y testing sin conexion a APIs externas.

**Archivos**: `services/chat/mock_provider.rs`, `services/chat/synthetic_results.rs`

### 17. SubAgents y Teams (E22a/b)

#### SubAgents (E22a)

Orquestacion de sub-agentes con roles especificos (coder, reviewer, planner). `AgentRegistry` es la fuente de verdad para estado de agentes (Pending, Running, Done, Failed, Cancelled). Mutado por el reducer, leido por UI panel.

#### Teams (E22b)

Framework de colaboracion multi-agente. Equipos ejecutan templates de roles (FullStack, Research, Planning) en paralelo hacia objetivos compartidos. Todos los miembros reciben el mismo objetivo con contexto especifico por rol.

**Archivos**: `services/agents/` (registry.rs, spawner.rs, role.rs, team.rs), `app/agents_handler.rs`, `app/team_handler.rs`, `ui/widgets/agent_panel.rs`

### 18. Cron Scheduler (E23)

Sistema de tareas recurrentes con expresiones cron de 6+1 campos (seconds a year). Almacena jobs en `crons.json`, evalua cada 30 segundos, previene fires duplicados con `last_fired_at` tracking.

**Archivos**: `services/cron/` (mod.rs, job.rs, registry.rs, scheduler.rs, store.rs), `workers/cron_worker.rs`, `app/cron_handler.rs`

### 19. Worktree Isolation (E24)

Creacion de git worktrees por sub-agente en `$XDG_DATA_HOME/ingenieria-tui/worktrees/<agent_id>/` con branches aisladas (prefix `ingenieria/`). Aislamiento opcional que degrada gracefully si CWD no es un repo git. Cleanup en shutdown.

**Archivos**: `services/worktree.rs`

### 20. LSP Integration (E25)

Cliente generico de Language Server que auto-detecta servidores apropiados, captura diagnosticos via `publishDiagnostics`, y los inyecta en el contexto AI para mejorar generacion de codigo. No-op si la deteccion falla.

**Archivos**: `services/lsp/` (mod.rs, client.rs, protocol.rs, server_detection.rs, diagnostics.rs, completion.rs), `app/lsp_handler.rs`

### 21. Process Monitors (E26)

Ejecutor de procesos shell en background (max 3 concurrentes) que hace streaming de stdout/stderr linea por linea sin bloquear el chat. Resultados publicados como mensajes del assistant con exit code y ultimas N lineas. Registry almacena hasta 15 monitores (activos + historicos).

**Archivos**: `services/monitor.rs`, `workers/process_monitor.rs`, `app/monitor_handler.rs`, `ui/widgets/monitor_panel.rs`

### 22. IDE Bridge (E27)

Servidor HTTP local (feature-gated `ide`, usa axum) que permite a IDEs (VS Code, JetBrains) enviar contexto, aprobar/denegar tools, y recibir estado del TUI. Comunicacion bidireccional entre editor y terminal.

**Archivos**: `services/bridge/` (mod.rs, protocol.rs, server.rs), `app/bridge_handler.rs`

### 23. Plugin System (E28)

Registry que gestiona trait objects `dyn Plugin` con lifecycle hooks. Maximo 16 plugins. Trait vive en `ingenieria-domain` para que plugins solo dependan de la capa de dominio. Despacha eventos (register, lifecycle) a instancias cargadas.

**Archivos**: `crates/ingenieria-domain/src/plugin.rs`, `services/plugins.rs`, `app/plugin_handler.rs`

### 24. Onboarding (E39)

Experiencia de primera ejecucion con:
- **Checklist**: 5 pasos dismissibles (Configure, Chat, Deploy, etc.)
- **Tips contextuales**: Con cooldown por sesion, seleccion segun pantalla activa
- **Platform detection**: Detecta terminal, tmux, SSH para adaptar hints

Estado persiste en `onboarding.json`. Defaults seguros si archivo falta o corrupto.

**Archivos**: `services/onboarding/` (mod.rs, checklist.rs, tips.rs, platform_hints.rs), `ui/widgets/onboarding_checklist.rs`, `ui/widgets/tip_card.rs`

### 25. Recovery Engine + File Watcher (E42)

#### Recovery Engine

Mapper estateless que interpreta `StructuredFailure` o `FailureScenario`, resuelve recetas de recuperacion, y genera toasts/actions. Cubre failures no-chat: MCP handshake, config validation, worker crash, disco lleno.

#### File Watcher

Worker que monitorea cambios en archivos criticos y emite actions:
- `ConfigChanged` — recarga configuracion
- `KeybindingsChanged` — recarga atajos
- `ClaudeMdChanged` — recarga instrucciones AI
- `EnvChanged` — recarga variables de entorno

**Archivos**: `services/recovery_engine.rs`, `crates/ingenieria-domain/src/recovery.rs`, `workers/file_watcher.rs`

---

## Objetivo estrategico

ingenierIA TUI es la puerta de entrada del desarrollador al ecosistema ingenierIA:

- **Visibilidad**: Ver skills, politicas, ADRs, workflows, agentes y eventos
- **Velocidad**: Encontrar cualquier cosa en menos de 3 segundos
- **Contexto**: El AI tiene todo el conocimiento del equipo integrado via MCP
- **Gobernanza**: Compliance validation automatica, planning mode con gates, permission system
- **Onboarding**: Configuracion completa en 2 minutos con checklist guiado
- **Extensibilidad**: Plugins, hooks, cron jobs, sub-agentes, IDE bridge
- **Resiliencia**: Cache fallback, recovery recipes, graceful degradation
