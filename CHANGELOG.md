# Changelog

## [0.9.5] - 2026-04-06

### Chore: unificar y corregir versionamiento

- **Bump version a 0.9.5**: Cargo.toml, todos los package.json de npm (ingenieria, darwin-arm64, darwin-x64, linux-x64, win32-x64)
- **Eliminar versiones hardcodeadas**: README usa placeholder `<VERSION>` en ejemplo de tag, docs-site elimina version del mockup terminal para no requerir actualización manual
- **Fix version mismatch**: el binario compilado ahora reporta 0.9.5 via `ingenierIA --version` (clap toma la version de Cargo.toml en compilación)

## [0.9.4] - 2026-04-07

### UX: Command Palette global, categorias, localizacion, npm distribution

- **Command palette global**: ahora se abre con `:` desde cualquier pantalla (Splash, Dashboard, Chat), no solo Dashboard. El overlay se renderiza como capa global en `ui/mod.rs`
- **Categorias en la paleta**: los comandos se agrupan por categoria (Sincronizacion, Estado, Contexto, Navegacion, Configuracion, Chat, Workflows) con headers visuales cuando no hay filtro activo
- **Descripciones mejoradas**: todos los comandos de la paleta y slash commands reescritos con textos mas descriptivos y accionables
- **Localizacion a espanol**: titulos de overlays traducidos — "Paleta de Comandos", "Permiso Requerido", "Monitor de Tools", "Costo de Sesion", botones "Permitir"/"Denegar", "Herramienta:" en modals
- **Auto-switch de factory**: al detectar el tipo de proyecto, la factory se cambia automaticamente al contexto correcto
- **Slash autocomplete full-width**: el popup de autocompletado en chat ahora usa el ancho completo del terminal, centrado
- **Unicode-safe truncation**: corregido truncamiento de strings para respetar limites de caracteres UTF-8 (previene panics en doc_picker, chat_tools, slash_autocomplete)
- **Scroll en search overlay**: los resultados de busqueda ahora soportan scroll cuando hay mas resultados que lineas visibles
- **Fondo consistente SURFACE**: overlays de command palette y search usan SURFACE como background en vez de BG, para mejor contraste
- **Comando `go` removido**: la navegacion por URI ahora se detecta automaticamente al pegar `ingenieria://...` en la paleta
- **Helpers `ensure_dashboard`/`ensure_chat`**: los comandos de la paleta navegan automaticamente a la pantalla correcta antes de ejecutarse
- **Docs-site en espanol**: paginas de referencia (architecture, configuration, keybindings) traducidas al espanol; sidebar "Reference" renombrada a "Referencia"
- **Distribucion npm**: paquetes `@your-org/ingenieria` publicos con binarios nativos por plataforma. `npm install -g` y funciona desde cualquier terminal sin configurar PATH
- **Fix clippy Rust 1.94**: corregido `needless_return` en wizard.rs para compatibilidad con rustc nuevos

## [0.8.0] - 2026-04-06

### AutoSkill: deteccion extendida + instalacion de skills externos

- **Nuevo modulo `autoskill_map`**: motor de deteccion de 26 tecnologias con soporte para packages, package prefixes (scoped), config files, file extensions y content markers en .csproj
- **Deteccion profunda por ecosistema**:
  - .NET: dotnet, Web API, Entity Framework Core, MediatR, FluentValidation, xUnit
  - Angular: core, Material, NgRx, RxJS, Karma/Jasmine
  - NestJS: core, Swagger, TypeORM, Config, Jest
  - Next.js/React: React, Next.js
  - Cross-cutting: TypeScript, Tailwind, Prisma, ESLint, Docker, Playwright, Vitest, shadcn/ui
- **Combo detection**: 9 reglas para combinaciones de techs (ej: NestJS + Prisma, Next.js + Tailwind, .NET + Angular Full Stack)
- **Skill mapping dual**:
  - Skills de ingenierIA MCP: sugeridos segun factory detectada, verificados contra docs disponibles
  - Skills externos (skills.sh): mapeados por tecnologia, estilo autoskills de midudev
