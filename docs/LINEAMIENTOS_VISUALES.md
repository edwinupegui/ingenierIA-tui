# Lineamientos Visuales de ingenierIA TUI

## Filosofia visual

ingenierIA TUI usa un tema oscuro con acentos de color. La interfaz debe sentirse profesional, limpia y rapida. Cada color tiene un proposito semantico. No se usan colores decorativos.

---

## Theme System

El sistema de themes vive en `crates/ingenieria-ui/src/theme/` y es runtime-switchable.

### Estructura

```
crates/ingenieria-ui/src/theme/
  |-- mod.rs           # ColorTheme (Copy struct), ThemeVariant, thread-local default
  |-- tokyonight.rs    # TOKYO_NIGHT (default)
  |-- solarized.rs     # SOLARIZED
  |-- high_contrast.rs # HIGH_CONTRAST (a11y)
  |-- gruvbox.rs       # GRUVBOX (retro)
  |-- monokai.rs       # MONOKAI
  |-- matrix.rs        # MATRIX (verde sobre negro)
  |-- oklch.rs         # Conversiones sRGB ↔ Oklch
  |-- scale.rs         # Escalas perceptuales (12 pasos) generadas
  +-- detection.rs     # Auto-deteccion terminal claro/oscuro → fallback
```

### ColorTheme

`ColorTheme` es un struct Copy con tokens semanticos. Zero-cost al pasar por valor.

```rust
pub struct ColorTheme {
    // Surfaces
    pub bg: Color,              // Fondo principal
    pub bar_bg: Color,          // Fondo de barras
    pub surface: Color,         // Fondo de paneles, cards
    pub border: Color,          // Bordes

    // Text
    pub text: Color,            // Texto principal
    pub text_secondary: Color,  // Texto secundario
    pub text_dim: Color,        // Hints, placeholders
    pub text_dimmer: Color,     // Texto terciario
    pub text_muted: Color,      // Texto muy tenue
    pub text_highlight: Color,  // Texto resaltado

    // Accents
    pub blue: Color,            // Links, info
    pub green: Color,           // Online, exito
    pub red: Color,             // Offline, error
    pub yellow: Color,          // Warning
    pub cyan: Color,            // Highlights especiales
    pub purple: Color,          // Acento general
    pub accent: Color,          // Acento activo (factory overlay)

    // Brand
    pub brand_primary: Color,
    pub brand_secondary: Color,

    // Selection surfaces
    pub surface_positive: Color,  // Diff added
    pub surface_negative: Color,  // Diff removed
    pub surface_inactive: Color,  // Items inactivos
}
```

### Variantes

| Variante | Descripcion | Constante |
|---------|------------|-----------|
| TokyoNight (default) | Fondo `#1A1B26`, paleta moderna tipo opencode | `theme::TOKYO_NIGHT` |
| Solarized | Paleta Solarized Dark de Ethan Schoonover | `theme::SOLARIZED` |
| HighContrast | Alto contraste para a11y (curva 0.10..0.95) | `theme::HIGH_CONTRAST` |
| Gruvbox | Paleta retro cálida | `theme::GRUVBOX` |
| Monokai | Paleta clásica Monokai | `theme::MONOKAI` |
| Matrix | Verde fosforo sobre negro (niche) | `theme::MATRIX` |

**Seleccion**: modal `ThemePicker` accesible con `:theme` en la paleta de comandos.
Navegacion con `↑/↓` aplica preview en vivo; `Enter` persiste, `Esc` revierte al
original. Implementacion en `src/ui/widgets/theme_picker.rs`.

**Auto-deteccion**: `theme::detection::auto_detect_theme()` lee el background
del terminal al arrancar y elige `Solarized` en terminales claros o `TokyoNight`
en cualquier otro caso (default seguro).

**Backward-compat**: `parse_theme_variant` aliasa `"dark"` → `TokyoNight` para
configs viejas guardadas en disco. Los alias `"light"` y `"rosepine"` caen al
fallback y se reportan como desconocidos.

### Acceso en codigo

```rust
// Obtener tema activo
let colors = state.active_theme.colors();

// Usar tokens semanticos
let style = Style::new().fg(colors.text).bg(colors.bg);
let border = Style::new().fg(colors.border);
let success = Style::new().fg(colors.green);
```

### Factory overlay

`ColorTheme::with_factory()` aplica el color de acento del factory activo:

```rust
let themed = colors.with_factory(&state.factory);
// themed.accent ahora es el color del factory
```

---

## Paleta base (TokyoNight)

Valores del tema default. Cada variante redefine estos hex; las proporciones
(contraste texto/fondo, rol semantico) se mantienen iguales en todas.

### Fondos

| Token | Hex | Uso |
|-------|-----|-----|
| `bg` | `#1A1B26` | Fondo principal de toda la app |
| `surface` | `#24283B` | Fondo de paneles, cards, bloques elevados |

### Texto

