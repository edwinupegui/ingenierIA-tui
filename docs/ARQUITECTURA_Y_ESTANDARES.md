# Arquitectura y Patrones

> Complemento de CLAUDE.md. Aqui van workspace architecture, patrones de diseno, anti-patrones, SOLID y feature flags.
> Las reglas de codigo, naming, imports y comandos viven en CLAUDE.md.

---

## Arquitectura de Workspace

### Crates y dependencias

El proyecto es un monorepo con 5 workspace crates bajo `crates/` y un binary principal en `src/`.

```
ingenieria-domain  (cero deps internas — tipos puros)
  ^         ^         ^
  |         |         |
ingenieria-ui   ingenieria-runtime   ingenieria-api (standalone)
  ^              ^
  |              |
  ingenieria-tools (dep: domain + runtime)
        ^
        |
    src/ (binary — depende de todos los crates)
```

### Patron re-export

`src/` re-exporta tipos de workspace crates para mantener paths familiares:

```rust
// src/domain/mod.rs
pub use ingenieria_domain::*;

// src/services/chat/mod.rs
pub use ingenieria_api::{ChatEvent, ChatStream, ModelInfo, ToolDefinition};
```

**Regla**: tipos nuevos van en el workspace crate correspondiente, no en `src/`. `src/domain/` y `src/ui/` son shims de re-export.

### Limites de los crates

| Crate | Puede importar | NO puede importar |
|-------|---------------|-------------------|
| `ingenieria-domain` | Solo std + crates externos (serde) | Nada interno |
| `ingenieria-ui` | domain + ratatui | services, app, runtime |
| `ingenieria-api` | Solo crates externos | Nada interno |
| `ingenieria-runtime` | domain | ui, api, services, app |
| `ingenieria-tools` | domain + runtime | ui, api, services, app |

---

## Feature Flags

### Flags disponibles

```toml
[features]
default   = ["full"]
full      = ["copilot", "mcp", "autoskill", "ide"]
minimal   = []                          # Core TUI + Claude only
copilot   = []                          # GitHub Copilot OAuth + provider
mcp       = []                          # MCP tool discovery + execution
autoskill = []                          # Tech detection + skill installer
ide       = ["dep:axum"]                # IDE Bridge HTTP server (E27)
```

### Patron de uso

Usar `#[cfg(feature = "...")]` en compile-time para gating de modulos y campos de state:

```rust
// BIEN: gating en actions.rs
#[cfg(feature = "mcp")]
McpToolsDiscovered(Vec<McpToolInfo>),

// BIEN: gating en state
#[cfg(feature = "mcp")]
pub mcp_tools: Vec<McpToolInfo>,

// BIEN: gating de modulo completo
#[cfg(feature = "autoskill")]
pub mod autoskill_map;
```

**Regla**: si un campo de state esta gated con `#[cfg]`, el action que lo modifica y el handler que lo procesa tambien deben estarlo. Consistencia en los 3 puntos: **action + state + handler**.

**Ejemplo canonico — feature `autoskill`**: el modal `AutoskillPicker` vive
solo en builds con la feature. Los cuatro puntos estan sincronizados:
`AppMode::AutoskillPicker` (variant gated), `AppState::autoskill_picker`
(campo gated), handlers en `src/app/autoskill_handler.rs` (modulo gated)
y widget en `src/ui/widgets/autoskill_picker.rs` (modulo gated).

### Cuando usar `#[cfg]` vs `cfg!`

| Contexto | Usar | Razon |
|---------|------|-------|
| Modulos, structs, campos | `#[cfg(feature = "...")]` | Eliminacion en compile-time |
| Logica condicional en runtime | `cfg!(feature = "...")` | Feature siempre compilado pero desactivable |
| Match arms de Action | `#[cfg(feature = "...")]` | Variante no existe sin feature |

---

## Patrones de diseno en uso

### Command Pattern (via Actions)

Cada evento del sistema es un comando tipado:

```rust
// BIEN: Accion especifica y tipada
enum Action {
    HealthUpdated(HealthStatus),
    ChatStreamDelta(String),
    ChatToolCall { id: String, name: String, arguments: String },
}

// MAL: Acciones genericas con strings
enum Action {
    Generic(String, String),
    Update(Box<dyn Any>),
}
```

### State Machine (para flujos multi-paso)

