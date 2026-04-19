//! Escalas perceptuales derivadas desde un color base en Oklch.
//!
//! Genera 12 pasos de luminancia con chroma adaptativo según el modo
//! (dark/light). Los colores resultantes son visualmente equidistantes,
//! garantizando contraste consistente a través de temas.
//!
//! Patrón inspirado en opencode-dev `color.ts:133` (generateScale).

use ratatui::style::Color;

use super::oklch::{srgb_to_oklch, Oklch};
use super::ThemeVariant;

/// Número de pasos en la escala generada.
pub const SCALE_STEPS: usize = 12;

/// Genera una escala de 12 pasos desde `base_hex`.
///
/// El modo (`ThemeVariant`) controla la curva de luminancia:
/// - En `Dark`, los pasos van de oscuro (step 0) a medio-claro (step 11),
///   con chroma ligeramente reducido en los extremos para legibilidad.
/// - En `Light`, la curva empieza más clara y el chroma se reduce menos,
///   porque los acentos sobre fondo claro necesitan más saturación.
/// - `HighContrast` usa curva extrema (0.1..0.95) con chroma conservado.
/// - `Solarized` se alinea con `Dark` por defecto.
///
/// Retorna `[Color::Rgb; SCALE_STEPS]`. Si `base_hex` es inválido, devuelve
/// una escala monocromática gris.
pub fn generate_scale(base_hex: &str, mode: ThemeVariant) -> [Color; SCALE_STEPS] {
    let base = Oklch::from_hex(base_hex).unwrap_or_else(|| srgb_to_oklch(128, 128, 128));
    let curve = lightness_curve(mode);
    let chroma_scale = chroma_scale_for_mode(mode);

    let mut out = [Color::Rgb(0, 0, 0); SCALE_STEPS];
    for (i, (l_target, c_factor)) in curve.iter().zip(chroma_scale.iter()).enumerate() {
        let stepped = Oklch::new(*l_target, base.c * c_factor, base.h);
        let (r, g, b) = stepped.to_rgb();
        out[i] = Color::Rgb(r, g, b);
    }
    out
}

/// Curva de luminancia (12 puntos) para cada modo.
fn lightness_curve(mode: ThemeVariant) -> [f32; SCALE_STEPS] {
    match mode {
        ThemeVariant::HighContrast => {
            [0.10, 0.17, 0.24, 0.32, 0.40, 0.48, 0.56, 0.64, 0.72, 0.81, 0.89, 0.95]
        }
        // Todos los demás temas son dark-style: curva monotónica ascendente.
        _ => [0.18, 0.24, 0.30, 0.37, 0.44, 0.51, 0.58, 0.65, 0.72, 0.79, 0.85, 0.90],
    }
}

/// Multiplicadores de chroma por paso. Los extremos reducen saturación para
/// evitar colores fuera de gamut sRGB o con contraste pobre.
fn chroma_scale_for_mode(mode: ThemeVariant) -> [f32; SCALE_STEPS] {
    match mode {
        ThemeVariant::HighContrast => {
            [0.80, 0.90, 0.95, 1.00, 1.05, 1.05, 1.05, 1.00, 0.95, 0.90, 0.80, 0.70]
        }
        _ => [0.60, 0.75, 0.85, 0.92, 0.98, 1.00, 1.00, 0.98, 0.92, 0.80, 0.65, 0.50],
    }
}

/// Índices semánticos en la escala — útiles para mapear a tokens existentes.
pub mod step {
    /// Borde sutil sobre fondo oscuro.
    pub const BORDER: usize = 3;
    /// Accent primario (uso típico en labels de focus).
    pub const ACCENT: usize = 6;
    /// Accent claro (hover / variantes).
    pub const ACCENT_LIGHT: usize = 8;
    /// Accent texto (foreground sobre fondo oscuro).
    pub const ACCENT_TEXT: usize = 10;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn l_of(c: Color) -> f32 {
        match c {
            Color::Rgb(r, g, b) => srgb_to_oklch(r, g, b).l,
            _ => 0.0,
        }
    }

    #[test]
    fn scale_has_twelve_steps() {
        let scale = generate_scale("#68217A", ThemeVariant::TokyoNight);
        assert_eq!(scale.len(), SCALE_STEPS);
    }

    #[test]
    fn dark_scale_is_monotonically_increasing_in_lightness() {
        let scale = generate_scale("#68217A", ThemeVariant::TokyoNight);
        let lightness: Vec<f32> = scale.iter().map(|c| l_of(*c)).collect();
        for w in lightness.windows(2) {
            assert!(w[0] < w[1], "luminancia no-monótona: {:?}", lightness);
        }
    }

    #[test]
    fn high_contrast_spans_wider_range_than_tokyo_night() {
        let tokyo = generate_scale("#48BB78", ThemeVariant::TokyoNight);
        let hc = generate_scale("#48BB78", ThemeVariant::HighContrast);
        let tokyo_range = l_of(tokyo[SCALE_STEPS - 1]) - l_of(tokyo[0]);
        let hc_range = l_of(hc[SCALE_STEPS - 1]) - l_of(hc[0]);
        assert!(hc_range > tokyo_range);
    }

    #[test]
    fn high_contrast_scale_is_monotonically_increasing_in_lightness() {
        let scale = generate_scale("#68217A", ThemeVariant::HighContrast);
        let lightness: Vec<f32> = scale.iter().map(|c| l_of(*c)).collect();
        for w in lightness.windows(2) {
            assert!(w[0] < w[1], "luminancia no-monótona: {:?}", lightness);
        }
    }

    #[test]
    fn fallback_to_gray_on_invalid_hex() {
        let scale = generate_scale("not-a-color", ThemeVariant::TokyoNight);
        // Todos los colores son grises (r ≈ g ≈ b).
        for c in scale {
            if let Color::Rgb(r, g, b) = c {
                let diff = (r as i32 - g as i32).abs().max((g as i32 - b as i32).abs());
                assert!(diff <= 5, "escala fallback no es gris: {c:?}");
            }
        }
    }

    #[test]
    fn all_outputs_are_rgb_color() {
        let scale = generate_scale("#C82333", ThemeVariant::TokyoNight);
        assert!(scale.iter().all(|c| matches!(c, Color::Rgb(_, _, _))));
    }
}
