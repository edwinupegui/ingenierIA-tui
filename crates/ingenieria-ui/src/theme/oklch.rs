//! Conversores Oklch ⇄ sRGB.
//!
//! Oklch es una representación cilíndrica perceptualmente uniforme del
//! espacio Oklab (Björn Ottosson, 2020). Permite generar escalas donde
//! cada paso es visualmente equidistante — imposible con HSL/HSV.
//!
//! Uso principal: [`generate_scale`](super::scale::generate_scale) para
//! derivar tonos light/dim desde un color base de factory.
//!
//! Referencias:
//! - <https://bottosson.github.io/posts/oklab/>
//! - opencode-dev `packages/ui/src/theme/color.ts:49-170`
//!
//! Implementación manual (sin `palette` crate) para no inflar deps.
#![allow(clippy::excessive_precision)]

/// Representación Oklch de un color.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Oklch {
    /// Lightness perceptual (0.0 .. 1.0).
    pub l: f32,
    /// Croma (saturación perceptual). No tiene tope fijo; valores típicos
    /// llegan hasta ~0.4 para colores puros.
    pub c: f32,
    /// Hue en radianes (0 .. 2π).
    pub h: f32,
}

impl Oklch {
    pub fn new(l: f32, c: f32, h: f32) -> Self {
        Self { l, c, h }
    }

    /// Parsea un color desde `#rrggbb`. Retorna `None` si el string no es un
    /// hex válido.
    pub fn from_hex(hex: &str) -> Option<Self> {
        let (r, g, b) = parse_hex(hex)?;
        Some(srgb_to_oklch(r, g, b))
    }

    /// Convierte a sRGB `(r, g, b)` con componentes en `[0, 255]`.
    pub fn to_rgb(self) -> (u8, u8, u8) {
        oklch_to_srgb(self)
    }
}

fn parse_hex(s: &str) -> Option<(u8, u8, u8)> {
    let s = s.trim_start_matches('#');
    if s.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&s[0..2], 16).ok()?;
    let g = u8::from_str_radix(&s[2..4], 16).ok()?;
    let b = u8::from_str_radix(&s[4..6], 16).ok()?;
    Some((r, g, b))
}

/// sRGB (0..255) → Oklch.
pub fn srgb_to_oklch(r: u8, g: u8, b: u8) -> Oklch {
    let r_lin = srgb_to_linear(r as f32 / 255.0);
    let g_lin = srgb_to_linear(g as f32 / 255.0);
    let b_lin = srgb_to_linear(b as f32 / 255.0);

    let l = 0.412_221_47 * r_lin + 0.536_332_55 * g_lin + 0.051_445_995 * b_lin;
    let m = 0.211_903_5 * r_lin + 0.680_699_56 * g_lin + 0.107_396_96 * b_lin;
    let s = 0.088_302_46 * r_lin + 0.281_718_85 * g_lin + 0.629_978_7 * b_lin;

    let l_ = l.cbrt();
    let m_ = m.cbrt();
    let s_ = s.cbrt();

    let l_out = 0.210_454_26 * l_ + 0.793_617_8 * m_ - 0.004_072_047 * s_;
    let a_out = 1.977_998_5 * l_ - 2.428_592_2 * m_ + 0.450_593_7 * s_;
    let b_out = 0.025_904_037 * l_ + 0.782_771_77 * m_ - 0.808_675_77 * s_;

    let c = (a_out * a_out + b_out * b_out).sqrt();
    let h = b_out.atan2(a_out);
    Oklch::new(l_out, c, h)
}

/// Oklch → sRGB (0..255), con clamp a gamut sRGB.
pub fn oklch_to_srgb(color: Oklch) -> (u8, u8, u8) {
    let a = color.c * color.h.cos();
    let b = color.c * color.h.sin();

    let l_ = color.l + 0.396_337_78 * a + 0.215_803_76 * b;
    let m_ = color.l - 0.105_561_35 * a - 0.063_854_17 * b;
    let s_ = color.l - 0.089_484_18 * a - 1.291_485_5 * b;

    let l = l_ * l_ * l_;
    let m = m_ * m_ * m_;
    let s = s_ * s_ * s_;

    let r_lin = 4.076_741_7 * l - 3.307_711_6 * m + 0.230_969_94 * s;
    let g_lin = -1.268_438 * l + 2.609_757_4 * m - 0.341_319_4 * s;
    let b_lin = -0.004_196_086_3 * l - 0.703_418_6 * m + 1.707_614_7 * s;

    let r = linear_to_srgb(r_lin.clamp(0.0, 1.0));
    let g = linear_to_srgb(g_lin.clamp(0.0, 1.0));
    let b = linear_to_srgb(b_lin.clamp(0.0, 1.0));

    ((r * 255.0).round() as u8, (g * 255.0).round() as u8, (b * 255.0).round() as u8)
}

/// sRGB gamma → linear.
fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.040_45 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// Linear → sRGB gamma.
fn linear_to_srgb(c: f32) -> f32 {
    if c <= 0.003_130_8 {
        12.92 * c
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn within(a: u8, b: u8, tol: i32) -> bool {
        (a as i32 - b as i32).abs() <= tol
    }

    #[test]
    fn parses_hex_with_and_without_hash() {
        assert!(Oklch::from_hex("#68217A").is_some());
        assert!(Oklch::from_hex("68217A").is_some());
        assert!(Oklch::from_hex("abc").is_none());
        assert!(Oklch::from_hex("").is_none());
    }

    #[test]
    fn roundtrip_preserves_pure_red() {
        let (r, g, b) = Oklch::from_hex("#FF0000").unwrap().to_rgb();
        assert!(within(r, 255, 2));
        assert!(within(g, 0, 2));
        assert!(within(b, 0, 2));
    }

    #[test]
    fn roundtrip_preserves_pure_green() {
        let (r, g, b) = Oklch::from_hex("#00FF00").unwrap().to_rgb();
        assert!(within(r, 0, 2));
        assert!(within(g, 255, 2));
        assert!(within(b, 0, 2));
    }

    #[test]
    fn roundtrip_preserves_ingenieria_net_purple() {
        let (r, g, b) = Oklch::from_hex("#68217A").unwrap().to_rgb();
        assert!(within(r, 0x68, 2));
        assert!(within(g, 0x21, 2));
        assert!(within(b, 0x7A, 2));
    }

    #[test]
    fn roundtrip_preserves_white_and_black() {
        let (wr, wg, wb) = Oklch::from_hex("#FFFFFF").unwrap().to_rgb();
        assert_eq!((wr, wg, wb), (255, 255, 255));
        let (br, bg, bb) = Oklch::from_hex("#000000").unwrap().to_rgb();
        assert_eq!((br, bg, bb), (0, 0, 0));
    }

    #[test]
    fn pure_colors_have_measurable_chroma() {
        let red = Oklch::from_hex("#FF0000").unwrap();
        let purple = Oklch::from_hex("#68217A").unwrap();
        assert!(red.c > 0.1);
        assert!(purple.c > 0.05);
    }

    #[test]
    fn lightness_ordering_matches_intuition() {
        let black = Oklch::from_hex("#000000").unwrap();
        let gray = Oklch::from_hex("#808080").unwrap();
        let white = Oklch::from_hex("#FFFFFF").unwrap();
        assert!(black.l < gray.l);
        assert!(gray.l < white.l);
    }
}
