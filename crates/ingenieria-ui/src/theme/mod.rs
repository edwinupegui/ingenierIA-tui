//! Theme system — runtime-switchable color themes with factory overlay.
//!
//! Provides `ColorTheme` (Copy struct with semantic tokens), 4 built-in themes,
//! auto-detection, and backward-compatible constants for existing code.

pub mod detection;
mod gruvbox;
mod high_contrast;
mod matrix;
mod monokai;
pub mod oklch;
pub mod scale;
mod solarized;
mod tokyonight;

use ratatui::style::Color;

pub use gruvbox::GRUVBOX;
pub use high_contrast::HIGH_CONTRAST;
pub use matrix::MATRIX;
pub use monokai::MONOKAI;
pub use solarized::SOLARIZED;
pub use tokyonight::TOKYO_NIGHT;

use ingenieria_domain::factory::UiFactory;

// ── ColorTheme ──────────────────────────────────────────────────────────────

/// Semantic color theme. Copy type for zero-cost passing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColorTheme {
    // Surfaces
    pub bg: Color,
    pub bar_bg: Color,
    pub surface: Color,
    pub border: Color,

    // Text
    pub text: Color,
    pub text_secondary: Color,
    pub text_dim: Color,
    pub text_dimmer: Color,
    pub text_muted: Color,
    pub text_highlight: Color,

    // Accents
    pub blue: Color,
    pub green: Color,
    pub red: Color,
    pub yellow: Color,
    pub cyan: Color,
    pub purple: Color,
    pub accent: Color,

    // Brand
    pub brand_primary: Color,
    pub brand_secondary: Color,

    // Selection surfaces
    pub surface_positive: Color,
    pub surface_negative: Color,
    pub surface_inactive: Color,
}

impl ColorTheme {
    /// Apply factory accent color as overlay on this theme.
    ///
    /// Usa el color base del factory como ancla y sobrescribe `accent`
    /// directamente. Para escalas perceptuales (variantes light/dim del
    /// acento) usar [`Self::with_factory_scaled`].
    pub fn with_factory(self, factory: &UiFactory) -> Self {
        let (r, g, b) = factory.color();
        Self { accent: Color::Rgb(r, g, b), ..self }
    }

    /// Variante de `with_factory` que deriva `accent` desde una escala Oklch
    /// de 12 pasos. El `mode` controla la curva de luminancia/chroma. Ver
    /// [`scale::generate_scale`].
    ///
    /// Útil cuando el caller quiere garantizar mejor contraste en temas
    /// Light/HighContrast.
    pub fn with_factory_scaled(self, factory: &UiFactory, mode: ThemeVariant) -> Self {
        let (r, g, b) = factory.color();
        let hex = format!("#{:02X}{:02X}{:02X}", r, g, b);
        let scale = scale::generate_scale(&hex, mode);
        Self { accent: scale[scale::step::ACCENT], ..self }
    }
}

// ── ThemeVariant ─────────────────────────────────────────────────────────────

/// Available theme variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThemeVariant {
    #[default]
    TokyoNight,
    Solarized,
    HighContrast,
    Gruvbox,
    Monokai,
    Matrix,
}

impl ThemeVariant {
    /// Lista ordenada usada por el picker y por `next()`.
    pub const ALL: &'static [ThemeVariant] = &[
        ThemeVariant::TokyoNight,
        ThemeVariant::Solarized,
        ThemeVariant::HighContrast,
        ThemeVariant::Gruvbox,
        ThemeVariant::Monokai,
        ThemeVariant::Matrix,
    ];

    /// Cycle to next theme variant.
    pub fn next(self) -> Self {
        let idx = Self::ALL.iter().position(|v| *v == self).unwrap_or(0);
        Self::ALL[(idx + 1) % Self::ALL.len()]
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::TokyoNight => "Tokyo Night",
            Self::Solarized => "Solarized",
            Self::HighContrast => "High Contrast",
            Self::Gruvbox => "Gruvbox",
            Self::Monokai => "Monokai",
            Self::Matrix => "Matrix",
        }
    }

    /// Slug estable (lowercase, ascii) para persistencia y matching de entrada.
    pub fn slug(self) -> &'static str {
        match self {
            Self::TokyoNight => "tokyonight",
            Self::Solarized => "solarized",
            Self::HighContrast => "high-contrast",
            Self::Gruvbox => "gruvbox",
            Self::Monokai => "monokai",
            Self::Matrix => "matrix",
        }
    }

    /// Resolve to the concrete `ColorTheme`.
    pub fn colors(self) -> ColorTheme {
        match self {
            Self::TokyoNight => TOKYO_NIGHT,
            Self::Solarized => SOLARIZED,
            Self::HighContrast => HIGH_CONTRAST,
            Self::Gruvbox => GRUVBOX,
            Self::Monokai => MONOKAI,
            Self::Matrix => MATRIX,
        }
    }
}

