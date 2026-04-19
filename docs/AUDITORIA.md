# Auditoria de Codigo — ingenierIA TUI

> Checklist estricto y repetible para auditar calidad, arquitectura, clean code,
> SOLID, performance y seguridad. Organizado por crate (boundaries) + transversales.
> Ejecutar antes de cada release o cuando se cambien boundaries.
>
> Usar con: "Ejecuta la auditoria de docs/AUDITORIA.md sobre el proyecto actual.
> Reporta hallazgos con file:line y severidad."

---

## §0. Como ejecutar esta auditoria

1. Recorrer cada seccion en orden. Las secciones `§2.x` son paralelizables (un agente por crate).
2. Cada hallazgo **obligatorio** incluye `file:line` y evidencia concreta (snippet, no parafrasis).
3. Clasificar: `CRITICO` (rompe invariante), `ALTO` (riesgo real), `MEDIO` (deuda), `BAJO` (estilo).
4. No reportar falsos positivos. Si una regla tiene excepcion documentada, verificar que se respeta antes de abrirla como hallazgo.
5. Entregar el reporte con el formato de `§8`.

**Comandos de apoyo**:

```bash
cargo fmt --check
cargo clippy --all-features -- -D warnings
cargo build --features full
cargo build --no-default-features --features minimal
cargo test --features full --bins
cargo tree -d                # Deps duplicadas
wc -l src/**/*.rs crates/**/*.rs | sort -rn | head -20
```

---

## §1. Reglas globales (aplican a todo el workspace)

### 1.1 Naming (Rust standard)

- [ ] Variables / funciones en `snake_case`.
- [ ] Structs / Enums / Traits en `PascalCase`.
- [ ] Constantes en `SCREAMING_SNAKE_CASE`.
- [ ] Modulos en `snake_case`.
- [ ] Booleans con prefijo semantico: `is_`, `has_`, `should_`, `can_`, `was_`, `will_`.
- [ ] Handlers con patron `handle_<contexto>_<accion>`. Renders con `render_<componente>`. Spawners con `spawn_<operacion>`.

### 1.2 Limites de tamano

Los limites viven en `docs/ESTRUCTURA_PROYECTO.md` §Tamano maximo. Resumen:

| Tipo | Limite | Tipo | Limite |
|------|--------|------|--------|
| Handler `app/*.rs` | 400 | State `state/*.rs` | 200 |
| Screen `ui/*.rs` | 400 | Worker `workers/*.rs` | 100 |
| Service `services/*.rs` | 300 | Domain `crates/ingenieria-domain/` | 100 |
| Widget `widgets/*.rs` | 200 | `main.rs` | 80 |
| Funcion individual | 40 lineas | Parametros por funcion | 4 max |

- [ ] Solo reportar archivos que exceden **y** no estan en la tabla "Excepciones conocidas" de `ESTRUCTURA_PROYECTO.md`.
- [ ] Archivo nuevo que nace sobre el limite es `CRITICO`.

### 1.3 Magic values

- [ ] No numeros magicos (excluir `0`, `1`, `-1`). Duraciones, timeouts, limites, precios y dimensiones UI repetidas deben ser constantes con nombre.
- [ ] No strings literales usadas como claves / ids / keys de config. Usar `const` o enums. Excluir: mensajes al usuario, errores, format strings.
- [ ] `Color::Rgb` solo en `crates/ingenieria-ui/src/theme/*.rs`. Excepciones: `src/ui/diff_word.rs` y `src/ui/diff_syntax.rs` (colores estructurales de diff).

### 1.4 Comentarios

- [ ] Cero comentarios del "que" (`// increment counter`). El codigo ya lo dice.
- [ ] Cero `TODO`, `FIXME`, `HACK`, `XXX` sin issue asociado. Buscar `rg "TODO|FIXME|HACK|XXX"`.
- [ ] Si hay comentario, debe explicar "por que" o "cuidado con...": workaround conocido, invariante no-obvio, heuristica, performance.

### 1.5 Imports

Orden obligatorio (separados por linea en blanco):

