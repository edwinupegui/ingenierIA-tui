//! Configuracion de servers MCP: carga `mcp-servers.json` y construye
//! [`ServerConfig`] validados.
//!
//! Formato:
//! ```json
//! {
//!   "servers": [
//!     {
//!       "name": "ingenieria",
//!       "transport": "sse",
//!       "url": "http://localhost:3001",
//!       "enabled": true
//!     },
//!     {
//!       "name": "fs",
//!       "transport": "stdio",
//!       "command": "npx",
//!       "args": ["@modelcontextprotocol/server-filesystem", "/tmp"]
//!     },
//!     {
//!       "name": "remote",
//!       "transport": "websocket",
//!       "url": "ws://localhost:4000/mcp"
//!     }
//!   ]
//! }
//! ```
//!
//! Defaults: `enabled=true`, `args=[]`. Un server invalido se descarta y genera warning.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Kind de transporte declarado en config. Se mapea 1:1 con
/// [`super::super::transport::TransportKind`] al conectar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConfigTransport {
    Sse,
    Stdio,
    Websocket,
}

/// Config bruta (como viene del JSON). Valida al convertir a [`ServerConfig`].
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServerRaw {
    pub name: String,
    pub transport: ConfigTransport,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub command: Option<String>,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_enabled() -> bool {
    true
}

/// Config validada de un server MCP.
#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub name: String,
    pub enabled: bool,
    pub kind: ServerKind,
}

/// Parametros por tipo de transporte, con datos ya validados.
#[derive(Debug, Clone)]
pub enum ServerKind {
    Sse { url: String },
    Stdio { command: String, args: Vec<String> },
    WebSocket { url: String },
}

impl ServerConfig {
    /// Convierte un raw en config validada. Errores describen por que se descarto.
    pub fn from_raw(raw: ServerRaw) -> anyhow::Result<Self> {
        let name = raw.name.trim();
        if name.is_empty() {
            anyhow::bail!("server sin name");
        }
        if name.contains('/') {
            anyhow::bail!("server name no puede contener '/': {name}");
        }
        let kind = match raw.transport {
            ConfigTransport::Sse => {
                let url = raw
                    .url
                    .ok_or_else(|| anyhow::anyhow!("server '{name}' transport=sse requiere url"))?;
                ServerKind::Sse { url }
            }
            ConfigTransport::Stdio => {
                let command = raw.command.ok_or_else(|| {
                    anyhow::anyhow!("server '{name}' transport=stdio requiere command")
                })?;
                ServerKind::Stdio { command, args: raw.args }
            }
            ConfigTransport::Websocket => {
                let url = raw.url.ok_or_else(|| {
                    anyhow::anyhow!("server '{name}' transport=websocket requiere url")
                })?;
                ServerKind::WebSocket { url }
            }
        };
        Ok(Self { name: name.to_string(), enabled: raw.enabled, kind })
    }
}

/// Raiz del archivo `mcp-servers.json`.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ServersFile {
    #[serde(default)]
    pub servers: Vec<ServerRaw>,
}

/// Carga y valida `mcp-servers.json`. Retorna `(configs, warnings)`.
/// Si no existe, retorna listas vacias (silencioso).
pub fn load_servers() -> (Vec<ServerConfig>, Vec<String>) {
    let Some(path) = servers_config_path() else {
        return (Vec::new(), Vec::new());
    };
    load_from_path(&path)
}

pub(crate) fn load_from_path(path: &std::path::Path) -> (Vec<ServerConfig>, Vec<String>) {
    let Ok(data) = std::fs::read_to_string(path) else {
        return (Vec::new(), Vec::new());
    };
    let parsed: ServersFile = match serde_json::from_str(&data) {
        Ok(p) => p,
        Err(e) => return (Vec::new(), vec![format!("mcp-servers.json invalido: {e}")]),
    };
    let mut configs = Vec::new();
    let mut warnings = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for raw in parsed.servers {
        match ServerConfig::from_raw(raw) {
            Ok(cfg) => {
                if !seen.insert(cfg.name.clone()) {
                    warnings.push(format!("server duplicado ignorado: {}", cfg.name));
                    continue;
                }
                configs.push(cfg);
            }
            Err(e) => warnings.push(e.to_string()),
        }
    }
    (configs, warnings)
}

