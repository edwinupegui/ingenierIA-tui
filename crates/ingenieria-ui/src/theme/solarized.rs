use ratatui::style::Color;

use super::ColorTheme;

/// Solarized Dark theme — Ethan Schoonover's solarized palette adapted for TUI.
pub const SOLARIZED: ColorTheme = ColorTheme {
    // Surfaces (solarized base03/base02)
    bg: Color::Rgb(0, 43, 54),
    bar_bg: Color::Rgb(7, 54, 66),
    surface: Color::Rgb(7, 54, 66),
    border: Color::Rgb(88, 110, 117),

    // Text (solarized base0/base1/base01)
    text: Color::Rgb(131, 148, 150),
    text_secondary: Color::Rgb(147, 161, 161),
    text_dim: Color::Rgb(88, 110, 117),
    text_dimmer: Color::Rgb(68, 90, 97),
    text_muted: Color::Rgb(48, 70, 77),
    text_highlight: Color::Rgb(238, 232, 213),

    // Accents (solarized named colors)
    blue: Color::Rgb(38, 139, 210),
    green: Color::Rgb(133, 153, 0),
    red: Color::Rgb(220, 50, 47),
    yellow: Color::Rgb(181, 137, 0),
    cyan: Color::Rgb(42, 161, 152),
    purple: Color::Rgb(108, 113, 196),
    accent: Color::Rgb(38, 139, 210),

    // Brand
    brand_primary: Color::Rgb(38, 139, 210),
    brand_secondary: Color::Rgb(133, 153, 0),

    // Selection surfaces
    surface_positive: Color::Rgb(10, 60, 50),
    surface_negative: Color::Rgb(60, 40, 45),
    surface_inactive: Color::Rgb(20, 55, 65),
};