```rust
use std::collections::HashMap;           // 1. std

use anyhow::Result;                      // 2. externos
use ratatui::prelude::*;

use crate::actions::Action;              // 3. propios
use crate::state::AppState;
```

- [ ] `cargo fmt --check` pasa sin cambios.
- [ ] `cargo clippy --all-features -- -D warnings` cero warnings. Cada `#[allow(...)]` con comentario justificando el por que.
- [ ] Cero imports muertos (`cargo clippy` los reporta).

### 1.6 Duplicacion

- [ ] Tres o mas lineas identicas / casi identicas en dos sitios: extraer.
- [ ] Constante con el mismo valor en dos archivos: consolidar en `ingenieria-domain` o en `crate::config`.

---

## §2. Per-crate boundaries

El proyecto son **5 workspace crates** bajo `crates/` + un **binary** en `src/`.
Cada crate tiene reglas propias de dependencias y responsabilidad. Auditar de forma
independiente. El grafo canonico:

```
ingenieria-domain (cero deps internas)
       ^
ingenieria-ui   ingenieria-runtime   ingenieria-api (standalone)
                     ^
              ingenieria-tools
                     ^
                   src/  (binary — consume todos)
```

### §2.1 `ingenieria-domain` — tipos puros

**Responsabilidad**: structs y enums del dominio, serializables, sin logica de IO.

- [ ] **Cero deps internas**. `crates/ingenieria-domain/Cargo.toml` no lista ningun otro crate del workspace.
- [ ] **Cero IO y cero async**. Buscar `tokio`, `reqwest`, `std::fs`, `std::net`, `async fn` en `crates/ingenieria-domain/src/`. Nada de eso puede vivir aqui.
- [ ] **Sin mutacion global**. Cero `static mut`, `lazy_static`, `OnceLock` con estado mutable. Constantes inmutables solamente.
- [ ] **Deps externas minimas**. Solo `serde`, `serde_json`, `thiserror` aceptados por defecto. Cualquier otra requiere justificacion.
- [ ] **Tipos derivables**. Los structs publicos tienen `Debug`, `Clone`, `Serialize`, `Deserialize` donde aplique.
- [ ] **`src/domain/mod.rs` solo re-exporta** (`pub use ingenieria_domain::*;`). No define tipos propios.

### §2.2 `ingenieria-ui` — theme + render primitives

**Responsabilidad**: 6 themes (TokyoNight default, Solarized, Gruvbox, Monokai, Matrix, HighContrast), design tokens, widgets primitivos, a11y.

- [ ] **Deps internas = solo `ingenieria-domain`**. `crates/ingenieria-ui/Cargo.toml` no lista runtime, api, tools.
- [ ] **Cero `tokio::spawn`, `reqwest`, `std::fs`, `std::env`**. Grep `crates/ingenieria-ui/src/`. El render es puro.
- [ ] **Los 6 themes viven en `theme/`** (`tokyonight.rs`, `solarized.rs`, `gruvbox.rs`, `monokai.rs`, `matrix.rs`, `high_contrast.rs`). Agregar uno: no modificar los existentes.
- [ ] **`Color::Rgb` solo dentro de `theme/`**. Ningun otro archivo del crate usa literales de color.
- [ ] **Cero allocations evitables** en funciones de render (ver §3.1).
- [ ] **Theme se accede via token** (`colors.primary`, `colors.border`), nunca por literal.

### §2.3 `ingenieria-api` — tipos de API

**Responsabilidad**: chat API types, pricing, retry, metrics, stream parsing.

- [ ] **Standalone**. `crates/ingenieria-api/Cargo.toml` no lista ningun crate interno. Actual: solo `serde_json` + `futures-util`.
- [ ] **Tipos puros**, sin logica de IO. Parsers y retry policies son funciones puras sobre structs.
- [ ] **Pricing actualizado**. La tabla de precios (`ingenieria-api/src/pricing.rs` o equivalente) coincide con los precios oficiales de Anthropic + GitHub. Actualizar cuando cambien.
- [ ] **Model fallback determinista**. `model_fallback.rs` mapea `modelo_no_disponible → alternativa` sin efectos secundarios.

