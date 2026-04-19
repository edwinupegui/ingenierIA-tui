# Comandos — `/` slash vs `:` palette

Fuente canónica de la taxonomía de comandos. Dos superficies mutuamente
excluyentes:

- **`/` slash** (42 entries, `src/state/chat_types.rs:SLASH_COMMANDS`) —
  exclusivamente "cosas del chat": turnos, contexto AI, agents/teams/monitors,
  todos, memoria, workflows ejecutables, cron, help.
- **`:` palette** (26 entries, `src/state/command_state.rs:PALETTE_COMMANDS`) —
  configuracion, diagnostico, navegacion, exploradores de documentos, modales
  especializados, historial del input.

**Regla de oro**: si el comando opera sobre el hilo de conversacion actual o
inyecta contexto al AI, va en `/`. Si es configuracion, diagnostico,
navegacion o un sub-sistema global, va en `:`.

---

## `/` slash commands (42)

### Sesion — 10
| Comando | Proposito |
|---------|-----------|
| `/clear` | Limpia el historial del chat. |
| `/exit` | Cierra el chat y vuelve al splash. |
| `/resume` | Retoma la ultima conversacion. |
| `/history` | Muestra las sesiones guardadas en el servidor. |
| `/fork <label>` | Ramifica la sesion actual. |
| `/export [path]` | Exporta la sesion como JSONL. |
| `/compact [strategy]` | Compacta mensajes viejos (aggressive/balanced/conservative). |
| `/undo` | Deshace el ultimo turn (pop user+assistant, restaura draft). |
| `/redo` | Rehace el ultimo `/undo`. |
| `/continue` | Reintenta el ultimo turno del AI sin nuevo prompt. |

### Contexto AI — 5
| Comando | Proposito |
|---------|-----------|
| `/diff` | Inyecta el git diff actual como contexto. |
| `/files` | Inyecta archivos modificados recientemente. |
| `/memory` | Muestra uso de tokens del contexto. |
| `/costs` | Costos + tokens de la sesion. |
| `/metrics` | TTFT, OTPS y duracion por turno. |

### Modo chat — 1
| Comando | Proposito |
|---------|-----------|
| `/plan` | Activa/desactiva modo planning. |

### Output AI — 2
| Comando | Proposito |
|---------|-----------|
| `/apply [n]` | Aplica un code block generado por la AI. |
| `/blocks` | Lista los code blocks detectados. |

### Agents / teams / monitores — 11
| Comando | Proposito |
|---------|-----------|
| `/spawn <role> <prompt>` | Lanza un subagent. |
| `/agent-list` · `/agent-cancel <id>` | Administra subagents activos. |
| `/team-start <template> <goal>` · `/team-list` · `/team-cancel <id>` · `/team-mail <id>` | Teams de subagents. |
| `/monitor <cmd>` · `/monitor-list` · `/monitor-kill <id>` · `/monitor-show <id>` | Procesos en background. |

### Todos — 6
| Comando | Proposito |
|---------|-----------|
| `/todos` | Muestra la lista de todos de la sesion. |
| `/todo-add <titulo>` | Agrega un todo. |
| `/todo-start <id>` | Marca en progreso. |
| `/todo-done <id>` | Marca completado. |
| `/todo-remove <id>` | Elimina. |
| `/todo-clear` | Vacia la lista. |

### Memoria persistente — 2
| Comando | Proposito |
|---------|-----------|
| `/remember <type> <file>: <body>` | Guarda memoria (user/feedback/project/reference). |
| `/forget <file>` | Elimina memoria. |

### Workflow — 1
| Comando | Proposito |
|---------|-----------|
| `/workflow <name>` | Carga un workflow ingenierIA. |

### Cron — 3
| Comando | Proposito |
|---------|-----------|
| `/cron-add <notify\|spawn> "<expr>" <args>` | Agrega cron job (requiere args inline). |
| `/cron-list` | Lista jobs con proxima ejecucion. |
| `/cron-remove <id>` | Elimina por id. |

### Meta — 1
| Comando | Proposito |
|---------|-----------|
| `/help` | Muestra ayuda agrupada por bucket. |

---

## `:` palette commands (26)

Se activa con `:` (shift+:) desde splash, dashboard o chat. Fuzzy search.

### Sincronizacion — 1
| id | Proposito |
|----|-----------|
| `sync` | Descarga documentos de todas las factories. |

