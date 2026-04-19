use ratatui::style::Color;

use super::ColorTheme;

/// Matrix — verde neón monocromático con acento púrpura. Portado desde
/// opencode matrix.
pub const MATRIX: ColorTheme = ColorTheme {
    // Surfaces
    bg: Color::Rgb(10, 14, 10),
    bar_bg: Color::Rgb(5, 8, 5),
    surface: Color::Rgb(18, 24, 18),
    border: Color::Rgb(30, 42, 31),

    // Text
    text: Color::Rgb(98, 255, 148),
    text_secondary: Color::Rgb(120, 210, 140),
    text_dim: Color::Rgb(140, 163, 145),
    text_dimmer: Color::Rgb(90, 120, 95),
    text_muted: Color::Rgb(50, 72, 55),
    text_highlight: Color::Rgb(230, 255, 87),

    // Accents
    blue: Color::Rgb(48, 179, 255),
    green: Color::Rgb(46, 255, 106),
    red: Color::Rgb(255, 75, 75),
    yellow: Color::Rgb(230, 255, 87),
    cyan: Color::Rgb(36, 246, 217),
    purple: Color::Rgb(199, 112, 255),
    accent: Color::Rgb(46, 255, 106),

    // Brand
    brand_primary: Color::Rgb(46, 255, 106),
    brand_secondary: Color::Rgb(36, 246, 217),

    // Selection surfaces
    surface_positive: Color::Rgb(15, 50, 22),
    surface_negative: Color::Rgb(60, 15, 15),
    surface_inactive: Color::Rgb(25, 35, 25),
};