### §2.4 `ingenieria-runtime` — sesion, audit, memoria, permisos

**Responsabilidad**: session store, audit log, memory CLAUDE.md, permissions enforcer, config validation.

- [ ] **Deps internas = solo `ingenieria-domain`**. Cero refs a `ui`, `api`, `tools`.
- [ ] **No UI, no TUI**. Cero `ratatui`, `crossterm`. Todo lo que renderiza vive en `ingenieria-ui` / `src/ui/`.
- [ ] **IO aislado** en modulos especificos (session persistence). Los tipos expuestos al resto no requieren filesystem.
- [ ] **Tests independientes**: `cargo test -p ingenieria-runtime` pasa sin arrancar el binary.

### §2.5 `ingenieria-tools` — bash safety, hooks, MCP protocol

**Responsabilidad**: BashValidator, HookConfig, tipos de MCP protocol (JsonRpcRequest, etc).

- [ ] **Deps internas = solo `ingenieria-domain` + `ingenieria-runtime`**. Verificar Cargo.toml.
- [ ] **Cero UI y cero HTTP**. La ejecucion real de requests vive en `src/services/`. Aqui solo tipos + validacion de seguridad.
- [ ] **BashValidator cubre patrones peligrosos**: `rm -rf /`, inyeccion shell, `eval`, escape de `"`. Mantener lista de patrones vigente.
- [ ] **HookConfig valida esquema** antes de ejecutar. Usuario no puede inyectar comandos via `on: "...";exec bash"`.

### §2.6 `src/` (binary) — Action-Reducer

**Responsabilidad**: dispatcher Elm-like. Unico lugar que muta `AppState`.

- [ ] **Flujo respeta Action-Reducer**: workers → `Action` → MPSC → `App::handle()` → `AppState` → `render()`.
- [ ] **Solo `app/` muta `AppState`**. Grep `&mut AppState` en `state/`, `services/`, `ui/`, `workers/`, `domain/` → cero matches.
- [ ] **Workers solo envian Actions**. `src/workers/*.rs` no mutan estado ni llaman a `App::handle`.
- [ ] **`ui/` nunca importa logica de `services/`**. Excepciones documentadas en `ESTRUCTURA_PROYECTO.md` §Dependencias (5 widgets: `onboarding_checklist`, `monitor_panel`, `agent_panel`, `tip_card`, `elicitation_modal`). Nuevo import de services en ui: solo tipos puros.
- [ ] **Render es puro**: `ui/**/*.rs` no tienen `tokio::spawn`, `send()`, `write()`, `fs::`, `reqwest`, `println!`, `eprintln!`.
- [ ] **`render()` recibe `&AppState`** inmutable. Ningun `&mut AppState` en firmas de `ui/`.
- [ ] **Split `/` vs `:` estricto**. `SLASH_COMMANDS` (42 entries en `src/state/chat_types.rs`) y `PALETTE_COMMANDS` (27 entries en `src/state/command_state.rs`) no comparten ids. Tests `config_commands_removed_from_slash` y `chat_only_commands_excluded_from_palette` defienden esto.
- [ ] **Dispatchers completos**: `handle_slash_command` enruta los 42 slashes. `execute_command` enruta los 27 palette ids. Cero brazos muertos.
- [ ] **Slashes migrados devuelven hint**. `is_migrated_to_palette` en `src/app/slash_commands.rs` lista los 17 slashes eliminados (`/compliance`, `/autoskill`, `/install-skills`, `/go`, `/fork-from`, `/load`, `/memories`, `/brief`, `/sessions`, `/retry`, `/todo-check`, `/lsp-status`, `/lsp-diag`, `/bridge-status`, `/features`, `/hooks`, `/detect`).
- [ ] **Workers spawned = 10** en `src/workers/mod.rs`: keyboard, tick, health, sse, hook_events, tool_events, cron_worker, file_watcher, process_monitor, lifecycle.
- [ ] **Feature flags consistentes** (ver §6). Ejemplo canonico: `autoskill` afecta `AppMode::AutoskillPicker`, `AppState::autoskill_picker`, `app/autoskill_handler.rs` y `ui/widgets/autoskill_picker.rs` — los 4 gated juntos.

