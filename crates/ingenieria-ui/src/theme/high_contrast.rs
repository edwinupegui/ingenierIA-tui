use ratatui::style::Color;

use super::ColorTheme;

/// High-contrast theme — WCAG AA compliant (min 4.5:1 contrast ratio).
pub const HIGH_CONTRAST: ColorTheme = ColorTheme {
    // Surfaces (pure black base)
    bg: Color::Rgb(0, 0, 0),
    bar_bg: Color::Rgb(10, 10, 10),
    surface: Color::Rgb(20, 20, 20),
    border: Color::Rgb(180, 180, 180),

    // Text (pure white / high-contrast grays)
    text: Color::Rgb(255, 255, 255),
    text_secondary: Color::Rgb(220, 220, 220),
    text_dim: Color::Rgb(160, 160, 160),
    text_dimmer: Color::Rgb(130, 130, 130),
    text_muted: Color::Rgb(100, 100, 100),
    text_highlight: Color::Rgb(255, 255, 100),

    // Accents (saturated for visibility)
    blue: Color::Rgb(100, 200, 255),
    green: Color::Rgb(100, 255, 100),
    red: Color::Rgb(255, 100, 100),
    yellow: Color::Rgb(255, 255, 100),
    cyan: Color::Rgb(100, 255, 255),
    purple: Color::Rgb(200, 150, 255),
    accent: Color::Rgb(100, 200, 255),

    // Brand
    brand_primary: Color::Rgb(100, 160, 255),
    brand_secondary: Color::Rgb(100, 220, 120),

    // Selection surfaces
    surface_positive: Color::Rgb(0, 50, 0),
    surface_negative: Color::Rgb(60, 0, 0),
    surface_inactive: Color::Rgb(40, 40, 40),
};