/// Ruta estandar: `$XDG_CONFIG_HOME/ingenieria-tui/mcp-servers.json`.
pub fn servers_config_path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("ingenieria-tui").join("mcp-servers.json"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_tmp(body: &str) -> tempfile::NamedTempFile {
        use std::io::Write;
        let mut f = tempfile::NamedTempFile::new().unwrap();
        f.write_all(body.as_bytes()).unwrap();
        f
    }

    #[test]
    fn parses_sse_server() {
        let tmp =
            write_tmp(r#"{"servers":[{"name":"ingenieria","transport":"sse","url":"http://x"}]}"#);
        let (cfgs, warns) = load_from_path(tmp.path());
        assert!(warns.is_empty(), "warnings inesperados: {warns:?}");
        assert_eq!(cfgs.len(), 1);
        assert_eq!(cfgs[0].name, "ingenieria");
        assert!(matches!(cfgs[0].kind, ServerKind::Sse { .. }));
        assert!(cfgs[0].enabled);
    }

    #[test]
    fn parses_stdio_with_args() {
        let tmp = write_tmp(
            r#"{"servers":[{"name":"fs","transport":"stdio","command":"npx","args":["a","b"]}]}"#,
        );
        let (cfgs, warns) = load_from_path(tmp.path());
        assert!(warns.is_empty());
        let ServerKind::Stdio { command, args } = &cfgs[0].kind else {
            panic!("expected stdio");
        };
        assert_eq!(command, "npx");
        assert_eq!(args, &["a", "b"]);
    }

    #[test]
    fn parses_websocket() {
        let tmp =
            write_tmp(r#"{"servers":[{"name":"remote","transport":"websocket","url":"ws://x"}]}"#);
        let (cfgs, _) = load_from_path(tmp.path());
        assert!(matches!(cfgs[0].kind, ServerKind::WebSocket { .. }));
    }

    #[test]
    fn sse_without_url_warns() {
        let tmp = write_tmp(r#"{"servers":[{"name":"x","transport":"sse"}]}"#);
        let (cfgs, warns) = load_from_path(tmp.path());
        assert!(cfgs.is_empty());
        assert_eq!(warns.len(), 1);
        assert!(warns[0].contains("requiere url"));
    }

    #[test]
    fn stdio_without_command_warns() {
        let tmp = write_tmp(r#"{"servers":[{"name":"x","transport":"stdio"}]}"#);
        let (cfgs, warns) = load_from_path(tmp.path());
        assert!(cfgs.is_empty());
        assert_eq!(warns.len(), 1);
        assert!(warns[0].contains("requiere command"));
    }

    #[test]
    fn rejects_name_with_slash() {
        let tmp = write_tmp(r#"{"servers":[{"name":"a/b","transport":"sse","url":"http://x"}]}"#);
        let (cfgs, warns) = load_from_path(tmp.path());
        assert!(cfgs.is_empty());
        assert!(warns[0].contains("no puede contener"));
    }

    #[test]
    fn duplicates_are_dropped() {
        let tmp = write_tmp(
            r#"{"servers":[
                {"name":"a","transport":"sse","url":"http://x"},
                {"name":"a","transport":"sse","url":"http://y"}
            ]}"#,
        );
        let (cfgs, warns) = load_from_path(tmp.path());
        assert_eq!(cfgs.len(), 1);
        assert_eq!(warns.len(), 1);
        assert!(warns[0].contains("duplicado"));
    }

    #[test]
    fn missing_file_returns_empty() {
        let path = std::path::Path::new("/tmp/__ingenieria_nope__.json");
        let (cfgs, warns) = load_from_path(path);
        assert!(cfgs.is_empty() && warns.is_empty());
    }

    #[test]
    fn invalid_json_is_reported() {
        let tmp = write_tmp("not json");
        let (cfgs, warns) = load_from_path(tmp.path());
        assert!(cfgs.is_empty());
        assert_eq!(warns.len(), 1);
        assert!(warns[0].contains("invalido"));
    }

    #[test]
    fn disabled_flag_respected() {
        let tmp = write_tmp(
            r#"{"servers":[{"name":"a","transport":"sse","url":"http://x","enabled":false}]}"#,
        );
        let (cfgs, _) = load_from_path(tmp.path());
        assert!(!cfgs[0].enabled);
    }
}