---

## §3. Performance

### 3.1 Render path (hot — 4Hz)

El ciclo corre cada 250ms. Optimizar aqui = fluidez percibida. Baseline actual en
`src/ui/` y `crates/ingenieria-ui/src/`: aprox 224 `format!`, 99 `.to_string()`,
54 `.clone()`. Meta: reducir esos numeros, no aumentarlos.

- [ ] **Cero allocations nuevas en loops de render**. Red flags:
  - `format!()` dentro de `for` que corre cada frame → mover al reducer.
  - `.to_string()` en render → guardar el `String` en state cuando el dato cambia.
  - `.clone()` de colecciones → pasar `&[T]` o `Arc<Vec<T>>`.
  - `Vec::new()` sin `with_capacity(n)` cuando `n` es conocido.
- [ ] **Preferir `&str` y `Cow<str>`** en firmas de render. `String` solo cuando necesitas ownership.
- [ ] **Markdown cacheado**. `render_markdown` cachea por `(doc_id, hash_of_content)`. Invalidar solo cuando el doc cambia.
- [ ] **Sidebar pre-computado**. Lista del sidebar se reconstruye cuando `documents` cambia, no cada frame.
- [ ] **Sin parsing en render**. JSON / YAML / TOML nunca se parsea dentro de `ui/`.

### 3.2 Caches e invalidacion

- [ ] **CacheLayer con invalidacion explicita**. Caches de docs, docs_details, search_results deben invalidarse cuando la fuente cambia (file watcher, SSE event). No TTL ciego.
- [ ] **Streaming markdown incremental** (E29). El delta se re-parsea sobre el cache existente, no el documento entero.
- [ ] **Arc para datos compartidos grandes**. Documentos, skills cargadas y policies deben ser `Arc<...>` y clonarse como `Arc::clone`.

### 3.3 Clone audit

- [ ] En cada `tokio::spawn`, lista de clones debe ser minima:
  - `tx.clone()` / `Arc<...>::clone()` — ok.
  - `state.campo_grande.clone()` — convertir a `Arc` o pasar solo lo necesario.
  - `AppState.clone()` — `CRITICO`.
- [ ] Clones dentro de `app/*.rs` (reducer) deben tener justificacion. Cada coleccion clonada es un hallazgo hasta que se prueba necesaria.
- [ ] Funciones que solo leen strings reciben `&str`, no `String` / `&String`.

### 3.4 Async y concurrencia

- [ ] **Cero blocking en async**. `std::fs::`, `std::thread::sleep`, `std::process::Command::output` dentro de `async fn` son `CRITICO`. Usar `tokio::fs`, `tokio::time::sleep`, `tokio::process`.
- [ ] **Timeouts en HTTP**. Cada `reqwest::get/post` tiene `.timeout(...)` configurado. Sin excepcion.
- [ ] **AbortHandle para tasks largas**. Streams de chat, LSP requests, MCP requests guardan su handle para cancelacion limpia.
- [ ] **No sleep como polling**. Usar channels / signals. Excepcion: workers de health/tick por diseno.
- [ ] **Backpressure MPSC**. Canales bounded con capacidad conocida (actual: 100). Si se llena repetidamente, el remitente debe loggear, no colgarse.
- [ ] **Stream stall detection** activo. `services/chat/stream_monitor.rs` emite `StreamWarning` / `StreamTimeout`.

### 3.5 Memoria

- [ ] **Sin acumulacion sin limite**. `Vec` / `VecDeque` que crecen con eventos (history, logs, tool_calls) tienen `max_len` y rotacion.
- [ ] **Strings grandes justificados**. Si un campo `String` de `AppState` almacena contenido completo de docs, verificar que se necesita o cachear en disco.
- [ ] **Drop order explicito** para `Arc<T>` en workers: al terminar el worker, sus `Arc` referencias se sueltan antes del shutdown.

### 3.6 Network

