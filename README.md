<div align="center">

```
    _                        _          _______ 
   (_)___  ____ ____  ____  (_)__  ____/  _/   |
  / / __ \/ __ `/ _ \/ __ \/ / _ \/ ___/ // /| |
 / / / / / /_/ /  __/ / / / /  __/ / _/ // ___ |
/_/_/ /_/\__, /\___/_/ /_/_/\___/_/ /___/_/  |_|
        /____/                                  
```

hecho con :heart: por **Edwin Upegui**

**Tu equipo dice QUE. ingenierIA construye el COMO.**

*Terminal UI para el ecosistema ingenierIA MCP Server*

[![CI](https://github.com/edwinupegui/ingenierIA-tui/actions/workflows/ci.yml/badge.svg)](https://github.com/edwinupegui/ingenierIA-tui/actions/workflows/ci.yml)
[![Version](https://img.shields.io/badge/version-0.23.0-68217A)](https://github.com/edwinupegui/ingenierIA-tui/releases)
[![Rust](https://img.shields.io/badge/rust-2021-orange)](https://www.rust-lang.org)
[![Plataformas](https://img.shields.io/badge/plataformas-macOS%20%7C%20Windows%20%7C%20Linux-blue)](#plataformas-soportadas)

</div>

---

## Que es ingenierIA TUI

Panel de control en terminal para el servidor [ingenierIA MCP](https://github.com/edwinupegui/ingenieria-mcp). Combina navegacion de documentos, chat con AI y herramientas de observabilidad en una sola interfaz.

### Funcionalidades principales

- **Dashboard interactivo** -- navega skills, policies, ADRs, workflows, commands y agents
- **Chat con AI** -- GitHub Copilot o Claude API con streaming, markdown y syntax highlighting
- **MCP integrado** -- tools, workflows, compliance gates y sync via protocolo MCP
- **Contexto inteligente** -- git diff, archivos recientes y errores de compilacion inyectados automaticamente
- **Session persistence** -- conversaciones guardadas con auto-save, /resume y /sessions
- **Context compaction** -- /compact y auto-compact al 80% del context window
- **Cost tracker** -- token usage en tiempo real, costo por sesion, budget alerts
- **Planning mode** -- /plan para generar planes estructurados antes de ejecutar
- **Code blocks aplicables** -- detecta codigo en respuestas del AI y permite aplicarlo a archivos
- **Syntax highlighting** -- coloreado de keywords, strings, comentarios, numeros y tipos (Rust, TS, C#)
- **Diff coloreado** -- lineas +/- con colores y backgrounds sutiles
- **Permission system** -- 3 modos (Standard/Permissive/Strict) con modal visual de aprobacion
- **AutoSkill** -- deteccion extendida de 26+ tecnologias, combos, skills de ingenierIA MCP + skills externos (skills.sh)
- **SubAgents y Teams** -- orquestacion multi-agente con roles especificos y ejecucion en paralelo
- **Cron Scheduler** -- tareas recurrentes con expresiones cron (6+1 campos)
- **LSP Integration** -- auto-detecta servidores de lenguaje e inyecta diagnosticos en el contexto AI
- **IDE Bridge** -- servidor HTTP local para comunicacion bidireccional con VS Code y JetBrains
- **Process Monitor** -- ejecucion de procesos shell en background con streaming de output
- **Hooks configurables** -- shell commands disparados por eventos del TUI
- **MCP degraded mode** -- funciona offline con notificaciones de reconexion
- **Tool monitor** -- feed en tiempo real de todas las invocaciones de tools del MCP server
- **Enforcement dashboard** -- visualizacion de hooks/guards de compliance
- **Agent panel** -- tareas agrupadas por tool con stats de latencia
- **Toast system** -- notificaciones de 4 niveles con auto-dismiss
- **Busqueda fuzzy** -- encuentra documentos al instante desde el teclado
- **Multi-turn tools** -- el AI ejecuta herramientas en loops hasta completar la tarea (max 10 rounds)
- **Keybindings configurables** -- personaliza atajos via keybindings.json

---

## Instalacion

### Script (Mac / Linux) — recomendado

```bash
curl -fsSL https://raw.githubusercontent.com/edwinupegui/ingenierIA-tui/main/scripts/install.sh | sh
```

### Script (Windows PowerShell)

```powershell
irm https://raw.githubusercontent.com/edwinupegui/ingenierIA-tui/main/scripts/install.ps1 | iex
```

### Instaladores nativos

Descarga el instalador de la [ultima release](https://github.com/edwinupegui/ingenierIA-tui/releases/latest):

| Plataforma | Archivo |
|-----------|---------|
| macOS Apple Silicon | `ingenieria-macos-apple-silicon.pkg` |
| macOS Intel | `ingenieria-macos-intel.pkg` |
| Windows 10/11 | `ingenieria-windows-x64-setup.exe` |
| Linux x86-64 | `ingenieria-x86_64-unknown-linux-musl.tar.gz` |

### npm (proxima version)

```bash
npm install -g @ingenieria/tui   # pendiente de configuracion de org npm
```

---

## Uso

```bash
ingenierIA                    # Arrancar (wizard en primera ejecucion)
ingenierIA --config           # Reconfigurar perfil
ingenierIA --server-url URL   # Override de URL para esta sesion
ingenierIA --version          # Ver version
```

### First-run wizard

La primera vez que ejecutas `ingenierIA`, un wizard te guia en 4 pasos:

1. **URL del servidor** -- se valida conectando a `/api/health`
2. **Tu nombre** -- visible para el equipo en sesiones activas
3. **Proveedor de AI** -- GitHub Copilot (OAuth) o Claude API (API key)
4. **Tu rol** -- Backend (.NET), Frontend (Angular), BFF (NestJS) o Full Stack

La config se guarda en `~/.config/ingenieria-tui/config.json`.

---

## Atajos de teclado

### Globales (cualquier pantalla)

| Tecla | Accion |
|-------|--------|
| `Ctrl+C` | Abortar / salir (doble tap) |
| `Esc` | Cerrar overlay / volver atras |
| `:` | Command palette |
| `Tab` | Ciclar factory |

### Splash

| Tecla | Accion |
|-------|--------|
| Escribir | Componer mensaje para iniciar chat |
| `Enter` | Enviar e iniciar chat |
| `Ctrl+D` | Abrir dashboard |
| `Ctrl+N` | Inicializar proyecto |

### Dashboard

| Tecla | Accion |
|-------|--------|
| `Up` / `Down` | Navegar sidebar |
| `Space` | Expandir/colapsar seccion |
| `Enter` | Abrir documento |
| `y` | Copiar contenido al clipboard |
| `Y` | Copiar slash command (`/nombre`) |
| `/` | Busqueda fuzzy |
| `T` | Tool Monitor (overlay) |
| `H` | Enforcement Dashboard (overlay) |
| `K` | Agent Panel (overlay) |
| `N` | Notification Center (overlay) |
| `S` | AutoSkill scan |
| `Shift+Up/Down` | Scroll del preview |

### Chat

| Tecla | Accion |
|-------|--------|
| `Enter` | Enviar mensaje |
| `Shift+Enter` | Nueva linea (multilinea) |
| `Up` / `Down` | Scroll del chat o historial de mensajes |
| `$` | Panel detallado de costos |
| `Ctrl+E` | Volver a la pantalla inicial |
| `Ctrl+L` | Limpiar chat |
| `y` / `n` | Aprobar/denegar tool (modal de permisos) |

### Slash commands (en el chat)

**Exploracion MCP**:

| Comando | Descripcion |
|---------|-------------|
| `/skills` | Explorar skills de ingenierIA |
| `/commands` | Explorar commands de ingenierIA |
| `/adrs` | Explorar ADRs de ingenierIA |
| `/policies` | Explorar policies de ingenierIA |
| `/agents` | Explorar agents de ingenierIA |

**Gestion del chat**:

| Comando | Descripcion |
|---------|-------------|
| `/clear` | Limpia el historial |
| `/model <nombre>` | Muestra o cambia el modelo AI |
| `/plan` | Activa/desactiva modo planning |
| `/permissions` | Ciclar modo de permisos |
| `/compact` | Compacta mensajes para liberar contexto |
| `/memory` | Cuanto contexto queda disponible |
| `/costs` | Tokens consumidos y costo estimado |

**Contexto**:

| Comando | Descripcion |
|---------|-------------|
| `/diff` | Inyecta el git diff actual como contexto |
| `/files` | Inyecta archivos modificados como contexto |
| `/apply [n]` | Aplica un code block al archivo destino |
| `/blocks` | Lista code blocks de la conversacion |
| `/workflow <nombre>` | Carga workflow ingenierIA |
| `/compliance` | Valida policies y ADRs sobre el proyecto |
| `/autoskill` | Detecta techs del proyecto y sugiere skills |

**Sesiones**:

| Comando | Descripcion |
|---------|-------------|
| `/history` | Sesiones guardadas para retomar |
| `/resume` | Retoma la ultima sesion |
| `/fork` | Crea rama de la conversacion actual |
| `/export` | Exporta la sesion |

**Agentes y automatizacion**:

| Comando | Descripcion |
|---------|-------------|
| `/spawn <rol>` | Lanza un sub-agente (coder, reviewer, planner) |
| `/team-<template>` | Equipo multi-agente (fullstack, research, planning) |
| `/cron-add <expr> <cmd>` | Programa tarea recurrente |
| `/cron-list` | Lista cron jobs activos |
| `/monitor <cmd>` | Ejecuta proceso en background con streaming |

### Command palette (`:`) — global

| Categoria | Comando | Descripcion |
|-----------|---------|-------------|
| Sincronizacion | `sync net/ang/nest/all` | Descarga skills, policies y ADRs |
| Estado | `health` | Consulta /api/health |
| Estado | `sessions` | Lista sesiones activas |
| Estado | `compliance` | Valida cumplimiento |
| Contexto | `context net/ang/nest` | Cambia factory activa |
| Configuracion | `config` | Abre wizard |
| Configuracion | `init` | Crea .ingenieria.json |
| Configuracion | `change-model` | Cambia modelo AI |
| Configuracion | `autoskill` | Detecta stack tecnologico |
| Configuracion | `theme` | Selector de themes con live preview |
| Diagnostico | `doctor` | Estado del sistema (MCP, LSP, IDE Bridge, Disk) |
| Chat | `history` | Conversaciones anteriores |
| Chat | `diff` | Inserta git diff como contexto AI |
| Workflows | `workflow <nombre>` | Ejecuta workflow paso a paso |

---

## Paneles de observabilidad

Accesibles como overlays desde el dashboard:

| Panel | Tecla | Descripcion |
|-------|-------|-------------|
| **Tool Monitor** | `T` | Feed en tiempo real de invocaciones MCP |
| **Enforcement** | `H` | Hooks/guards de compliance |
| **Agent Panel** | `K` | Sub-agentes activos con stats |
| **Notifications** | `N` | Historial completo de toasts |
| **Cost Detail** | `$` | Tokens input/output, costo por categoria |
| **Doctor** | `:doctor` | Diagnosticos del sistema |
| **Monitor Panel** | `:monitor` | Output de procesos en background |

---

## AutoSkill: deteccion de tecnologias

Al presionar `S` en el dashboard o `:autoskill` en la paleta, la TUI escanea el proyecto y sugiere skills de dos fuentes:

**1. Skills de ingenierIA MCP** — cargados via `get_workflow`, especificos por factory.

**2. Skills externos (skills.sh)** — instalados via `npx skills add`.

### Tecnologias detectadas (26 reglas)

| Ecosistema | Tecnologias |
|-----------|------------|
| **.NET** (factory: net) | .NET, Web API, Entity Framework Core, MediatR, FluentValidation, xUnit |
| **Angular** (factory: ang) | Angular, Angular Material, NgRx, RxJS, Karma/Jasmine |
| **NestJS** (factory: nest) | NestJS, Swagger, TypeORM, Config, Jest |
| **Next.js/React** | React, Next.js |
| **Cross-cutting** | TypeScript, Tailwind CSS, Prisma, ESLint, Docker, Playwright, Vitest, shadcn/ui |

---

## Arquitectura

```
ingenierIA MCP Server (:3001)
    |
    +-- REST: /api/health, /api/documents, /api/search
    +-- SSE:  /api/events, /api/tool-events, /api/hook-events
    +-- MCP:  /claude/sse (8 tools: get_factory_context, get_workflow, ...)
    |
    v
