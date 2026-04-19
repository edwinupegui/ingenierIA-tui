# Manejo de Errores

## Principio general

```
thiserror -> para errores tipados en services/, domain/ y workspace crates
anyhow    -> para propagacion en app/ y main.rs
Action    -> para comunicar errores al usuario via UI
```

Un error NUNCA debe causar panic en produccion. Todo error llega al usuario como notificacion o status.

---

## Propagacion por capa

### services/ — Retornar Result con error tipado

```rust
pub async fn fetch_documents(url: &str, factory: &str) -> Result<Vec<DocumentSummary>, IngenieriaError> {
    let response = client.get(url).send().await?;
    if !response.status().is_success() {
        return Err(IngenieriaError::InvalidResponse(format!("HTTP {}", response.status())));
    }
    Ok(response.json().await?)
}
```

### workers/ — Convertir a Action

Los workers capturan el error y lo envian como Action. NUNCA hacen `.unwrap()` en un Result de servicio.

```rust
match ingenieria_client::check_health(&url).await {
    Ok(health) => tx.send(Action::HealthResult(health)).await.ok(),
    Err(e) => tx.send(Action::HealthError(e.to_string())).await.ok(),
};
```

### app/ — anyhow para propagacion interna

### main.rs — Capturar con `.context()` y mostrar

### workspace crates — thiserror con tipos especificos

Cada crate define sus propios tipos de error:
- `ingenieria-domain`: `StructuredFailure` para failures del chat, `FailureScenario` para categorias de fallo
- `ingenieria-api`: `RetryError` para reintentos agotados, errores de pricing
- `ingenieria-runtime`: errores de session store, audit, permissions
- `ingenieria-tools`: errores de validacion bash, hooks

---

## Tipos de error especializados

### StructuredFailure (ingenieria-domain)

Error estructurado para failures de chat streams. Captura categoria del fallo, mensaje amigable y posible recovery recipe:

```rust
pub struct StructuredFailure {
    pub scenario: FailureScenario,
    pub message: String,
    pub recovery_hint: Option<String>,
}
```

Usado por el Recovery Engine (E42) para mapear a recetas de recuperacion.

### Retry y Model Fallback (ingenieria-api)

Patron de reintentos con backoff exponencial para llamadas HTTP:

```rust
// ingenieria-api/src/retry.rs
pub async fn with_retry<F, T>(config: RetryConfig, f: F) -> Result<T>
```

Si un modelo falla consistentemente, `model_fallback.rs` sugiere un modelo alternativo compatible.

### Stream Stall Detection

`services/chat/stream_monitor.rs` detecta cuando un stream deja de enviar deltas y emite `Action::StreamWarning` o `Action::StreamTimeout`.

---

## Presentacion al usuario

| Nivel | Color | Donde | Duracion |
|-------|-------|-------|----------|
| Error | `RED` | Toast flotante | Hasta cierre manual |
| Warning | `YELLOW` | Status bar / toast | 5 segundos |
| Info | `BLUE` | Status bar | 3 segundos |
| Success | `GREEN` | Toast | 3 segundos |

Errores durante streaming se muestran inline como mensaje del sistema en el chat.

### Recovery Engine (E42)

Para failures no-chat (MCP handshake, config validation, worker crash, disco lleno), el Recovery Engine (`services/recovery_engine.rs`) interpreta el `StructuredFailure`, resuelve una receta de recuperacion, y genera toasts con sugerencias accionables.

### Doctor (diagnosticos)

`services/doctor.rs::run_checks` genera un `DoctorReport` con 7 chequeos:
**MCP Server** (health + latency), **Config** (validate_config_files),
**MCP Tools** (count vs expected), **Features** (flags compilados),
**LSP** (server detectado + diagnosticos activos), **IDE Bridge**
(estado del axum server) y **Disk** (config dir escribible).

Los subsistemas que antes tenian slashes dedicados (`/lsp-status`,
`/lsp-diag`, `/bridge-status`, `/features`, `/hooks`) fueron absorbidos
aqui. El snapshot de LSP + bridge se pasa via `DoctorInputs` para evitar
compartir estado vivo con el tokio task del scan.

Accesible via `:doctor` (command palette). Toggle: si el panel esta abierto,
la segunda invocacion lo cierra sin re-escanear.

---

## Patrones prohibidos

1. **`.unwrap()` en produccion** — Usar `?` o `unwrap_or`
2. **`String` como tipo de error** — Usar thiserror en services, anyhow en app
3. **Silenciar errores** (`let _ = save();`) — Loggear con `tracing::warn!` si no puedes propagar
4. **`.expect()` sin justificacion** — Si es invariante, el mensaje debe explicar por que

---

## Boundaries del sistema

### Input del usuario — Validar ANTES de procesar

```rust
if url.is_empty() {
    self.state.wizard.error = Some("URL no puede estar vacia".into());
    return;
}
```

### Respuestas HTTP — Verificar status antes de deserializar

### Archivos de disco — Manejar ausencia y corrupcion con defaults

Patron best-effort: hooks, plugins, onboarding cargan con defaults si el archivo falta o esta corrupto. No bloquean startup.

### MCP — Timeout + graceful degradation

Si el MCP server no responde al handshake, la TUI funciona sin MCP tools. Cache de documentos offline cubre el degraded mode.

---

## Logging

| Capa | Que loggear |
|------|-------------|
| `services/` | `error!` en fallos HTTP, `debug!` en requests |
| `workers/` | `warn!` en reintentos, `info!` en conexion/desconexion |
| `app/` | `debug!` en actions procesadas |
| `ui/` | NUNCA |
| `main.rs` | `info!` al iniciar y terminar |
| `workspace crates` | `debug!` en operaciones internas, `warn!` en fallbacks |

Variable `RUST_LOG` controla niveles: `RUST_LOG=debug cargo run`