- [ ] **Un solo `reqwest::Client`** (connection pool) compartido. No se crea por request.
- [ ] **SSE reconnect con backoff exponencial + jitter**. Revisar `src/workers/sse.rs`.
- [ ] **Cero requests duplicados**. Misma URL en la misma ventana: dedup o cache.

### 3.7 Binary size y deps

- [ ] **Sin deps duplicadas en versiones distintas**. `cargo tree -d` no debe reportar conflictos criticos (ej. dos `tokio`, dos `ratatui`). Excepcion permitida hoy: `bitflags` 1.x / 2.x via transitive.
- [ ] **Deps pesadas justificadas**. `tokio` full, `reqwest`, `ratatui`, `syntect`, `pulldown-cmark`, `tokio-tungstenite`, `axum` (solo con feature `ide`), `notify`. Cualquier adicion nueva requiere review.
- [ ] **Check de build minimal**. `cargo build --no-default-features --features minimal` compila y arranca. Preserva la promesa de build ligero.

---

## §4. Error handling

Patron: `thiserror` en libraries (workspace crates), `anyhow` en aplicacion (`src/`).
Detalles en `docs/ERROR_HANDLING.md`.

### 4.1 Tipos

- [ ] **Cero `Result<T, String>`**. Errores siempre tipados via `thiserror`.
- [ ] **Variants de error con prefijo del dominio** y mensaje `#[error("...")]` accionable.
- [ ] **`anyhow::Error` solo en `src/`**. Los workspace crates exportan errores tipados.
- [ ] **`.context("...")` en `?` boundaries** para enriquecer la cadena (`anyhow`).

### 4.2 Panics y unwraps

- [ ] **Cero `.unwrap()` en codigo de produccion**. Excepciones: tests, valores post-validacion inmediatos, constantes conocidas en compilacion.
- [ ] **`.expect("...")` solo con mensaje que explica el invariante** ("config validated above, None imposible aqui"). Sin justificacion = hallazgo.
- [ ] **Cero panics en `tokio::spawn`**. Cualquier panic en tarea async tumba el worker silenciosamente. Cubrir con `Result` o `catch_unwind` en entry point del worker.

### 4.3 Propagacion y silencios

- [ ] **`?` en lugar de match manual** cuando la firma retorna `Result`.
- [ ] **Cero silencios**. `let _ = op_that_may_fail();` requiere `tracing::debug!` al lado o comentario con razon.
- [ ] **`Result<()>` no se ignora**. Si una funcion retorna `Result<()>`, el caller lo maneja o propaga.

### 4.4 Recovery y resiliencia

- [ ] **Recovery recipes mapeadas**. `services/recovery_engine.rs` mapea `StructuredFailure` conocidos a recetas de recovery. Agregar receta nueva = no tocar existentes.
- [ ] **Doctor cubre 7 subsistemas**: MCP Server, Config, MCP Tools, Features, LSP, IDE Bridge, Disco. Cada check devuelve `Ok | Warn | Fail` con descripcion.
- [ ] **Model fallback registrado** en `ingenieria-api/model_fallback.rs`. Modelo off → sugerencia + log.

---

## §5. Seguridad

### 5.1 Tokens y secretos

- [ ] **Nunca loggear tokens**. Grep `tracing::`, `log::`, `println!`, `eprintln!`, `dbg!` con variables de `token`, `key`, `password`, `secret`. Cero matches.
- [ ] **Tokens NO viajan como Actions**. Se guardan en `AppState` protegido y se leen directo. Buscar `Action::*Token*`, `Action::*Auth*` con payload de token.
- [ ] **Archivos sensibles con permisos `0o600`**. `copilot_auth.json`, Claude API key, cualquier archivo con secret. Verificar `set_permissions` al escribir.
- [ ] **Redaccion en transcript**. Transcripts y logs exportados no incluyen Authorization headers ni tokens.

### 5.2 Input validation

