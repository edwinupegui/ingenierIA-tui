# Keybindings — atajos de teclado

Fuente canonica de los atajos. Diseno guiado por un principio: **evitar
colisiones con emuladores de terminal populares** (VSCode terminal, Zed,
Windows Terminal, etc). Los `Ctrl+X` se reducen al minimo indispensable;
todo lo demas se accede por `/` (chat) o `:` (palette).

Implementacion: `src/workers/keyboard.rs` mapea las teclas a variantes de
`Action`; los handlers viven en `src/app/keys.rs` y sub-modulos.

---

## Globales (cualquier pantalla)

| Tecla | Action | Notas |
|-------|--------|-------|
| `Ctrl+C` | Abort / quit (double-tap) | Primera pulsacion: arma el quit (2s, notifica "Ctrl+C otra vez para salir"). Si hay stream activo, lo aborta. Si el input tiene texto, lo limpia. Segunda pulsacion dentro de la ventana: sale. Ver `src/app/quit_handler.rs`. |
| `Esc` | Cierra overlay / vuelve atras | Se propaga top-down: elicitation → monitor panel → transcript → history-search → command palette → picker activo → screen-specific. |
| `Enter` | Accion primaria | Contexto-dependiente: envia chat, ejecuta palette, selecciona item, etc. |
| `Tab` | Cicla foco / factory | En dashboard cicla factories; en chat completa el slash autocomplete. |
| `:` | Abre palette (command mode) | Requiere input vacio + chat en Ready + sin doc picker. Configurable en `Keybindings::command_palette`. |
| `/` | Abre slash autocomplete | Solo en chat. Activa el popup de slashes. |

### Navegacion

| Tecla | Uso |
|-------|-----|
| `↑/↓` | Navega listas, cursor en input, historia del chat. |
| `PageUp/PageDown` | Scroll por paginas. |
| `Home/End` | Saltar al principio/final de la linea o lista. |

### Edicion de input

| Tecla | Uso |
|-------|-----|
| `Backspace/Delete` | Borrar caracter. |
| `Ctrl+Z` / `Ctrl+Y` | Undo/redo del input (stack dedicado, ver `state::input_undo`). |
| `Alt+Enter` / `Shift+Enter` | Newline literal en el input multilinea. |

### Chat (scroll + navegacion de turnos)

| Tecla | Uso |
|-------|-----|
| `Ctrl+↑` / `Ctrl+↓` | Salta entre `ChatRole::User` messages (nav cursor). |
| `Alt+↑` / `Alt+↓` | Scroll linea a linea del preview/chat. |
| `Shift+↑` / `Shift+↓` | Scroll por bloques grandes. |

---

## Double Ctrl+C — escalation

Patron inspirado en opencode-dev (`QUIT_SHORTCUT_TIMEOUT`). Escalacion:

```
1ra Ctrl+C:
  ├─ si chat streaming/executing → abort (no arma quit)
  ├─ si input no vacio → clear input
  └─ caso contrario → arma quit por 8 ticks (~2s @ 4Hz), notifica

2da Ctrl+C dentro de la ventana: sale (return true al event loop).

2da Ctrl+C fuera de la ventana: re-arma, sigue sin salir.
```

Estado: `AppState::quit_armed_until: Option<u64>` (tick-based, sin `Instant`
para simplicidad de test determinista).

---

## Pickers modales

Los pickers comparten el patron `↑↓ navegar · enter confirmar · esc cancelar`.
El comportamiento especifico de cada modal:

### `ThemePicker` (`:theme`)

| Tecla | Accion |
|-------|--------|
| `↑/↓` | Mueve cursor y **aplica preview en vivo** (sin persistir). |
| `<char>` | Agrega a `query` (fuzzy filter por slug/label). |
| `Backspace` | Borra de `query`. |
| `Enter` | Persiste el theme seleccionado + invalida caches markdown. |
| `Esc` | Revierte al theme original (el que estaba activo al abrir). |

### `ModelPicker` (`:model`)

| Tecla | Accion |
|-------|--------|
| `↑/↓` | Mueve cursor. |
| `Enter` | Aplica el modelo al estado. |
| `Esc` | Cancela sin cambio. |

### `AutoskillPicker` (`:autoskill`, feature `autoskill`)

| Tecla | Accion |
|-------|--------|
| `↑/↓` | Mueve cursor entre skills recomendadas. |
| `Space` | Toggle seleccion del item actual (no-op si `installed`). |
| `Enter` | Lanza install batch de las seleccionadas + cierra modal. |
| `Esc` | Cierra sin instalar. |

---

## Dashboard

| Tecla | Accion |
|-------|--------|
| `j/k` o `↑/↓` | Navega sidebar. |
| `Enter` | Abre el documento en preview. |
| `Space` | Expande/colapsa categoria. |
| `y` | Copia contenido al clipboard (arboard + fallback OSC 52). |
| `Y` | Copia `/<nombre>` del skill actual. |
| `T` | Toggle tool monitor. |
| `H` | Toggle enforcement dashboard. |
| `K` | Toggle agents panel. |
| `N` | Toggle notifications panel. |
| `S` | Escaneo autoskill + notify (atajo debug; uso recomendado: `:autoskill`). |

---

## Chat

| Tecla | Accion |
|-------|--------|
| `Enter` | Envia el mensaje. |
| `Esc` | Vuelve al dashboard (si no hay overlay abierto). |
| `/` | Abre slash autocomplete. |
| `:` | Abre command palette. |
| `↑/↓` en input vacio | Cicla input history. |
| `Ctrl+↑/↓` | Nav user messages. |

---

## Wizard

| Tecla | Accion |
|-------|--------|
| `↑/↓` | Navega entre opciones del step actual. |
| `Tab` | Avanza al siguiente step validado. |
| `Enter` | Confirma el step / finaliza wizard. |
| `Esc` | Vuelve al step anterior / cancela. |

---

## Teclas eliminadas (no usar en documentacion nueva)

Estos atajos fueron podados del build actual. Referencias en docs viejas
deben sustituirse por `/` o `:` segun corresponda:

| Atajo viejo | Reemplazo |
|-------------|-----------|
| `Ctrl+D` | `:dashboard` |
| `Ctrl+E` | `:home` o `/exit` |
| `Ctrl+L` | `/clear` |
| `Ctrl+N` | `:init` |
| `Ctrl+T` | `:theme` (modal) |
| `Ctrl+O` | `:transcript` |
| `Ctrl+F` | Dentro del transcript modal: tecla `/` local. |
| `Ctrl+R` | `:history-search` |

---

## Hints en UI

Los hints renderizados en la base de cada screen leen directamente del
estado (no son strings hardcodeados) via `src/ui/widgets/hints.rs`.
Cuando cambia el set de atajos, los hints se regeneran automaticamente —
no hace falta tocar tres sitios.

El armor state del double Ctrl+C se muestra como span amarillo:
`ctrl+c otra vez para salir`.
