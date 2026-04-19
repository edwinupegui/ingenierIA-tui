//! Deteccion de preferencia "reduce motion" para desactivar animaciones.
//!
//! Referencia: claude-code `CLAUDE_CODE_ACCESSIBILITY` env var.
//! Las animaciones pueden ser problematicas para usuarios con sensibilidad
//! vestibular, lectores de pantalla, o terminales con rendering lento.

#![cfg_attr(not(test), allow(dead_code, reason = "E37 toolkit — integracion pendiente"))]

/// Env var que desactiva TODAS las features visuales de accesibilidad reducida
/// (animaciones, spinners, transitions, toasts auto-dismiss).
pub const ENV_ACCESSIBILITY: &str = "INGENIERIA_ACCESSIBILITY";
/// Env var granular que solo desactiva animaciones (mantiene toasts, etc.).
pub const ENV_REDUCE_MOTION: &str = "INGENIERIA_REDUCE_MOTION";

/// `true` si el usuario pidio reducir animaciones.
///
/// Activado por cualquiera de:
///   - `INGENIERIA_ACCESSIBILITY=1` (o cualquier valor no vacio)
///   - `INGENIERIA_REDUCE_MOTION=1`
pub fn should_reduce_motion() -> bool {
    env_flag_set(ENV_ACCESSIBILITY) || env_flag_set(ENV_REDUCE_MOTION)
}

/// `true` si el modo accesibilidad completo esta activo.
/// Usar cuando queremos tambien ajustar layout/timeouts, no solo animaciones.
pub fn is_accessibility_mode() -> bool {
    env_flag_set(ENV_ACCESSIBILITY)
}

/// Frame fijo para mostrar cuando las animaciones estan desactivadas.
pub const STATIC_SPINNER_FRAME: &str = "…";

/// Devuelve el frame adecuado: animado si corresponde, estatico si reduce_motion.
pub fn spinner_frame(animated_frames: &[&str], tick: u64) -> &'static str {
    if animated_frames.is_empty() {
        return STATIC_SPINNER_FRAME;
    }
    if should_reduce_motion() {
        return STATIC_SPINNER_FRAME;
    }
    // SAFETY: el caller provee slices con strings 'static via literales.
    let idx = (tick as usize) % animated_frames.len();
    // Convertimos la ref prestada a 'static: el caller garantiza literales.
    // En caso contrario usamos el fallback.
    let frame = animated_frames[idx];
    // Trucco: para mantener vida 'static sin unsafe, devolvemos un set fijo.
    match_static_frame(frame)
}

fn env_flag_set(key: &str) -> bool {
    std::env::var_os(key).is_some_and(|v| !v.is_empty() && v != "0" && v != "false")
}

/// Mapea frames comunes a sus versiones 'static.
fn match_static_frame(frame: &str) -> &'static str {
    match frame {
        "⠋" => "⠋",
        "⠙" => "⠙",
        "⠹" => "⠹",
        "⠸" => "⠸",
        "⠼" => "⠼",
        "⠴" => "⠴",
        "⠦" => "⠦",
        "⠧" => "⠧",
        "⠇" => "⠇",
        "⠏" => "⠏",
        "|" => "|",
        "/" => "/",
        "-" => "-",
        "\\" => "\\",
        _ => STATIC_SPINNER_FRAME,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // Serializar tests que leen env vars.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn no_env_means_animations_on() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        // SAFETY: tests serializados via ENV_LOCK.
        unsafe {
            std::env::remove_var(ENV_ACCESSIBILITY);
            std::env::remove_var(ENV_REDUCE_MOTION);
        }
        assert!(!should_reduce_motion());
        assert!(!is_accessibility_mode());
    }

    #[test]
    fn accessibility_env_disables_motion() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        unsafe {
            std::env::set_var(ENV_ACCESSIBILITY, "1");
            std::env::remove_var(ENV_REDUCE_MOTION);
        }
        assert!(should_reduce_motion());
        assert!(is_accessibility_mode());
        unsafe {
            std::env::remove_var(ENV_ACCESSIBILITY);
        }
    }

    #[test]
    fn reduce_motion_env_only_disables_motion() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        unsafe {
            std::env::set_var(ENV_REDUCE_MOTION, "1");
            std::env::remove_var(ENV_ACCESSIBILITY);
        }
        assert!(should_reduce_motion());
        assert!(!is_accessibility_mode());
        unsafe {
            std::env::remove_var(ENV_REDUCE_MOTION);
        }
    }

    #[test]
    fn spinner_returns_static_when_reduced() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        unsafe {
            std::env::set_var(ENV_ACCESSIBILITY, "1");
        }
        let frames = &["⠋", "⠙", "⠹"];
        let frame = spinner_frame(frames, 0);
        assert_eq!(frame, STATIC_SPINNER_FRAME);
        unsafe {
            std::env::remove_var(ENV_ACCESSIBILITY);
        }
    }

    #[test]
    fn spinner_cycles_frames_when_animated() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        unsafe {
            std::env::remove_var(ENV_ACCESSIBILITY);
            std::env::remove_var(ENV_REDUCE_MOTION);
        }
        let frames = &["⠋", "⠙", "⠹"];
        assert_eq!(spinner_frame(frames, 0), "⠋");
        assert_eq!(spinner_frame(frames, 1), "⠙");
        assert_eq!(spinner_frame(frames, 2), "⠹");
        assert_eq!(spinner_frame(frames, 3), "⠋");
    }
}