```rust
// BIEN: Estados explicitos
enum WizardStep { ServerUrl, Developer, Provider, Role }
enum ChatStatus { Idle, Loading, Streaming, WaitingToolResult, Error(String) }

// MAL: Flags booleanos
struct ChatState {
    is_loading: bool,
    is_streaming: bool,
    is_waiting: bool,  // que pasa si dos son true?
}
```

### Strategy Pattern (providers de AI)

Trait `ChatProvider` con implementaciones intercambiables:

```rust
#[async_trait]
pub trait ChatProvider: Send + Sync {
    async fn stream_chat(
        &self, model: &str, messages: &[ChatMessage], tools: &[ToolDefinition],
    ) -> Result<ChatStream>;
    async fn list_models(&self) -> Result<Vec<ModelInfo>>;
}

// Implementaciones: ClaudeProvider, CopilotProvider, MockProvider
```

Despacho via `resolve_provider()` — el chat no conoce el provider concreto.

### Strategy Pattern (MCP Transports)

Trait `McpTransport` para abstraer el medio JSON-RPC:

```rust
#[async_trait]
pub trait McpTransport: Send + Sync {
    async fn send_request(&self, request: JsonRpcRequest) -> Result<JsonRpcResponse>;
    async fn send_notification(&self, method: &str, params: Value) -> Result<()>;
    fn kind(&self) -> TransportKind;
}

// Implementaciones: SseTransport, StdioTransport, WebSocketTransport
```

### Strategy Pattern (Tools)

Trait `Tool` para herramientas extensibles:

```rust
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn permission(&self) -> ToolPermission;
    fn definition(&self) -> ToolDefinition;
    async fn execute(&self, arguments: &str) -> String;
}

// ToolRegistry mantiene Vec<Box<dyn Tool>>
```

### Plugin System (E28)

Extensibilidad via trait `Plugin` (vive en `ingenieria-domain` para minimas deps):

```rust
pub trait Plugin: Send + Sync {
    fn name(&self) -> &str;
    fn version(&self) -> &str { "0.0.0" }
    fn on_init(&self) -> Vec<PluginEffect> { vec![] }
    fn on_pre_action(&self, action_tag: &str) -> PluginResponse { PluginResponse::Continue }
    fn on_post_action(&self, action_tag: &str) { let _ = action_tag; }
    fn on_shutdown(&self) {}
}
```

Lifecycle: `on_init` -> (`on_pre_action` / `on_post_action`)* -> `on_shutdown`.
`PluginResponse::Intercept` puede cancelar una accion antes de que el reducer la procese.

### Observer Pattern (Hooks E16 + File Watcher E42)

Event hooks configurables que reaccionan a Actions:

```rust
// hooks.json define triggers por action tag
{ "on": "ChatStreamDone", "run": "notify-send 'Chat completado'" }
```

File watcher emite acciones al detectar cambios: `ConfigChanged`, `KeybindingsChanged`, `ClaudeMdChanged`, `EnvChanged`.

### Sub-Agent Pattern (E22a/b)

Orquestacion multi-agente con roles y aislamiento:

- `AgentRegistry` — lifecycle de sub-agentes (spawn, track, terminate)
- `AgentRole` — capabilities y permisos por rol (coder, reviewer, planner)
- `TeamRegistry` — composicion de equipos multi-agente (E22b)
- `WorktreeManager` — aislamiento via git worktrees (E24)

### Surface Split: `/` slash vs `:` palette

Los comandos del usuario viven en dos arrays **mutuamente excluyentes**:

| Superficie | Array | Scope | Dispatcher |
|-----------|-------|-------|-----------|
| `/` slash | `state::chat_types::SLASH_COMMANDS` (42) | Chat only | `App::handle_slash_command` |
| `:` palette | `state::command_state::PALETTE_COMMANDS` (26) | Config/ops/nav | `App::execute_command` |

**Criterio**: si el comando opera sobre el hilo de conversacion actual o
inyecta contexto al AI, pertenece a `/`. Si es configuracion, diagnostico,
navegacion o un sub-sistema global (theme, model, doctor, sync, exploradores,
cron, plugins, autoskill modal), pertenece a `:`.

**Invariantes defendidos por tests** (`state/chat_types.rs`,
`state/command_state.rs`):
- `config_commands_removed_from_slash` — los ids ops/config NUNCA viven en `/`.
- `chat_only_commands_excluded_from_palette` — los ids chat-only NUNCA en `:`.
- `renamed_entries_only_exist_with_new_name` — `change-model` murio a favor de `model`.