ingenierIA TUI — monorepo Rust (5 workspace crates)
```

### Workspace crates

| Crate | Responsabilidad |
|-------|----------------|
| `ingenieria-domain` | Tipos puros del dominio (Document, Event, Health, Chat, etc.) |
| `ingenieria-ui` | Theme system (10+ variantes), design system, a11y, primitives |
| `ingenieria-api` | API types, pricing, metrics, retry, model fallback |
| `ingenieria-runtime` | Session, audit, memory, permissions, config validation |
| `ingenieria-tools` | Bash safety, hooks, MCP protocol types |

### Patron Action-Reducer (Elm-like)

```
Workers (tokio::spawn) ──> Action ──> MPSC Channel ──> App::handle() ──> AppState ──> render()
```

| Modulo | Responsabilidad |
|--------|----------------|
| `src/app/` | Reducer handlers, keybindings, spawners, slash commands |
| `src/state/` | AppState completo — unica fuente de verdad |
| `src/actions.rs` | Enum Action con 100+ variantes |
| `src/services/` | HTTP, providers AI, MCP client, tools, context, compactor |
| `src/workers/` | 11 tasks async de larga duracion |
| `src/ui/` | Render puro con `&AppState` inmutable |

**Regla critica:** `ui/` nunca importa logica de `services/`. `domain/` no depende de nada interno.

---

## Proveedores de AI

| Proveedor | Autenticacion | Token tracking |
|-----------|---------------|----------------|
| **GitHub Copilot** | OAuth device flow | No |
| **Anthropic Claude** | API key | Si (input/output/cache tokens) |
| **Mock** | Sin configuracion | Para testing offline |

---

## Stack

| Componente | Crate | Version |
|-----------|-------|---------|
| TUI Framework | `ratatui` | 0.30 |
| Terminal | `crossterm` | 0.29 |
| Async runtime | `tokio` | 1 |
| HTTP client | `reqwest` | 0.13 |
| WebSocket | `tokio-tungstenite` | 0.24 |
| Fuzzy search | `nucleo-matcher` | 0.3 |
| Markdown | `pulldown-cmark` | 0.13 |
| Syntax | `syntect` | 5 |
| Clipboard | `arboard` | 3 |
| CLI args | `clap` | 4 |
| Errores | `thiserror` + `anyhow` | 2 / 1 |
| File watch | `notify` | 7 |
| Cron | `cron` | 0.16 |

---

## Configuracion

Prioridad de resolucion:

1. `--server-url` (CLI argument)
2. `INGENIERIA_SERVER_URL` (env var)
3. `.mcp.json` en el directorio actual (busca hacia arriba)
4. `~/.config/ingenieria-tui/config.json` (config global)
5. `http://localhost:3001` (fallback)

