# ingenierIA TUI — Instrucciones para desarrollo con IA

## Que es este proyecto

Terminal UI en Rust para el ecosistema ingenierIA MCP Server. Panel de control visual para navegar skills, policies, ADRs, workflows y chatear con AI usando el contexto de ingenierIA. Monorepo con 5 workspace crates y binary principal.

## Stack

- **Lenguaje**: Rust (edition 2021)
- **TUI**: ratatui 0.30 + crossterm 0.29
- **Async**: tokio (full features)
- **HTTP**: reqwest 0.13 (rustls, streaming)
- **WebSocket**: tokio-tungstenite 0.24
- **JSON**: serde + serde_json
- **Errores**: thiserror 2 (libreria) + anyhow (aplicacion)
- **Search**: nucleo-matcher 0.3
- **CLI**: clap 4
- **Markdown**: pulldown-cmark 0.13
- **Syntax**: syntect 5
- **File watch**: notify 7

## Workspace Crates

El proyecto es un monorepo con 5 crates internos bajo `crates/`:

| Crate | Responsabilidad | Deps internas |
|-------|----------------|---------------|
| `ingenieria-domain` | Tipos puros del dominio (Document, Event, Health, Chat, etc.) | ninguna |
| `ingenieria-ui` | Theme system (4 variantes), design system, a11y, primitives | ingenieria-domain |
| `ingenieria-api` | API types, pricing, metrics, retry, model fallback | — |
| `ingenieria-runtime` | Session, audit, memory, permissions, config validation | ingenieria-domain |
| `ingenieria-tools` | Bash safety, hooks, MCP protocol types | ingenieria-domain, ingenieria-runtime |

`src/domain/` y `src/ui/` re-exportan desde workspace crates. Tipos nuevos van en el crate correspondiente, no en `src/`.

## Feature Flags

| Flag | Default | Que habilita |
|------|---------|-------------|
| `full` | si | copilot + mcp + autoskill + ide |
| `minimal` | — | Core TUI + Claude API only |
| `copilot` | via full | GitHub Copilot OAuth + provider |
| `mcp` | via full | MCP tool discovery + execution |
| `autoskill` | via full | Tech detection + skill installer |
| `ide` | via full | IDE Bridge HTTP server (axum) |

Build: `cargo build` (full), `cargo build --no-default-features --features minimal`

## Arquitectura

Patron **Action-Reducer** (Elm-like):

```
Workspace Crates (domain, ui, api, runtime, tools)
    |
Workers (tokio::spawn) ──> Action ──> MPSC Channel ──> App::handle() ──> AppState ──> render()
```

### Modulos principales (`src/`)

- `app/` (33 archivos) — Reducer handlers, keybindings, spawners, slash commands. Unico lugar que muta estado.
- `state/` (15 archivos) — AppState completo con sub-states (chat, dashboard, wizard, etc.). Solo tipos y defaults.
- `actions.rs` — Enum Action con 100+ variantes. Solo tipos.
- `config.rs` — Carga y resolucion de configuracion (env, .mcp.json, ~/.config).
- `domain/` — Re-export de ingenieria-domain. Sin logica.
- `services/` — Funciones async puras, organizadas por subsistema:
  - `agents/` — SubAgents y Teams (E22a/b): registry, spawner, roles, teams
  - `auth/` — Copilot OAuth device flow
  - `bridge/` — IDE Bridge HTTP server (E27, feature `ide`)
  - `chat/` — ChatProvider trait + impls (Claude, Copilot, Mock), stream parser, SSE
  - `compactor/` — Context compaction del chat (E19)
  - `cron/` — Cron scheduler: jobs, registry, store (E23)
  - `hooks/` — Event hooks configurables (E16)
  - `lsp/` — Language Server Protocol client (E25)
  - `mcp/` (16 archivos) — MCP client, transports (WebSocket/Stdio/SSE), lifecycle manager
  - `onboarding/` — Checklist guiado + tips (E39)
  - `permissions/` — Tool permission enforcer: Standard/Permissive/Strict (E17)
  - `structured_output/` — JSON extraction de respuestas AI (E19)
  - `tools/` — Tool registry, filesystem (read/write), config tool (E20)
  - Archivos sueltos: `autoskill_map.rs`, `cache.rs`, `compliance.rs`, `context.rs`, `copilot.rs`, `doctor.rs`, `draft_store.rs`, `ingenieria_client.rs`, `features.rs`, `history.rs`, `monitor.rs` (E26), `paste_handler.rs`, `plugins.rs` (E28), `prompt_suggestions.rs`, `recovery_engine.rs` (E42), `skill_installer.rs`, `sync.rs`, `uri.rs`, `worktree.rs` (E24)