- **Frontend bonus skills**: auto-agregados para proyectos con React, Angular, Next.js (frontend-design, accessibility, seo)
- **Nuevo modulo `skill_installer`**: instalacion via `npx skills add owner/repo --skill name -a claude-code` con concurrencia de 4 workers
- **Deteccion de agentes AI**: escanea ~/.claude, ~/.cursor, ~/.copilot, etc. para pasar `-a` al installer
- **Deteccion de skills instalados**: lee `skills-lock.json` o `.agents/skills/` o `.claude/skills/`
- **Nuevo slash command `/autoskill`**: escanea techs, detecta combos, sugiere skills de ambas fuentes con markdown formateado
- **Nuevo slash command `/install-skills`**: instala skills externos pendientes detectados por `/autoskill`
- **`Shift+S` y command palette `autoskill`**: ahora usan el scan extendido (antes solo listaban techs)
- **Limpieza**: removido `detect_technologies()` y `format_detected_techs()` de `init.rs` (migrados a `autoskill_map`)

## [0.7.1] - 2026-04-06

### Security
- `search_files` tool now validates glob results against project sandbox (prevents path traversal)
- Claude API key file saved with `0o600` permissions on Unix

### Performance
- HTTP timeouts added to Copilot (30s) and Claude (60s) clients
- Chat message history uses `Arc<[ChatMessage]>` instead of deep clone per completion
- Input history capped at 200 entries to prevent unbounded growth

### Refactor — file size compliance (max 400 lines)
- `app/keys.rs` (815→371) split into `keys_chat`, `keys_wizard`, `keys_splash`
- `app/mod.rs` (431→317) handlers extracted to `handler_actions`
- `ui/wizard.rs` (672→384) split into `wizard_auth`, `wizard_model`
- `ui/chat.rs` (567→155) split into `chat_render`, `chat_tools`
- `ui/widgets/markdown.rs` (495→249) split into `markdown_code`
- `state/chat_state.rs` (653→278) types extracted to `chat_types`
- `services/init.rs` (641→244) split into `init_gen`, `init_templates`
- `services/tools/fs.rs` (513→302) split into `fs_write`
- `app/chat.rs` (494→292) tools extracted to `chat_tools`
- `app/spawners.rs` (459→343) chat spawners to `spawners_chat`

### Code quality
- `MAX_BACKOFF_SECS` shared across 3 workers (was duplicated)
- `extract_time()` helper replaces 3 duplicate timestamp extractions
- `OUTPUT_PRICE` unified via `CostState::input_cost()`/`output_cost()`
- Inline `.chars().take(N).collect()` replaced with existing `truncate()` utility
- 17 `#[allow(dead_code)]` migrated to `#[expect(dead_code, reason = "...")]`
- 10+ single-letter variables renamed to descriptive names

### Docs sync
- Slash commands count updated from 19 to 26 (7 MCP explorer commands were missing)
- Missing keybindings documented: `Ctrl+E`, `Ctrl+L`, `Ctrl+Enter`/`Shift+Enter`
- Action enum variant count corrected from "100+" to "~74"

## [0.7.0] - 2026-04-05

### Skill Picker UX (breaking change)
- **Workflow shortcuts eliminados del slash autocomplete**: los 23 comandos directos (`/add-feature`, `/sprint`, etc.) ya no aparecen en el autocomplete de `/`. Ahora se accede via `/skills` picker
- `/workflows` redirige al picker de skills (ya no muestra lista hardcodeada)
- Constante `KNOWN_WORKFLOWS` eliminada — la lista de skills viene dinamicamente del servidor MCP

### Flujo select-then-type para Skills
- Al seleccionar un skill del picker, se inserta como prefijo en el input (ej: `/add-feature `)
- El usuario escribe su peticion despues del prefijo (ej: `/add-feature crear endpoint de pagos`)
- Al presionar Enter, se carga el skill via `get_workflow` (paquete completo con policies + ADRs) y el mensaje del usuario se envia automaticamente
- Si no se escribe argumento, el workflow queda listo esperando input del usuario
- Indicador visual: prefijo del comando en cyan bold, placeholder contextual cuando el input esta vacio
- Al borrar el prefijo, el skill seleccionado se limpia automaticamente

### Chat Scroll Fix
- Corregido calculo de scroll que usaba lineas logicas en vez de lineas wrapeadas (visuales)
- Corregido scroll up que no funcionaba cuando estaba en auto-bottom (`u16::MAX - 3` se clampeaba de vuelta al fondo)
- Nuevo: `scroll_up()` / `scroll_down()` en ChatState resuelven `u16::MAX` al valor real antes de operar
- `last_max_scroll` (via `Cell<u16>`) permite al render comunicar el max scroll a los key handlers
- Scroll down vuelve automaticamente a auto-bottom cuando llega al final