**Migracion suave**: `is_migrated_to_palette` en `slash_commands.rs` lista
los slashes viejos; cuando el usuario los tipea, el dispatcher responde con
un hint `"<cmd> se movio a la paleta. Pulsa : y busca <name>."` en lugar
del generico "Comando desconocido".

Ver [`docs/COMANDOS.md`](./COMANDOS.md) para la taxonomia canonica completa.

### Resilience Pattern

Estrategias de recuperacion ante fallos:

- **Exponential backoff**: SSE reconnect (`workers/sse.rs`), health checks
- **Cache fallback**: documentos cacheados cuando MCP server offline (`services/doc_cache.rs`)
- **Best-effort init**: hooks, plugins, onboarding cargan con warnings, no bloquean startup
- **Recovery recipes**: patrones de recuperacion para failures conocidos (`services/recovery_engine.rs`, E42)
- **Model fallback**: si un modelo falla, sugiere alternativa (`ingenieria-api/model_fallback.rs`)
- **Stream stall detection**: detecta streams bloqueados y alerta (`services/chat/stream_monitor.rs`)

---

## Anti-patrones a evitar

### God Object

Un struct o archivo que hace demasiado. Solucion: extraer handlers por pantalla, el `App::handle()` delega a funciones especificas.

### Stringly-typed programming

Usar `String` donde deberia haber un tipo especifico. Solucion: enums para estados finitos, newtypes para IDs.

### Clone innecesario

Clonar para evitar el borrow checker. Si clonas para `tokio::spawn`, usa `Arc<T>`. Si clonas para evitar un lifetime issue, redisena la funcion.

### Panic en produccion

`.unwrap()` solo en: tests, valores post-validacion, y constantes conocidas en compilacion. Todo lo demas usa `?` o `unwrap_or`.

### Efectos secundarios en render

Las funciones en `ui/*.rs` reciben `&AppState` inmutable. Si necesitan disparar algo, debe ser via `Action`.

### Estado distribuido

Todo estado observable de la UI vive en `AppState`. Los workers pueden tener estado interno transitorio pero no estado de negocio.

### Spawned tasks sin AbortHandle

Tasks async sin forma de cancelarlas. Solucion: guardar `AbortHandle` en state para cancelacion limpia (patron ya implementado en chat streaming).

### Feature flag inconsistente

Usar `#[cfg(feature = "mcp")]` en el action pero no en el handler o vice versa. Solucion: gating consistente en action + state + handler.

---

## SOLID en Rust

### S - Single Responsibility

Cada modulo tiene una responsabilidad unica:
- `ingenieria_client.rs` = HTTP REST
- `claude_provider.rs` = streaming Claude
- `ui/chat.rs` = render del chat
- `services/agents/registry.rs` = lifecycle de sub-agentes

### O - Open/Closed

Traits para extensibilidad sin modificar existentes:
- `ChatProvider` — agregar provider sin tocar Claude/Copilot
- `Tool` — agregar tool sin modificar tools existentes
- `Plugin` — extender comportamiento sin cambiar el core
- `McpTransport` — nuevo transporte sin tocar SSE/Stdio

### L - Liskov Substitution

Cualquier impl de `ChatProvider` debe ser intercambiable — el chat funciona identico con Copilot, Claude o Mock. Lo mismo aplica para `McpTransport` y `Tool`.

### I - Interface Segregation

Traits pequenos y especificos:
- `ChatProvider` — solo stream_chat + list_models
- `Tool` — solo name + permission + definition + execute
- `Plugin` — metodos con defaults, plugins implementan solo lo que necesitan
- `AgentRole` define capabilities granulares por tipo de agente

### D - Dependency Inversion

Alto nivel depende de abstracciones:
- `App` usa `Box<dyn ChatProvider>`, no un tipo concreto
- MCP client usa `Box<dyn McpTransport>`, no SSE directamente
- `ToolRegistry` almacena `Vec<Box<dyn Tool>>`
- Plugin trait vive en `ingenieria-domain` (capa mas baja), no en app/

---

## Notas sobre el dominio

Los tipos de inicializacion de proyecto (`ProjectType`, `InitClient`, `InitFileResult`) viven en `ingenieria-domain`, no en services. Esto mantiene la regla de que `domain/` no depende de ningun modulo interno y que otros modulos pueden importar estos tipos sin depender de `services/`.

Los tipos de chat (`ChatRole`, `ChatMode`, `ToolCall`, `ToolCallStatus`) viven parcialmente en `ingenieria-domain` y parcialmente en `state/chat_types.rs` (pendiente migracion completa al crate).