### Estado / Diagnostico — 4
| id | Proposito |
|----|-----------|
| `health` | Consulta `/api/health`. |
| `doctor` | Diagnostico completo: MCP Server, MCP Tools, Config, **Features**, **LSP**, **IDE Bridge**, Disk. |
| `mcp-status` | Transport, tools descubiertos, lifecycle multi-server. |
| `audit` | Audit log de tools y fallos. |

### Contexto de ingenierIA — 1
| id | Proposito |
|----|-----------|
| `context` | Cicla la factory activa (Net → Ang → Nest → All). |

### Navegacion — 4
| id | Proposito |
|----|-----------|
| `search` | Busqueda fuzzy en todos los documentos. |
| `dashboard` | Abre el dashboard. |
| `home` | Vuelve al splash. |
| `transcript` | Abre/cierra el transcript overlay. |

### Exploradores — 6
| id | Proposito |
|----|-----------|
| `skills` · `commands` · `adrs` · `policies` · `agents` · `workflows` | Abren el `DocPicker` filtrado por tipo. |

### Configuracion — 7
| id | Proposito |
|----|-----------|
| `config` | Abre el wizard (servidor, provider, modelo). |
| `init` | Crea `.ingenieria.json` en el directorio actual. |
| `disconnect` | Elimina token del provider AI y reabre el wizard. |
| `model` | Abre `ModelPickerState` (antes `change-model`). |
| `permissions` | Cicla permisos de tools (Standard → Permissive → Strict). |
| `theme` | Abre `ThemePickerState` con live preview. |
| `plugins` | Lista plugins cargados. |
| `plugins-reload` | Recarga plugins desde disco. |

### Instalacion de stack — 1
| id | Proposito |
|----|-----------|
| `autoskill` | Abre el **modal Autoskill**: escanea el stack, lista skills recomendadas con flag `installed`, permite toggle con Space e install batch con Enter. Reemplaza los viejos `/autoskill` + `/install-skills` slashes. |

### Historial del input — 1
| id | Proposito |
|----|-----------|
| `history-search` | Busca en el historial de inputs anteriores del prompt. |

---

## Migracion: slashes eliminados

Cuando el usuario tipea uno de estos en el chat, el dispatcher responde
`"<slash> se movio a la paleta. Pulsa : y busca <name>."` (ver
`is_migrated_to_palette` en `src/app/slash_commands.rs`).

| Slash viejo | Nuevo acceso |
|-------------|--------------|
| `/theme` `/model` `/permissions` `/doctor` `/audit` `/mcp-status` `/plugins` `/dashboard` `/home` `/init` `/transcript` `/history-search` | `:` palette |
| `/skills` `/commands` `/adrs` `/policies` `/agents` `/workflows` | `:` palette (exploradores) |
| `/sync` `/health` `/search` `/config` `/disconnect` `/context` | `:` palette |

Completamente eliminados (sin reemplazo equivalente):

| Slash | Motivo |
|-------|--------|
| `/compliance` | Nunca usado. |
| `/autoskill` + `/install-skills` | Reemplazados por **modal Autoskill**. |
| `/features` `/hooks` `/lsp-status` `/lsp-diag` `/bridge-status` | Absorbidos por `doctor`. |
| `/go` | Navegacion por URI de baja frecuencia. |
| `/fork-from` | `/fork` cubre el caso comun. |
| `/load` | `/history` + seleccion del panel. |
| `/memories` | Overlap con `/memory`. |
| `/brief` | No usado. |
| `/sessions` · `/retry` · `/todo-check` | Aliases duros. |
| `/detect` | Baja frecuencia (el wizard y `.ingenieria.json` cubren el caso). |

---

## Invariantes

1. Ningun comando vive en ambas superficies.
2. `/` autocomplete se activa solo en chat con slash-prefix; `:` palette
   se activa desde splash/dashboard/chat con el prefijo configurado en
   `Keybindings::command_palette` (default `:`).
3. Los ids de `:` son kebab-case sin `:` leading. Los `/` siempre llevan
   leading slash en `SLASH_COMMANDS`.
4. Agregar un comando nuevo requiere actualizar este archivo + el array
   correspondiente + wiring del dispatcher (slash) o `execute_command` (palette).
5. Tests en `src/state/chat_types.rs` y `src/state/command_state.rs`
   defienden el split (ver `config_commands_removed_from_slash`,
   `chat_only_commands_excluded_from_palette`).
