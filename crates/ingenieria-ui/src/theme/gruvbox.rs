use ratatui::style::Color;

use super::ColorTheme;

/// Gruvbox Dark — paleta retro cálida (marrón/mostaza). Portada desde
/// opencode gruvbox-dark con los 11 tokens canónicos mapeados a los 22
/// slots de `ColorTheme`.
pub const GRUVBOX: ColorTheme = ColorTheme {
    // Surfaces
    bg: Color::Rgb(40, 40, 40),
    bar_bg: Color::Rgb(30, 30, 30),
    surface: Color::Rgb(60, 56, 54),
    border: Color::Rgb(80, 73, 69),

    // Text
    text: Color::Rgb(235, 219, 178),
    text_secondary: Color::Rgb(189, 174, 147),
    text_dim: Color::Rgb(146, 131, 116),
    text_dimmer: Color::Rgb(112, 101, 86),
    text_muted: Color::Rgb(80, 73, 69),
    text_highlight: Color::Rgb(250, 189, 47),

    // Accents
    blue: Color::Rgb(131, 165, 152),
    green: Color::Rgb(184, 187, 38),
    red: Color::Rgb(251, 73, 52),
    yellow: Color::Rgb(250, 189, 47),
    cyan: Color::Rgb(142, 192, 124),
    purple: Color::Rgb(211, 134, 155),
    accent: Color::Rgb(131, 165, 152),

    // Brand
    brand_primary: Color::Rgb(131, 165, 152),
    brand_secondary: Color::Rgb(184, 187, 38),

    // Selection surfaces
    surface_positive: Color::Rgb(50, 60, 34),
    surface_negative: Color::Rgb(70, 30, 26),
    surface_inactive: Color::Rgb(68, 62, 58),
};
