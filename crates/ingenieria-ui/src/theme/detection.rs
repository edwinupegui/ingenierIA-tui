use super::ThemeVariant;

/// Auto-detect theme para terminales en modo light.
///
/// El proyecto ya no expone variante "light" de marca propia; si detectamos
/// un terminal con fondo claro se cae al tema `Solarized` (mejor legible sobre
/// fondo claro). En cualquier otro caso se usa el default `TokyoNight`.
pub fn auto_detect_theme() -> ThemeVariant {
    if bg_is_light() {
        ThemeVariant::Solarized
    } else {
        ThemeVariant::TokyoNight
    }
}

fn bg_is_light() -> bool {
    if let Some(light) = detect_from_colorfgbg() {
        return light;
    }
    if let Ok(prog) = std::env::var("TERM_PROGRAM") {
        if prog.eq_ignore_ascii_case("apple_terminal") {
            return true;
        }
    }
    false
}

/// Parse `COLORFGBG="fg;bg"` — bg < 8 suggests light background.
fn detect_from_colorfgbg() -> Option<bool> {
    let val = std::env::var("COLORFGBG").ok()?;
    let bg_str = val.rsplit(';').next()?;
    let bg: u8 = bg_str.parse().ok()?;
    Some(bg < 8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fallback_is_tokyo_night() {
        // Without env vars, detection defaults to TokyoNight
        assert_eq!(auto_detect_theme(), ThemeVariant::TokyoNight);
    }
}