Variables de entorno: `INGENIERIA_SERVER_URL`, `INGENIERIA_DEVELOPER`, `INGENIERIA_MODEL`, `INGENIERIA_PROVIDER`.

| Archivo | Proposito |
|---------|-----------|
| `~/.config/ingenieria-tui/config.json` | Config global (server, developer, model, factory) |
| `~/.config/ingenieria-tui/history/` | Sesiones guardadas con mensajes y tool calls |
| `~/.config/ingenieria-tui/keybindings.json` | Atajos de teclado personalizados |
| `~/.config/ingenieria-tui/claude_key` | API key de Claude (permisos 0600) |

### Keybindings personalizados

Crea `~/.config/ingenieria-tui/keybindings.json`:

```json
{
  "toggle_sidebar": "space",
  "search": "/",
  "command_palette": ":",
  "copy": "y",
  "factory_switch": "tab"
}
```

---

## Plataformas soportadas

| Sistema | Arquitectura | Instalador | Binario portable |
|---------|-------------|------------|------------------|
| macOS 13+ | Apple Silicon (M1-M4) | .pkg | .tar.gz |
| macOS 12+ | Intel x86_64 | .pkg | .tar.gz |
| Windows 10/11 | x86_64 | .exe (NSIS) | .zip |
| Ubuntu 20.04+ | x86_64 | -- | .tar.gz (static musl) |

