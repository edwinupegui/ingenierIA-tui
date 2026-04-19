//! Deteccion de terminal (E39).
//!
//! Detecta el emulador de terminal desde variables de entorno estandar. El
//! resultado se usa para:
//! - Habilitar OSC 8 hyperlinks en iTerm2 / kitty / alacritty / wezterm.
//! - Decidir si se usa el keyboard protocol extendido (kitty, ghostty).
//! - Sugerir `set -g mouse off` cuando tmux esta activo (scroll nativo no
//!   funciona correctamente con mouse capture).
//!
//! La deteccion es best-effort: un terminal desconocido cae en `Unknown` y
//! usamos los defaults mas conservadores.

/// Clasificacion del terminal actual. No exhaustiva.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Terminal {
    ITerm2,
    Kitty,
    Ghostty,
    Alacritty,
    WezTerm,
    VsCode,
    Apple, // Terminal.app
    GenericXterm,
    Unknown,
}

/// Snapshot de deteccion de plataforma calculado una sola vez al startup.
#[derive(Debug, Clone, Copy)]
pub struct PlatformHints {
    pub terminal: Terminal,
    pub inside_tmux: bool,
    pub inside_ssh: bool,
    pub supports_hyperlinks: bool,
    /// Kitty keyboard protocol (reportado por kitty y ghostty). Guardado para
    /// cuando E37/E40 ajusten la deteccion de modificadores extendidos.
    #[allow(dead_code, reason = "E39 — disponible para consumidores futuros del protocolo")]
    pub supports_kitty_keyboard: bool,
}

impl PlatformHints {
    /// Deteccion actual leyendo `std::env`. Equivale a `detect(&env_snapshot())`.
    pub fn detect() -> Self {
        let term_program = std::env::var("TERM_PROGRAM").unwrap_or_default();
        let term = std::env::var("TERM").unwrap_or_default();
        let inside_tmux =
            !std::env::var("TMUX").unwrap_or_default().is_empty() || term.starts_with("tmux");
        let inside_ssh = !std::env::var("SSH_CLIENT").unwrap_or_default().is_empty()
            || !std::env::var("SSH_TTY").unwrap_or_default().is_empty();
        let terminal = classify(&term_program, &term);
        PlatformHints {
            terminal,
            inside_tmux,
            inside_ssh,
            supports_hyperlinks: terminal_supports_hyperlinks(terminal, inside_tmux),
            supports_kitty_keyboard: matches!(terminal, Terminal::Kitty | Terminal::Ghostty),
        }
    }

    /// Hint textual para mostrar en el status bar o log al arranque. `None` si
    /// no hay nada notable que decir del entorno.
    pub fn summary(&self) -> Option<String> {
        let term_label = terminal_label(self.terminal);
        let mut extras: Vec<&'static str> = Vec::new();
        if self.inside_tmux {
            extras.push("tmux");
        }
        if self.inside_ssh {
            extras.push("ssh");
        }
        if self.supports_hyperlinks {
            extras.push("hyperlinks");
        }
        if term_label.is_none() && extras.is_empty() {
            return None;
        }
        let mut parts: Vec<String> = Vec::new();
        if let Some(label) = term_label {
            parts.push(label.to_string());
        }
        if !extras.is_empty() {
            parts.push(extras.join(","));
        }
        Some(parts.join(" · "))
    }
}

fn classify(term_program: &str, term: &str) -> Terminal {
    match term_program {
        "iTerm.app" => Terminal::ITerm2,
        "Apple_Terminal" => Terminal::Apple,
        "vscode" => Terminal::VsCode,
        "WezTerm" => Terminal::WezTerm,
        "ghostty" => Terminal::Ghostty,
        _ => classify_by_term_var(term),
    }
}

fn classify_by_term_var(term: &str) -> Terminal {
    if term.contains("kitty") {
        Terminal::Kitty
    } else if term.contains("alacritty") {
        Terminal::Alacritty
    } else if term.contains("ghostty") {
        Terminal::Ghostty
    } else if term.contains("xterm") {
        Terminal::GenericXterm
    } else {
        Terminal::Unknown
    }
}

fn terminal_label(t: Terminal) -> Option<&'static str> {
    match t {
        Terminal::ITerm2 => Some("iTerm2"),
        Terminal::Kitty => Some("kitty"),
        Terminal::Ghostty => Some("ghostty"),
        Terminal::Alacritty => Some("alacritty"),
        Terminal::WezTerm => Some("wezterm"),
        Terminal::VsCode => Some("vscode"),
        Terminal::Apple => Some("Terminal.app"),
        Terminal::GenericXterm => Some("xterm"),
        Terminal::Unknown => None,
    }
}

/// Hyperlinks OSC 8 son soportados por iTerm2, kitty, alacritty reciente,
/// wezterm, ghostty. VsCode tambien los soporta. tmux los pasa si el outer
/// terminal los soporta, pero nosotros somos conservadores y asumimos que si
/// estamos en tmux puede haber issues y los deshabilitamos.
fn terminal_supports_hyperlinks(t: Terminal, inside_tmux: bool) -> bool {
    if inside_tmux {
        return false;
    }
    matches!(
        t,
        Terminal::ITerm2
            | Terminal::Kitty
            | Terminal::Ghostty
            | Terminal::Alacritty
            | Terminal::WezTerm
            | Terminal::VsCode
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_detects_iterm() {
        assert_eq!(classify("iTerm.app", ""), Terminal::ITerm2);
    }

    #[test]
    fn classify_detects_kitty_via_term() {
        assert_eq!(classify("", "xterm-kitty"), Terminal::Kitty);
    }

    #[test]
    fn classify_falls_back_to_unknown() {
        assert_eq!(classify("", "weird-term"), Terminal::Unknown);
    }

    #[test]
    fn hyperlinks_disabled_inside_tmux() {
        assert!(!terminal_supports_hyperlinks(Terminal::ITerm2, true));
    }

    #[test]
    fn hyperlinks_enabled_for_modern_terminals() {
        assert!(terminal_supports_hyperlinks(Terminal::Kitty, false));
        assert!(terminal_supports_hyperlinks(Terminal::WezTerm, false));
    }

    #[test]
    fn summary_empty_when_unknown_and_clean() {
        let h = PlatformHints {
            terminal: Terminal::Unknown,
            inside_tmux: false,
            inside_ssh: false,
            supports_hyperlinks: false,
            supports_kitty_keyboard: false,
        };
        assert!(h.summary().is_none());
    }

    #[test]
    fn summary_includes_tmux_ssh_flags() {
        let h = PlatformHints {
            terminal: Terminal::Kitty,
            inside_tmux: true,
            inside_ssh: true,
            supports_hyperlinks: false,
            supports_kitty_keyboard: true,
        };
        let s = h.summary().unwrap();
        assert!(s.contains("kitty"));
        assert!(s.contains("tmux"));
        assert!(s.contains("ssh"));
    }
}