| Token | Hex | Uso |
|-------|-----|-----|
| `text` | `#C0CAF5` | Texto principal, titulos, contenido |
| `text_dim` | `#565F89` | Texto secundario, hints, placeholders |
| `text_dimmer` | `#414868` | Texto terciario, bordes inactivos |

### Bordes

| Token | Hex | Uso |
|-------|-----|-----|
| `border` | `#3B4261` | Bordes de paneles, divisores |
| `accent` | Depende del factory | Borde del panel con foco |

### Semanticos

| Token | Hex | Uso |
|-------|-----|-----|
| `green` | `#9ECE6A` | Online, exito, confirmacion |
| `red` | `#F7768E` | Offline, error, desconectado |
| `yellow` | `#E0AF68` | Warning, reconectando |
| `blue` | `#7AA2F7` | Links, informacion |
| `purple` | `#BB9AF7` | Acento general, sin factory |
| `cyan` | `#7DCFFF` | Highlights especiales, tool calls |

---

## Colores por Factory

Cada factory tiene un color de acento que se aplica via `with_factory()`. El color base de la app NO cambia. Solo cambia el `accent`.

### Factory: Net (.NET)

| Token | Hex |
|-------|-----|
| `ACCENT` | `#68217A` |
| `ACCENT_LIGHT` | `#8A3CA0` |
| `ACCENT_TEXT` | `#C8A0DC` |

### Factory: Ang (Angular)

| Token | Hex |
|-------|-----|
| `ACCENT` | `#C82333` |
| `ACCENT_LIGHT` | `#E64650` |
| `ACCENT_TEXT` | `#FFA0A5` |

### Factory: All (Full Stack)

| Token | Hex |
|-------|-----|
| `ACCENT` | `#48BB78` |
| `ACCENT_LIGHT` | `#64D291` |
| `ACCENT_TEXT` | `#B4F0C8` |

### Donde se aplica el color de acento

| Elemento | Sin factory | Con factory |
|---------|------------|------------|
| Tab activo en header | `purple` | `accent` |
| Borde del panel con foco | `blue` | `accent` |
| Titulo del chat | `purple` | `accent` |
| Indicador de factory en header | `text_dim` | `accent` |
| Item seleccionado en sidebar | surface + text | surface + `accent` |
| Barra de scroll activa | `text_dim` | `accent` |
| Prompt del command palette | `blue` | `accent` |

### Lo que NO cambia con el factory

- Fondos (`bg`, `surface`)
- Texto principal (`text`, `text_dim`, `text_dimmer`)
- Colores semanticos (`green`, `red`, `yellow`)
- Bordes inactivos (`border`)
- Colores del markdown (headings, code blocks)

---

## Design System

El design system vive en `crates/ingenieria-ui/src/design_system/`:

| Modulo | Responsabilidad |
|--------|----------------|
| `tokens.rs` | Design tokens (spacing, sizing, typography constants) |
| `dialog.rs` | Componente de dialogo reutilizable |
| `keyboard_hint.rs` | Hint de keybinding con estilo consistente |
| `list_item.rs` | Item de lista estandarizado |
| `pane.rs` | Panel con bordes y titulo |
| `status_icon.rs` | Icono de status (online/offline/warning) |

---

## Accesibilidad

Modulo `crates/ingenieria-ui/src/a11y/`:

| Modulo | Responsabilidad |
|--------|----------------|
| `focus_stack.rs` | Stack de foco para navegacion modal |
| `focus_trap.rs` | Trap de foco dentro de modales |
| `reduced_motion.rs` | Deteccion de preferencia de movimiento reducido |

El tema `HighContrast` maximiza ratios de contraste para usuarios con baja vision.

---

## Tipografia terminal

La TUI no controla la fuente (la elige el emulador de terminal del usuario). Pero si controla:

### Estilos de texto

| Estilo | Uso | Ratatui |
|--------|-----|---------|
| **Bold** | Titulos, headings, items seleccionados | `Style::new().bold()` |
| *Italic* | Metadata, hints, placeholders | `Style::new().italic()` |
| Normal | Contenido, texto de documentos | `Style::new()` |
| ~~Dim~~ | Texto secundario, deshabilitado | `Style::new().fg(colors.text_dim)` |

### Recomendacion de fuente para el usuario

En la documentacion se recomienda usar una Nerd Font para que los iconos se rendericen correctamente. Fuentes sugeridas:

- JetBrains Mono Nerd Font
- FiraCode Nerd Font
- Hack Nerd Font

---

## Layout y espaciado

### Estructura general del Dashboard

