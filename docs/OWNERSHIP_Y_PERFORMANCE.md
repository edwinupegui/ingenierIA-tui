# Ownership, Async y Performance

> Decisiones de ownership, ciclo de vida de tareas async y optimizaciones de rendimiento.

---

## Ownership y borrowing

### Principio: ownership claro, clones justificados

| Razon para clonar | Aceptable |
|-------------------|-----------|
| Cruzar boundary de tokio::spawn | Si |
| Dato pequeno y simple (String corto, enum) | Si |
| Arc para datos grandes compartidos | Si |
| Evitar pelear con el borrow checker | **NO** |

### Patrones por capa

- **Domain types** (ingenieria-domain): `String` owned — vienen de JSON, viven en AppState
- **AppState**: owned por App, pasa `&ref` inmutable a UI
- **Workers**: clonar solo lo minimo al cruzar spawn boundary (tx + datos necesarios, NO todo el state)
- **Datos grandes compartidos**: `Arc<T>` cuando se comparten entre tasks
- **Workspace crate types**: owned por AppState, workspace crates nunca poseen estado mutable

### Cuando usar cada tipo

| Tipo | Cuando |
|------|--------|
| `String` | Datos en structs que viven en AppState |
| `&str` | Parametros de funciones que solo leen |
| `Arc<T>` | Datos compartidos entre tokio::spawn tasks (ej: `Arc<IngenieriaClient>`, `Arc<McpLifecycleManager>`) |
| `Cow<'_, str>` | Evitar allocation condicional (raro) |
| `Box<dyn Trait>` | Polimorfismo: ChatProvider, Tool, Plugin, McpTransport |

---

## Ciclo de vida de tareas async

### Workers de larga duracion

| Worker | Duracion | Cancelacion | Feature gate |
|--------|----------|-------------|-------------|
| `keyboard` | Siempre activo | Se detiene al salir de la app | — |
| `tick` | Siempre activo | Se detiene al salir de la app | — |
| `health` | Siempre activo | Se detiene al salir de la app | — |
| `sse` | Mientras hay conexion | Reconecta con backoff | — |
| `hook_events` | Siempre activo | Se detiene al salir de la app | — |
| `tool_events` | Siempre activo | Se detiene al salir de la app | — |
| `cron_worker` | Siempre activo | Se detiene al salir de la app | — |
| `file_watcher` | Siempre activo | Se detiene al salir de la app | — |
| `process_monitor` | Per-monitor | Termina con el proceso | — |
| `lifecycle` | Framework (E08) | Se detiene al salir de la app | — |

Se cancelan naturalmente cuando el `Sender` del channel se dropea.

### Servicios de larga duracion (no workers)

| Servicio | Duracion | Cancelacion | Feature gate |
|---------|----------|-------------|-------------|
| MCP Lifecycle Manager | Mientras hay config | Shutdown explicito | `mcp` |
| LSP Client | Mientras hay servidor | Shutdown via `Arc<AtomicBool>` | — |
| IDE Bridge Server | Mientras la app corre | Drop del handle | `ide` |

### Tasks de corta duracion

Patron: spawn + AbortHandle para cancelacion.

```rust
fn spawn_chat_stream(&mut self) {
    if let Some(handle) = self.state.chat.stream_abort.take() {
        handle.abort();
    }
    let tx = self.tx.clone();
    let handle = tokio::spawn(async move { /* streaming */ });
    self.state.chat.stream_abort = Some(handle.abort_handle());
}
```

### Reglas de cancelacion

| Evento | Accion |
|--------|--------|
| Esc durante streaming | abort() el stream |
| Cambio de screen durante stream | abort() el stream |
| Nuevo stream iniciado | abort() el anterior |
| Primer Ctrl+C con stream activo | abort() el stream (NO arma quit) |
| Primer Ctrl+C sin stream | arma `quit_armed_until = tick + 8` (~2s @ 4Hz) |
| Segundo Ctrl+C dentro de la ventana | sale de la app |
| Segundo Ctrl+C fuera de la ventana | re-arma |
| App se cierra | Todos se cancelan (channel dropea) |

Estado de la ventana de quit: `AppState::quit_armed_until: Option<u64>`
(tick-based, determinista para tests). Logica en
`src/app/quit_handler.rs::on_ctrl_c`. Los hints muestran un span amarillo
`ctrl+c otra vez para salir` mientras esta armada.

### Timeouts

| Operacion | Timeout |
|-----------|---------|
| Health check | 5s |
| Document fetch | 15s |
| Search | 10s |
| Chat streaming | Sin timeout (cancelable con Esc) |
| OAuth device flow | 300s |
| MCP handshake | 10s |
| LSP initialize | 10s |

### MPSC Channel

Buffer size: 100 (bounded). Suficiente para rafagas de keyboard + tick + SSE. Backpressure natural si el reducer se atrasa.

---

## Performance

### Ciclo de render

```
Tick (250ms / 4Hz) -> Action::Tick -> handle() -> state mutation -> render(frame, &state)
```

Cada 250ms se renderiza todo el frame. Trabajo costoso en `render()` afecta la fluidez.

### Reglas para codigo nuevo

1. **Nunca allocar en funciones de render** salvo que sea inevitable. Preferir `&str`, `Cow<str>`, spans pre-computados.
2. **Cachear resultados costosos** (markdown parsed, search results). Invalidar cuando los datos fuente cambian.
3. **Pre-allocar Vecs** cuando el tamano es conocido o estimable.
4. **Clone solo en spawn boundaries**. Clonar lo minimo necesario.
5. **Medir antes de optimizar**. Usar profiling real, no intuicion.

### Optimizaciones implementadas

- Markdown cacheado en PreviewState (se re-parsea solo cuando el documento cambia)
- Search results cacheados hasta que cambian los documentos fuente
- Sidebar pre-computado al cargar documentos (no en cada frame)
- CacheLayer con invalidacion selectiva (docs, doc_details, search)
- Buffer diff para minimizar escrituras al terminal (`ingenieria-ui/buffer_diff`)
- Frame throttle para limitar FPS (`ingenieria-ui/frame_throttle`)
- Streaming markdown incremental (E29) — solo re-parsea el delta

### Areas de mejora identificadas

- Allocations en render loops (sidebar items, events log): usar `Cow<str>` o pre-computar
- Local search con allocation por documento por keystroke: pre-computar haystack
- Chat streaming con allocations por chunk: reusar buffers

### Profiling

```bash
cargo flamegraph -- --server-url http://localhost:3001
```

| Operacion | Objetivo |
|-----------|----------|
| Frame render (sin markdown) | < 2ms |
| Frame render (con markdown) | < 5ms |
| Local search (500 docs) | < 5ms |
| Chat delta processing | < 1ms |