// ── Active theme (thread-local) ─────────────────────────────────────────────
//
// Las funciones `bg()`, `border()`, `blue()` etc. resuelven al tema activo
// del frame en curso. `set_active_theme` debe invocarse al inicio de cada
// render top-level (dashboard, chat, wizard, splash, init) con
// `state.active_theme.colors()`. Fallback inicial: TOKYO_NIGHT.

use std::cell::Cell;

thread_local! {
    static ACTIVE_COLORS: Cell<ColorTheme> = const { Cell::new(TOKYO_NIGHT) };
}

/// Actualiza el tema activo del thread actual. Llamar una vez por frame
/// en el entrypoint de render. Barato — no alloca.
pub fn set_active_theme(theme: ColorTheme) {
    ACTIVE_COLORS.with(|c| c.set(theme));
}

/// Devuelve el tema activo del thread actual.
pub fn active_theme() -> ColorTheme {
    ACTIVE_COLORS.with(|c| c.get())
}

// Acceso a tokens individuales. Cada fn lee del thread-local. Usados
// extensivamente por widgets; marcadas `#[inline]` para evitar overhead.

#[inline]
pub fn bg() -> Color {
    active_theme().bg
}
#[inline]
pub fn bar_bg() -> Color {
    active_theme().bar_bg
}
#[inline]
pub fn surface() -> Color {
    active_theme().surface
}
#[inline]
pub fn border() -> Color {
    active_theme().border
}

#[inline]
pub fn white() -> Color {
    active_theme().text
}
#[inline]
pub fn gray() -> Color {
    active_theme().text_secondary
}
#[inline]
pub fn dim() -> Color {
    active_theme().text_dim
}
#[inline]
pub fn dimmer() -> Color {
    active_theme().text_dimmer
}
#[inline]
pub fn muted() -> Color {
    active_theme().text_muted
}
#[inline]
pub fn highlight() -> Color {
    active_theme().text_highlight
}

#[inline]
pub fn blue() -> Color {
    active_theme().blue
}
#[inline]
pub fn green() -> Color {
    active_theme().green
}
#[inline]
pub fn red() -> Color {
    active_theme().red
}
#[inline]
pub fn yellow() -> Color {
    active_theme().yellow
}
#[inline]
pub fn cyan() -> Color {
    active_theme().cyan
}
#[inline]
pub fn purple() -> Color {
    active_theme().purple
}
#[inline]
pub fn accent() -> Color {
    active_theme().accent
}

#[inline]
pub fn brand_blue() -> Color {
    active_theme().brand_primary
}
#[inline]
pub fn brand_green() -> Color {
    active_theme().brand_secondary
}

#[inline]
pub fn surface_green() -> Color {
    active_theme().surface_positive
}
#[inline]
pub fn surface_purple() -> Color {
    active_theme().surface_negative
}
#[inline]
pub fn step_inactive() -> Color {
    active_theme().surface_inactive
}

// ── Glyphs — Estados ────────────────────────────────────────────────────────

pub const GLYPH_SUCCESS: &str = "✓";
pub const GLYPH_ERROR: &str = "✗";
pub const GLYPH_PENDING: &str = "●";
pub const GLYPH_TOOL_BLOCK: &str = "■";
pub const GLYPH_TOOL_PENDING: &str = "□";
pub const GLYPH_IDLE: &str = "○";
pub const GLYPH_CHECKING: &str = "◌";
pub const GLYPH_THINKING: &str = "∴";
pub const GLYPH_WARNING: &str = "⚠";
pub const GLYPH_TOOL: &str = "⚡";
pub const GLYPH_HEART: &str = "♥";