- `ui/` (50 archivos) — Render puro. Screens + widgets/ + markdown/. Recibe `&AppState` inmutable.
- `workers/` (11 archivos) — Tasks async de larga duracion: keyboard, tick, health, sse, hook_events, tool_events, cron_worker, file_watcher, process_monitor, lifecycle
- `registries/` — Singletons globales (OnceLock)
- `utils/` — Helpers de texto (truncate, visible_width)

### Reglas de dependencia

```
ingenieria-domain (cero deps internas)
  ^
ingenieria-ui, ingenieria-runtime
  ^
ingenieria-tools
  ^
src/ (binary)
  +-- app/      -> state, actions, services, workers
  +-- state/    -> domain
  +-- ui/       -> state, domain, [tipos puros de services/*]
  +-- workers/  -> actions, services
  +-- services/ -> domain, config
```

**Regla critica**: `ui/` NUNCA importa **logica** de `services/`. Excepcion documentada: tipos de datos puros (structs/enums sin metodos) para render (ej: `AgentStatus`, `ChecklistStep`, `MonitorInfo`). `domain/` no depende de nada interno.

## Comandos

```bash
cargo build                                    # Compilar (features=full)
cargo build --no-default-features --features minimal  # Build minimo
cargo run                                      # Ejecutar
cargo fmt                                      # Formatear (OBLIGATORIO antes de commit)
cargo clippy -- -D warnings                    # Lint (OBLIGATORIO, cero warnings)
RUST_LOG=debug cargo run                       # Con logs de debug
```

## Reglas de codigo

### Obligatorias

- **Max 400 lineas por archivo**. Si crece mas, dividir por responsabilidad. Ver excepciones conocidas en `docs/ESTRUCTURA_PROYECTO.md`.
- **Max 40 lineas por funcion**. Extraer subfunciones si crece.
- **Max 4 parametros por funcion**. Agrupar en struct si necesita mas.
- **No `.unwrap()` en codigo de produccion** excepto post-validacion o constantes. Usar `?` o `unwrap_or`.
- **No clonar para evitar el borrow checker**. Redisenar ownership o usar Arc.
- **No efectos secundarios en funciones de render** (`ui/*.rs`).
- **No allocar en funciones de render** salvo que sea inevitable. Preferir `&str`, `Cow<str>`.
- **No estado fuera de AppState**. Todo estado observable vive ahi.
- **Cachear resultados costosos** (markdown parsed, search results). Invalidar cuando datos fuente cambian.
- **`cargo fmt` + `cargo clippy` antes de cada commit**. Sin excepciones.
- **Tipos nuevos van en workspace crates**, no en `src/`. Domain types en `ingenieria-domain`, UI en `ingenieria-ui`.
- **Feature flags con `#[cfg(feature = "...")]`** para todo codigo condicional. Verificar consistencia en actions, state y services.

### Nomenclatura (Rust standard)

| Elemento | Convencion | Ejemplo |
|---------|-----------|---------|
| Variables, funciones | snake_case | `server_url`, `handle_key_event` |
| Structs, Enums, Traits | PascalCase | `AppState`, `ChatRole` |
| Constantes | SCREAMING_SNAKE_CASE | `MAX_RETRIES` |
| Modulos | snake_case | `ingenieria_client` |

### Nombres semanticos

- Booleans: `is_`, `has_`, `should_`, `can_` (ej: `is_server_online`)
- Handlers: `handle_<screen>_<accion>` (ej: `handle_chat_key_event`)
- Renders: `render_<componente>` (ej: `render_sidebar`)
- Workers: `spawn_<nombre>_worker` (ej: `spawn_sse_worker`)

### Imports (orden)

```rust
// 1. Standard library
use std::collections::HashMap;

// 2. Crates externos
use anyhow::Result;
use ratatui::prelude::*;

// 3. Crate propio
use crate::actions::Action;
use crate::state::AppState;
```

## Colores

Usar siempre los tokens del theme system en `crates/ingenieria-ui/src/theme/`, NUNCA literales `Color::Rgb(...)`.

4 variantes de theme: Dark (default), Light, HighContrast, Solarized. Acceso via `state.active_theme.colors()`.

Paleta base: fondo oscuro `#121624`, texto `#DCE1F0`, bordes `#323C5A`.
Factory accents: Net=purple `#68217A`, Ang=red `#C82333`, All=green `#48BB78`.

Ver `docs/LINEAMIENTOS_VISUALES.md` para la referencia completa.

## Documentacion del proyecto