- [ ] **URLs validadas** (`http` / `https`) antes de request. El wizard y los tool calls rechazan `file://`, `javascript:`, etc.
- [ ] **Input del usuario sanitizado** antes de shell. `BashValidator` en `ingenieria-tools` cubre patrones peligrosos — verificar que todo exec pasa por el.
- [ ] **Path traversal prevenido**. Tools `read_file`, `write_file`, `list_directory` rechazan `..` fuera del workspace root.
- [ ] **Permissions enforcer activo**. `services/permissions/` valida cada tool call. Modos Standard / Permissive / Strict con comportamiento probado.

### 5.3 Superficie de red

- [ ] **IDE Bridge (`feature = "ide"`) bindea a `127.0.0.1`** por defecto. Configuracion explicita para exponer.
- [ ] **Requests outbound acotados**. Solo a servers configurados (MCP server, provider APIs). Sin wildcard egress.

---

## §6. Feature flags consistency

Flags: `default = ["full"]`, `full = ["copilot", "mcp", "autoskill", "ide"]`, `minimal = []`.

- [ ] **`#[cfg(feature = "...")]` consistente en los 4 puntos**: action + state + handler + widget. Si `AppMode::AutoskillPicker` esta gated, el campo `AppState::autoskill_picker`, los handlers en `app/autoskill_handler.rs` y el widget en `ui/widgets/autoskill_picker.rs` tambien.
- [ ] **Cero codigo MCP sin gate**. Imports de `crate::services::mcp` fuera de bloques `#[cfg(feature = "mcp")]` = `CRITICO`.
- [ ] **Cero codigo Copilot sin gate**. Imports de `services::auth` copilot / `CopilotProvider` fuera de gates = `CRITICO`.
- [ ] **`axum` solo en `services/bridge/`** y con `#[cfg(feature = "ide")]`.
- [ ] **Build minimal verde**. `cargo build --no-default-features --features minimal` + `cargo test --no-default-features --features minimal` pasan.

---

## §7. Sync docs-site ↔ TUI

El sitio (`docs-site/`) es la cara publica. Un feature documentado que no existe es
peor que uno sin documentar.

### 7.1 Version y requisitos

- [ ] Version en `docs-site/src/content/docs/index.mdx` coincide con `Cargo.toml` (`version = "0.21.0"` o la actual).
- [ ] `rust-version` de `Cargo.toml` coincide con el requisito en `guia/instalacion.mdx`.

### 7.2 Features documentados vs implementados

- [ ] Cada card del landing tiene implementacion real en `src/`. No cards fantasma.
- [ ] Cada screen descrito en `funcionalidades/` tiene archivo en `src/ui/screens/` + handler en `src/app/`.
- [ ] Features implementados sin documentar se reportan como `MEDIO` (decidir: documentar o marcar interno).

### 7.3 Comandos y tools

| Array | Ubicacion | Count esperado |
|-------|-----------|----------------|
| `SLASH_COMMANDS` | `src/state/chat_types.rs` | 42 |
| `PALETTE_COMMANDS` | `src/state/command_state.rs` | 27 |

- [ ] Split estricto `/` vs `:` — ningun id en ambos arrays (tests `config_commands_removed_from_slash`, `chat_only_commands_excluded_from_palette`).
- [ ] `is_migrated_to_palette` en `src/app/slash_commands.rs` incluye los 17 slashes podados.
- [ ] Tools MCP documentadas en `docs/FLUJOS_Y_OBJETIVOS.md` (8 tools: `get_factory_context`, `get_workflow`, `validate_compliance`, `sync_project`, `search_documents`, `get_document`, `list_factories`, `get_agents`) coinciden con las registradas en `src/services/tools/` y las invocadas via MCP client.
- [ ] Slashes eliminados no reaparecen: `/compliance`, `/autoskill`, `/install-skills`, `/go`, `/fork-from`, `/load`, `/memories`, `/brief`, `/sessions`, `/retry`, `/todo-check`, `/lsp-status`, `/lsp-diag`, `/bridge-status`, `/features`, `/hooks`, `/detect`.

### 7.4 Keybindings

Canonico: `docs/KEYBINDINGS.md`. Mapa activo: `src/workers/keyboard.rs` con 4 modifier
bindings (`Ctrl+C`, `Ctrl+Z`, `Ctrl+Y`, `Ctrl+↑/↓`) + `Alt/Shift+↑/↓` para scroll.

