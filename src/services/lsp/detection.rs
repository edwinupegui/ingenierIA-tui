//! LSP server detection (E25).
//!
//! Detecta automaticamente que language server usar basandose en los
//! archivos del proyecto. Busca en CWD por marcadores conocidos y verifica
//! que el binario exista en PATH.

use std::path::Path;

/// Configuracion minima para arrancar un language server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LspServerConfig {
    /// Nombre legible (e.g. "rust-analyzer").
    pub name: &'static str,
    /// Comando/binario a ejecutar.
    pub command: &'static str,
    /// Argumentos CLI para modo stdio.
    pub args: &'static [&'static str],
    /// Language ID (LSP spec) para didOpen.
    pub language_id: &'static str,
    /// Extensiones de archivo relevantes.
    pub extensions: &'static [&'static str],
}

/// Configuraciones conocidas. Orden = prioridad de deteccion.
const KNOWN_SERVERS: &[LspServerConfig] = &[
    LspServerConfig {
        name: "rust-analyzer",
        command: "rust-analyzer",
        args: &[],
        language_id: "rust",
        extensions: &["rs"],
    },
    LspServerConfig {
        name: "typescript-language-server",
        command: "typescript-language-server",
        args: &["--stdio"],
        language_id: "typescript",
        extensions: &["ts", "tsx", "js", "jsx"],
    },
    LspServerConfig {
        name: "omnisharp",
        command: "omnisharp",
        args: &["-lsp", "--stdio"],
        language_id: "csharp",
        extensions: &["cs"],
    },
];

/// Marcadores de archivo que identifican cada language server.
const MARKERS: &[(&str, &str)] = &[
    ("Cargo.toml", "rust-analyzer"),
    ("rust-toolchain.toml", "rust-analyzer"),
    ("package.json", "typescript-language-server"),
    ("tsconfig.json", "typescript-language-server"),
    ("*.csproj", "omnisharp"),
    ("*.sln", "omnisharp"),
];

/// Detecta el language server mas adecuado para el directorio `cwd`.
/// Retorna `None` si no hay marcador reconocido o el binario no esta en PATH.
pub fn detect(cwd: &Path) -> Option<&'static LspServerConfig> {
    for (marker, server_name) in MARKERS {
        if marker_exists(cwd, marker) {
            let config = KNOWN_SERVERS.iter().find(|s| s.name == *server_name)?;
            if binary_in_path(config.command) {
                return Some(config);
            }
        }
    }
    None
}

fn marker_exists(cwd: &Path, marker: &str) -> bool {
    if marker.starts_with('*') {
        // Glob pattern: busca cualquier archivo con esa extension.
        let ext = marker.trim_start_matches("*.");
        if let Ok(entries) = std::fs::read_dir(cwd) {
            return entries
                .flatten()
                .any(|e| e.path().extension().and_then(|x| x.to_str()) == Some(ext));
        }
        false
    } else {
        cwd.join(marker).exists()
    }
}

fn binary_in_path(name: &str) -> bool {
    std::process::Command::new("which")
        .arg(name)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marker_exists_finds_cargo_toml() {
        // El propio proyecto tiene Cargo.toml.
        let cwd = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        assert!(marker_exists(&cwd, "Cargo.toml"));
    }

    #[test]
    fn marker_exists_glob_finds_toml() {
        let cwd = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        assert!(marker_exists(&cwd, "*.toml"));
    }

    #[test]
    fn marker_exists_returns_false_for_missing() {
        let cwd = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        assert!(!marker_exists(&cwd, "nope.xyz"));
    }

    #[test]
    fn known_servers_has_entries() {
        assert!(!KNOWN_SERVERS.is_empty());
    }

    #[test]
    fn detect_returns_some_for_this_rust_project() {
        let cwd = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        // Solo si rust-analyzer esta instalado; skip if not.
        if binary_in_path("rust-analyzer") {
            let config = detect(&cwd);
            assert!(config.is_some());
            assert_eq!(config.unwrap().name, "rust-analyzer");
        }
    }

    #[test]
    fn detect_returns_none_in_empty_dir() {
        let tmp = std::env::temp_dir().join("ingenieria-lsp-test-empty");
        let _ = std::fs::create_dir_all(&tmp);
        assert!(detect(&tmp).is_none());
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