> Windows requiere **Windows Terminal** o **PowerShell 7** para renderizado correcto.

---

## Desarrollo

```bash
cargo build                                    # Compilar (features=full)
cargo build --no-default-features --features minimal  # Build minimo
cargo run                                      # Ejecutar
cargo test                                     # Tests
cargo clippy -- -D warnings                    # Lint (cero warnings)
cargo fmt                                      # Formatear
RUST_LOG=debug cargo run                       # Con logs de debug
```

### Feature flags

| Flag | Default | Que habilita |
|------|---------|-------------|
| `full` | si | copilot + mcp + autoskill + ide |
| `minimal` | — | Core TUI + Claude API only |
| `copilot` | via full | GitHub Copilot OAuth + provider |
| `mcp` | via full | MCP tool discovery + execution |
| `autoskill` | via full | Tech detection + skill installer |
| `ide` | via full | IDE Bridge HTTP server |

### Reglas de codigo

- Max 400 lineas por archivo, max 40 por funcion
- No `.unwrap()` en produccion — usar `?` o `unwrap_or`
- No efectos secundarios en funciones de render (`ui/*.rs`)
- `cargo fmt` + `cargo clippy` antes de cada commit
- Colores siempre via tokens del theme system (`state.active_theme.colors()`)
- Tipos nuevos en workspace crates — nunca en `src/` directamente

Ver `docs/` para guias completas de arquitectura, errores, ownership y performance.

---

## Release

Los binarios e instaladores se compilan automaticamente con GitHub Actions al crear un tag:

```bash
git tag tui-v<VERSION>
git push origin tui-v<VERSION>
```

---

Desarrollado por **Edwin Upegui**