## [0.6.0] - 2026-04-05

### Document Picker (MCP)
- Nuevos slash commands `/skills`, `/commands`, `/adrs`, `/policies`, `/agents` que abren un picker interactivo con documentos del MCP filtrados por factory activa
- `/workflows` abre picker con los 23 workflows conocidos del MCP (add-feature, sprint, new-project, etc.)
- Busqueda en tiempo real dentro del picker (filtra por nombre y descripcion)
- Seleccion con Enter carga el documento completo al contexto del chat
- Funciona tanto desde la pantalla Home como desde el Chat
- Pre-carga de documentos al inicio de la app para respuesta inmediata

### Workflow Shortcuts
- 23 workflows del MCP disponibles como slash commands directos: `/add-feature`, `/sprint`, `/new-project`, `/review-pr`, `/health-check`, `/generate-tests`, etc.
- Aparecen en el autocomplete con prefijo "Workflow:" para distinguirlos
- Ejecucion inmediata al seleccionar del autocomplete (sin segundo Enter)

### Slash Autocomplete en Home
- El popup de autocompletado ahora funciona en la pantalla Splash/Home (antes solo en Chat)
- Seleccionar un comando del autocomplete lo ejecuta directamente

### UX Mejorado
- `Ctrl+E` para volver al inicio desde el chat (nuevo atajo)
- `/exit` slash command para volver a la pantalla Splash
- `/clear` ahora resetea completamente el ChatState
- Hints bar multi-fila: se expande automaticamente a 2+ filas si los atajos no caben en una
- Flechas `Up/Down` en chat sin input scrollean el contenido (con input navegan historial)
- Seleccion de slash commands del autocomplete ejecuta directamente (sin doble Enter)

### Bug Fixes
- **Space bar en Dashboard**: corregido bug donde `" "` no matcheaba el keybinding `"space"` para expandir/colapsar secciones
- **Config corrupto por tests**: `save_config()` ya no escribe al disco durante `cargo test`, previniendo que los valores de test sobreescriban la config del usuario
- **Hint `espacio expandir`** agregado al dashboard para hacer visible el atajo

## [0.2.0] - 2026-03-28

### Arquitectura
- Action-Reducer pattern (Elm-like) con modules por responsabilidad
- Theme centralizado en `ui/theme.rs` (19 tokens de color)
- State dividido en 7 modulos, app en 9 modulos
- 43 tests unitarios (reducer, config, codeblocks, URI)

### Chat competitivo
- Multi-turn tool loops (max 10 rounds por turno)
- 12 slash commands: /clear /model /diff /files /apply /blocks /go /history /load /workflow /compliance /help
- Contexto inteligente: git diff, archivos recientes y errores de compilacion inyectados al system prompt
- Historial persistente con auto-save en ~/.config/ingenieria-tui/history/
- Code blocks interactivos: deteccion automatica y /apply para escribir a archivos
- Permisos de herramientas: Safe (auto) / Ask (confirmacion y/n) / Dangerous
- Navegacion por URI ingenieria://type/factory/name

### Proveedores de AI
- GitHub Copilot via OAuth device flow (existente, mejorado)
- Claude API (Anthropic) via API key -- Messages API con streaming SSE
- Trait ChatProvider compartido entre proveedores
- Tool calling con ambos proveedores

### Integracion MCP
- MCP Client SSE transport para sc-mcp-ingenieria
- get_factory_context: contexto rico de factory via MCP
- get_workflow: carga y ejecucion de workflows ingenierIA
- validate_compliance: 4 gates (security, testing, coding-standards, ADRs)
- sync_project: tracking de documentos nuevos/modificados con badges
- SSE reactivo: reload y sync events actualizan el TUI en tiempo real

### Performance
- Cache de markdown renderizado en PreviewState (evita re-parse por frame)
- Eliminacion de clones redundantes
- Dependencias actualizadas: crossterm 0.29, reqwest 0.13, pulldown-cmark 0.13, dirs 6

### Factories
- Soporte completo para Net (.NET), Ang (Angular), Nest (NestJS), All (Full Stack)
- Sidebar con 6 secciones: Skills, Commands, Workflows, ADRs, Policies, Agents

## [0.1.2] - 2024

- Version inicial con dashboard, chat basico con Copilot, wizard de configuracion