```
+--[Header: 3 lineas]------------------------------------------+
|  Status  | Net | Ang | All |     docs: 42    | dev: john     |
+---[Sidebar: 30%]---+---[Preview: 70%]------------------------+
|                     |                                          |
|  > Skills           |  # Skill: auth-flow                     |
|    - auth-flow  <-- |                                          |
|    - payment-api    |  Este skill implementa el flujo de...   |
|  > ADRs             |                                          |
|  > Policies         |  ## Uso                                  |
|  > Workflows        |  ```bash                                 |
|                     |  ingenieria auth --provider oauth          |
|                     |  ```                                     |
|                     |                                          |
+---------------------+------------------------------------------+
|  [Hints: 1 linea]  j/k nav  Enter open  y copy  / search     |
+---------------------------------------------------------------+
```

### Estructura del Chat

```
+--[Header: 3 lineas]------------------------------------------+
|  Chat - ingenierIA-Net  |  Model: gpt-4  |  Status: Ready      |
+--[Messages]-----------------------------------------------+
|                                                            |
|  [user] Como implemento el auth flow?                     |
|                                                            |
|  [assistant] Segun la policy de autenticacion...          |
|  > [read_file] src/auth/handler.rs                        |
|  Basandome en tu codigo, deberias...                      |
|                                                            |
+--[Input: 3-6 lineas]-------------------------------------+
|  > escribe tu mensaje aqui...                             |
+--[Hints: 1 linea]  enter send  esc back  alt+↑↓ scroll  +
+-----------------------------------------------------------+
```

### Reglas de espaciado

| Regla | Valor |
|-------|-------|
| Padding interno de paneles | 1 caracter horizontal, 0 vertical |
| Separacion entre sidebar y preview | 1 caracter (borde compartido) |
| Altura minima del header | 3 lineas |
| Altura minima de hints | 1 linea |
| Ancho minimo del sidebar | 20 caracteres |
| Ancho maximo del sidebar | 35% del terminal |
| Altura del input del chat | 3-6 lineas (crece con contenido) |
| Scroll de preview | Page up/down: media pantalla |

---

## Iconos y simbolos

| Simbolo | Uso | Contexto |
|---------|-----|----------|
| `>` | Categoria expandida en sidebar | Dashboard |
| `v` | Categoria colapsada en sidebar | Dashboard |
| `-` | Item de documento | Sidebar |
| `*` | Item seleccionado | Sidebar |
| `[user]` | Mensaje del usuario | Chat |
| `[assistant]` | Mensaje del AI | Chat |
| `[tool]` | Tool call ejecutado | Chat |
| `...` | Streaming en progreso | Chat |
| `[OK]` | Health check exitoso | Status |
| `[!!]` | Error o desconexion | Status |

Glyphs constantes definidos en `theme/mod.rs`: `GLYPH_SUCCESS`, `GLYPH_ERROR`, `GLYPH_PENDING`, `GLYPH_HEART`, etc.

---

## Markdown en terminal

El renderizado de Markdown sigue estas reglas:

| Elemento MD | Color | Estilo |
|-------------|-------|--------|
| `# H1` | `blue` | Bold |
| `## H2` | `cyan` | Bold |
| `### H3` | `purple` | Bold |
| `**bold**` | `text` | Bold |
| `*italic*` | `text` | Italic |
| `` `inline code` `` | `yellow` | Normal, fondo `surface` |
| Code block | `text` | Normal, fondo `surface`, borde `border` |
| `- list item` | `text` | Prefijo `text_dim` |
| `> blockquote` | `text_dim` | Italic, borde izquierdo `border` |
| `[link](url)` | `blue` | Underline |

Pipeline: `ui/markdown/` con token cache, thinking collapse, fence normalizer, y streaming incremental (E29).

---

## Animaciones y transiciones

La TUI usa animaciones minimas para no distraer:

| Animacion | Implementacion | Duracion |
|-----------|---------------|----------|
| Spinner de carga | Tick cada 250ms, caracteres rotativos | Mientras dure la operacion |
| Splash logo | Renderizado inmediato | Estatico |
| Notificacion | Aparece, se mantiene 3s, desaparece | 3 segundos |
| Scroll suave | No hay, es discreto por lineas | Inmediato |
| Onboarding checklist | Auto-dismiss con countdown (E39) | Configurable |

---

## Principios de consistencia visual

1. **Un color, un significado**: Verde siempre es exito/online. Rojo siempre es error/offline. No mezclar.

2. **El acento del factory es acento, no fondo**: Los colores de factory se usan en bordes, titulos y highlights. Nunca como fondo de paneles completos.

3. **Contraste minimo 4.5:1**: Todo texto sobre fondo debe ser legible. `text` sobre `bg` tiene contraste ~12:1. `text_dim` sobre `bg` tiene ~3:1 (solo para hints).

4. **Bordes como estructura, no como decoracion**: Los bordes definen zonas. No se usan bordes dobles, sombras ni bordes decorativos.

5. **Menos es mas**: Si un elemento no necesita color, es `text` o `text_dim`. Los colores semanticos se reservan para estados y acciones.

6. **Usar siempre tokens semanticos**: NUNCA literales `Color::Rgb(...)` fuera del theme system. Acceso via `state.active_theme.colors()`.