- `docs/FLUJOS_Y_OBJETIVOS.md` — Que hace el proyecto, flujos principales, todos los subsistemas (E16-E42)
- `docs/ARQUITECTURA_Y_ESTANDARES.md` — Workspace architecture, patrones de diseno, anti-patrones, SOLID, feature flags
- `docs/ESTRUCTURA_PROYECTO.md` — Arbol del proyecto, limites de archivo, excepciones conocidas, dependencias
- `docs/LINEAMIENTOS_VISUALES.md` — Theme system, colores, layout, factory themes, design system, a11y
- `docs/ERROR_HANDLING.md` — Tipos de error, propagacion, retry/fallback, presentacion al usuario
- `docs/OWNERSHIP_Y_PERFORMANCE.md` — Ownership, async lifecycle (11 workers), optimizaciones de rendimiento
- `docs/AUDITORIA.md` — Checklist estricto y repetible para auditar calidad del codigo

## Servidor ingenierIA (backend)

- Base URL: `http://localhost:3001` (configurable)
- REST: `/api/health`, `/api/documents`, `/api/search`, `/api/events` (SSE)
- MCP: `/claude/sse` (8 tools: get_factory_context, get_workflow, validate_compliance, sync_project, search_documents, get_document, list_factories, get_agents)

## Proveedores de AI

- **GitHub Copilot**: OAuth device flow, modelos via API de Copilot
- **Claude API**: API key de Anthropic, Messages API con streaming
- **Mock Provider**: Para testing offline (E21)

La seleccion de provider se hace en el wizard. El chat usa el provider configurado automaticamente.

## Al trabajar en este proyecto

1. **Usa `/rust-best-practices` SIEMPRE**. Invocarlo antes de implementar structs, traits, error handling, async, o cualquier logica nueva. No es opcional.
2. **Lee los archivos antes de modificarlos**. Entiende el contexto.
3. **Sigue la arquitectura Action-Reducer**. Si necesitas hacer algo async, hazlo via tokio::spawn y envia el resultado como Action.
4. **No agregues features no solicitados**. No "mejores" codigo que no te pidieron cambiar.
5. **Verifica con `cargo build` y `cargo clippy`** despues de cada cambio significativo.
6. **Respeta los limites de archivo**. Si un archivo crece, dividelo segun `docs/ESTRUCTURA_PROYECTO.md`.
7. **Tipos nuevos en workspace crates**. Domain en `ingenieria-domain`, UI en `ingenieria-ui`, runtime en `ingenieria-runtime`.
8. **Feature flags consistentes**. Codigo condicional usa `#[cfg(feature = "...")]` en actions, state y services.

<!-- autoskills:start -->

Summary generated by `autoskills`. Check the full files inside `.claude/skills`.

## Rust Best Practices

>

- `.claude/skills/rust-best-practices/SKILL.md`
- `.claude/skills/rust-best-practices/references/chapter_01.md`: Rust's ownership system encourages **borrow** (`&T`) instead of **cloning** (`T.clone()`).
- `.claude/skills/rust-best-practices/references/chapter_02.md`: Be sure to have `cargo clippy` installed with your rust compiler, run `cargo clippy -V` in your terminal for a rust project and you should get something like this `clippy 0.1.86 (05f9846f89 2025-03-31)`. If terminal fails to show a clippy version, please run the following code `rustup update && r...
- `.claude/skills/rust-best-practices/references/chapter_03.md`: The **golden rule** of performance work:
- `.claude/skills/rust-best-practices/references/chapter_04.md`: Rust enforces a strict error handling approach, but *how* you handle them defines where your code feels ergonomic, consistent and safe - as opposing cryptic and painful. This chapter dives into best practices for modeling and managing fallible operations across libraries and binaries.
- `.claude/skills/rust-best-practices/references/chapter_05.md`: In Rust, as in many other languages, tests often show how the functions are meant to be used. If a test is clear and targeted, it's often more helpful than reading the function body, when combined with other tests, they serve as living documentation.
- `.claude/skills/rust-best-practices/references/chapter_06.md`: Rust allows you to handle polymorphic code in two ways: * **Generics / Static Dispatch**: compile-time, monomorphized per use. * **Trait Objects / Dynamic Dispatch**: runtime vtable, single implementation.
- `.claude/skills/rust-best-practices/references/chapter_07.md`: Models state at compile time, preventing bugs by making illegal states unrepresentable. It takes advantage of the Rust generics and type system to create sub-types that can only be reached if a certain condition is achieved, making some operations illegal at compile time.
- `.claude/skills/rust-best-practices/references/chapter_08.md`: Use `//` comments (double slashed) when something can't be expressed clearly in code, like: * **Safety Guarantees**, some of which can be better expressed with code conditionals. * Workarounds or **Optimizations**. * Legacy or **platform-specific** behaviors. Some of them can be expressed with `#...
- `.claude/skills/rust-best-practices/references/chapter_09.md`: Many higher level languages hide memory management, typically **passing by value** (copy data) or **passing by reference** (reference to shared data) without worrying about allocation, heap, stack, ownership and lifetimes, it is all delegated to the garbage collector or VM. Here is a comparison o...

<!-- autoskills:end -->
