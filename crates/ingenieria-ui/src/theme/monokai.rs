use ratatui::style::Color;

use super::ColorTheme;

/// Monokai — paleta clásica high-saturation verde/magenta. Portado desde
/// opencode monokai.
pub const MONOKAI: ColorTheme = ColorTheme {
    // Surfaces
    bg: Color::Rgb(39, 40, 34),
    bar_bg: Color::Rgb(30, 31, 26),
    surface: Color::Rgb(62, 61, 50),
    border: Color::Rgb(73, 72, 62),

    // Text
    text: Color::Rgb(248, 248, 242),
    text_secondary: Color::Rgb(196, 196, 192),
    text_dim: Color::Rgb(117, 113, 94),
    text_dimmer: Color::Rgb(90, 88, 74),
    text_muted: Color::Rgb(70, 68, 58),
    text_highlight: Color::Rgb(253, 151, 31),

    // Accents
    blue: Color::Rgb(102, 217, 239),
    green: Color::Rgb(166, 226, 46),
    red: Color::Rgb(249, 38, 114),
    yellow: Color::Rgb(253, 151, 31),
    cyan: Color::Rgb(102, 217, 239),
    purple: Color::Rgb(174, 129, 255),
    accent: Color::Rgb(249, 38, 114),

    // Brand
    brand_primary: Color::Rgb(102, 217, 239),
    brand_secondary: Color::Rgb(166, 226, 46),

    // Selection surfaces
    surface_positive: Color::Rgb(45, 62, 28),
    surface_negative: Color::Rgb(80, 26, 56),
    surface_inactive: Color::Rgb(58, 56, 46),
};