- [ ] Atajo documentado → implementado en `src/app/keys.rs` o sub-handler (`keys_chat.rs`, `keys_splash.rs`, `keys_wizard.rs`).
- [ ] Atajos huerfanos (implementados, no documentados) → reportar como `MEDIO`.
- [ ] `Ctrl+D`, `Ctrl+E`, `Ctrl+L`, `Ctrl+N`, `Ctrl+T`, `Ctrl+O`, `Ctrl+F`, `Ctrl+R` NO se reintroducen. Grep debe ser negativo.
- [ ] Double `Ctrl+C` funcional: abort stream → clear input → arm window (1.5s) → exit. Tests `ctrl_c_*` en `src/app/tests.rs` verdes.
- [ ] Hints del footer (`src/ui/widgets/hints.rs`) leen del estado, no son strings hardcodeados.

### 7.5 Configuracion y providers

- [ ] Rutas documentadas (`~/.config/ingenieria-tui/config.json`, `~/.config/ingenieria-tui/history/`, `.mcp.json`, `hooks.json`) coinciden con `src/config.rs`.
- [ ] Env vars documentadas coinciden con `std::env::var` reales (grep en `src/`).
- [ ] Providers documentados (GitHub Copilot, Claude API, Mock) coinciden con impls de `ChatProvider` en `src/services/chat/`.
- [ ] Precios de modelos (Sonnet 4, Haiku 4.5, Opus 4) en docs-site coinciden con `ingenieria-api/pricing.rs` y con precios oficiales.

### 7.6 Themes y factories

- [ ] 6 themes documentados coinciden con archivos en `crates/ingenieria-ui/src/theme/`: `tokyonight.rs` (default), `solarized.rs`, `gruvbox.rs`, `monokai.rs`, `matrix.rs`, `high_contrast.rs`.
- [ ] 4 factories documentadas (Net, Ang, Nest, All) existen como constantes / enum en `src/domain/` o `src/state/`.
- [ ] Skills, workflows y policies por factory listadas en docs-site existen como categorias reales, no inventadas.

### 7.7 Arquitectura

- [ ] Diagrama Action-Reducer en `reference/architecture.mdx` refleja flujo real.
- [ ] Lista de workers en docs coincide con los **10** reales de `src/workers/mod.rs`.
- [ ] Modulos descritos existen en `src/`.

---

## §8. Formato del reporte

Al ejecutar esta auditoria, entregar el reporte en este formato exacto:

```
## Reporte de Auditoria — YYYY-MM-DD

### Resumen
- Criticos: N
- Altos: N
- Medios: N
- Bajos: N
- Secciones auditadas: §1, §2.1..§2.6, §3..§7
- Secciones saltadas: [lista con razon]

### Hallazgos

#### [CRITICO] Titulo corto
- Archivo: path/to/file.rs:123
- Regla violada: §X.Y del checklist
- Evidencia: <snippet o descripcion concreta>
- Fix sugerido: <que hacer>

#### [ALTO] ...
#### [MEDIO] ...
#### [BAJO] ...

### Secciones sin hallazgos (passed)
- §1.1 Naming ✓
- §2.1 ingenieria-domain ✓
- ...

### Metricas baseline (opcional)
- LOC: <numero>
- Archivos > limite no-documentados: <numero>
- `format!` / `.clone()` / `.to_string()` en `ui/`: <numeros actuales>
- Deps duplicadas: <lista de `cargo tree -d`>
```

---

## Anexo: ejecucion en paralelo

Para audits rapidas, dividir el trabajo por seccion:

| Agente | Cubre | Tiempo estimado |
|--------|-------|-----------------|
| A | §1 + §2.1..§2.5 (crates) | rapido, archivos pocos |
| B | §2.6 (binary) | el mas largo |
| C | §3 (performance) | cross-cutting |
| D | §4 + §5 + §6 | error / seguridad / flags |
| E | §7 (sync docs-site) | requiere leer `docs-site/` |

Consolidar hallazgos en un solo reporte.