// ── Glyphs — Navegacion ─────────────────────────────────────────────────────

pub const GLYPH_CURSOR: &str = "›";
pub const GLYPH_EXPANDED: &str = "▼";
pub const GLYPH_COLLAPSED: &str = "▶";
pub const GLYPH_TREE_RESULT: &str = "↳";

// ── Glyphs — Contenido ──────────────────────────────────────────────────────

pub const GLYPH_BULLET: &str = "•";
pub const GLYPH_BULLET_NESTED: &str = "◦";
pub const GLYPH_CURSOR_BLOCK: &str = "█";
pub const GLYPH_ACCENT_BAR: &str = "▌";

// ── Glyphs — Spinners ───────────────────────────────────────────────────────

pub const SPINNERS: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

// ── Color component helpers ─────────────────────────────────────────────────

/// Extract the red component from an RGB color (fallback: 0).
pub fn color_r(c: Color) -> u8 {
    if let Color::Rgb(r, _, _) = c {
        r
    } else {
        0
    }
}

/// Extract the green component from an RGB color (fallback: 0).
pub fn color_g(c: Color) -> u8 {
    if let Color::Rgb(_, g, _) = c {
        g
    } else {
        0
    }
}

/// Extract the blue component from an RGB color (fallback: 0).
pub fn color_b(c: Color) -> u8 {
    if let Color::Rgb(_, _, b) = c {
        b
    } else {
        0
    }
}

/// Linear interpolation between two u8 values.
pub fn lerp_u8(a: u8, b: u8, t: f32) -> u8 {
    (a as f32 + (b as f32 - a as f32) * t).round() as u8
}

/// Convierte el color representativo de un `UiFactory` a `Color`.
///
/// Centralizado aquí porque `Color::Rgb` solo puede usarse dentro de `theme/`.
pub fn factory_color(factory: &UiFactory) -> Color {
    let (r, g, b) = factory.color();
    Color::Rgb(r, g, b)
}

/// Build a gradient Color from two colors at factor t (0.0 = a, 1.0 = b).
pub fn lerp_color(a: Color, b: Color, t: f32) -> Color {
    Color::Rgb(
        lerp_u8(color_r(a), color_r(b), t),
        lerp_u8(color_g(a), color_g(b), t),
        lerp_u8(color_b(a), color_b(b), t),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokyo_night_theme_tokens_resolve_when_active() {
        set_active_theme(TOKYO_NIGHT);
        assert_eq!(bg(), TOKYO_NIGHT.bg);
        assert_eq!(white(), TOKYO_NIGHT.text);
        assert_eq!(green(), TOKYO_NIGHT.green);
    }

    #[test]
    fn gruvbox_theme_tokens_resolve_when_active() {
        set_active_theme(GRUVBOX);
        assert_eq!(bg(), GRUVBOX.bg);
        assert_eq!(white(), GRUVBOX.text);
        assert_ne!(bg(), TOKYO_NIGHT.bg);
    }

    #[test]
    fn theme_variant_cycles_through_all() {
        let first = ThemeVariant::default();
        let mut v = first;
        for _ in 0..ThemeVariant::ALL.len() {
            v = v.next();
        }
        assert_eq!(v, first, "cycle must return to start after ALL.len() steps");
    }

    #[test]
    fn theme_colors_resolves() {
        let colors = ThemeVariant::TokyoNight.colors();
        assert_eq!(colors.bg, TOKYO_NIGHT.bg);
        let gruv = ThemeVariant::Gruvbox.colors();
        assert_ne!(gruv.bg, TOKYO_NIGHT.bg);
    }

    #[test]
    fn factory_overlay_changes_accent() {
        let base = TOKYO_NIGHT;
        let overlaid = base.with_factory(&UiFactory::Net);
        assert_ne!(overlaid.accent, base.accent);
        // Other colors unchanged
        assert_eq!(overlaid.bg, base.bg);
    }

    #[test]
    fn color_theme_is_copy() {
        let a = TOKYO_NIGHT;
        let b = a; // Copy, not move
        assert_eq!(a, b);
    }

    #[test]
    fn default_theme_is_tokyo_night() {
        assert_eq!(ThemeVariant::default(), ThemeVariant::TokyoNight);
    }
}
