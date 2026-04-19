use ratatui::style::Color;

use super::ColorTheme;

/// Tokyo Night — azul noche con acentos púrpura/cyan. Portado desde
/// opencode tokyonight.
pub const TOKYO_NIGHT: ColorTheme = ColorTheme {
    // Surfaces
    bg: Color::Rgb(26, 27, 38),
    bar_bg: Color::Rgb(20, 21, 32),
    surface: Color::Rgb(36, 40, 59),
    border: Color::Rgb(65, 72, 104),

    // Text
    text: Color::Rgb(192, 202, 245),
    text_secondary: Color::Rgb(154, 165, 206),
    text_dim: Color::Rgb(86, 95, 137),
    text_dimmer: Color::Rgb(68, 76, 110),
    text_muted: Color::Rgb(52, 58, 84),
    text_highlight: Color::Rgb(187, 154, 247),

    // Accents
    blue: Color::Rgb(122, 162, 247),
    green: Color::Rgb(158, 206, 106),
    red: Color::Rgb(247, 118, 142),
    yellow: Color::Rgb(224, 175, 104),
    cyan: Color::Rgb(125, 207, 255),
    purple: Color::Rgb(187, 154, 247),
    accent: Color::Rgb(122, 162, 247),

    // Brand
    brand_primary: Color::Rgb(122, 162, 247),
    brand_secondary: Color::Rgb(158, 206, 106),

    // Selection surfaces
    surface_positive: Color::Rgb(30, 48, 40),
    surface_negative: Color::Rgb(56, 36, 52),
    surface_inactive: Color::Rgb(48, 54, 80),
};
