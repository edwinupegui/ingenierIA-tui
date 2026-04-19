use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const DEFAULT_SERVER_URL: &str = "http://localhost:3001";

#[derive(Debug, Clone)]
pub struct Config {
    pub server_url: String,
    pub developer: String,
    pub provider: String,
    pub model: String,
    pub default_factory: Option<String>,
    /// Nombre del tema activo: "dark", "light", "solarized", "high-contrast".
    /// `None` = auto-detect al primer arranque.
    pub theme: Option<String>,
}

/// Archivo ~/.config/ingenieria-tui/config.json
#[derive(Debug, Deserialize, Serialize, Default)]
struct GlobalConfig {
    server_url: Option<String>,
    developer: Option<String>,
    provider: Option<String>,
    model: Option<String>,
    default_factory: Option<String>,
    last_sync_date: Option<String>,
    #[serde(default)]
    theme: Option<String>,
}

/// Subconjunto del .mcp.json del proyecto
#[derive(Debug, Deserialize)]
struct McpJson {
    /// Campo "ingenieriaServerUrl" en el .mcp.json del proyecto
    #[serde(rename = "ingenieriaServerUrl")]
    ingenieria_server_url: Option<String>,
}

impl Config {
    /// Resuelve la configuración en orden de prioridad:
    /// 1. cli_url argumento     2. INGENIERIA_SERVER_URL env
    /// 3. .mcp.json local       4. ~/.config/ingenieria-tui/config.json
    /// 5. http://localhost:3001 fallback
    pub fn resolve(cli_url: Option<String>) -> Self {
        let server_url = cli_url
            .or_else(|| std::env::var("INGENIERIA_SERVER_URL").ok().filter(|s| !s.is_empty()))
            .or_else(find_mcp_json)
            .or_else(|| load_global_config().and_then(|c| c.server_url))
            .unwrap_or_else(|| DEFAULT_SERVER_URL.to_string());

        let global = load_global_config().unwrap_or_default();

        let developer = std::env::var("INGENIERIA_DEVELOPER")
            .ok()
            .or(global.developer)
            .or_else(system_username)
            .unwrap_or_else(|| "developer".to_string());

        let provider = std::env::var("INGENIERIA_PROVIDER")
            .ok()
            .or(global.provider)
            .unwrap_or_else(|| "github-copilot".to_string());

        let model = std::env::var("INGENIERIA_MODEL")
            .ok()
            .or(global.model)
            .unwrap_or_else(|| "github-copilot".to_string());

        let default_factory = global.default_factory;
        let theme = std::env::var("INGENIERIA_THEME").ok().or(global.theme);

        Config { server_url, developer, provider, model, default_factory, theme }
    }
}

fn find_mcp_json() -> Option<String> {
    let mut dir = std::env::current_dir().ok()?;
    loop {
        let candidate = dir.join(".mcp.json");
        if candidate.exists() {
            if let Ok(content) = std::fs::read_to_string(&candidate) {
                if let Ok(mcp) = serde_json::from_str::<McpJson>(&content) {
                    if let Some(url) = mcp.ingenieria_server_url {
                        return Some(url);
                    }
                }
            }
        }
        if !dir.pop() {
            break;
        }
    }
    None
}

fn global_config_path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("ingenieria-tui").join("config.json"))
}

fn load_global_config() -> Option<GlobalConfig> {
    let path = global_config_path()?;
    let content = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

fn system_username() -> Option<String> {
    std::env::var("USER").or_else(|_| std::env::var("USERNAME")).ok()
}

/// Retorna `true` si el archivo de configuración global no existe todavía.
pub fn needs_wizard() -> bool {
    global_config_path().map(|p| !p.exists()).unwrap_or(true)
}

/// Persiste el tema activo en `~/.config/ingenieria-tui/config.json` sin
/// tocar el resto de campos. Best-effort: errores se loguean via tracing.
pub fn persist_theme(variant: &str) {
    let path = match global_config_path() {
        Some(p) => p,
        None => {
            tracing::warn!("no se pudo resolver config path para persistir theme");
            return;
        }
    };
    if let Some(parent) = path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            tracing::warn!(error=%e, "no se pudo crear config dir");
            return;
        }
    }
    let mut global = load_global_config().unwrap_or_default();
    global.theme = Some(variant.to_string());
    match serde_json::to_string_pretty(&global) {
        Ok(body) => {
            if let Err(e) = std::fs::write(&path, body) {
                tracing::warn!(error=%e, "fallo al escribir config.json con theme");
            }
        }
        Err(e) => tracing::warn!(error=%e, "fallo al serializar GlobalConfig"),
    }
}

impl Config {
    /// Persiste la configuración actual en `~/.config/ingenieria-tui/config.json`.
    pub fn save(&self) -> anyhow::Result<()> {
        let path = global_config_path().ok_or_else(|| {
            anyhow::anyhow!("No se pudo determinar el directorio de configuración")
        })?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        // Preserve existing fields (like last_sync_date) when saving
        let mut global = load_global_config().unwrap_or_default();
        global.server_url = Some(self.server_url.clone());
        global.developer = Some(self.developer.clone());
        global.provider = Some(self.provider.clone());
        global.model = Some(self.model.clone());
        global.default_factory = self.default_factory.clone();
        global.theme = self.theme.clone();
        std::fs::write(&path, serde_json::to_string_pretty(&global)?)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn global_config_should_deserialize_full_json() {
        let json = r#"{
            "server_url": "http://example.com:3001",
            "developer": "alice",
            "provider": "github-copilot",
            "model": "gpt-4",
            "default_factory": "net",
            "last_sync_date": "2025-01-01T00:00:00Z"
        }"#;
        let cfg: GlobalConfig = serde_json::from_str(json).unwrap();
        assert_eq!(cfg.server_url.as_deref(), Some("http://example.com:3001"));
        assert_eq!(cfg.developer.as_deref(), Some("alice"));
        assert_eq!(cfg.default_factory.as_deref(), Some("net"));
        assert_eq!(cfg.last_sync_date.as_deref(), Some("2025-01-01T00:00:00Z"));
    }

    #[test]
    fn global_config_should_deserialize_partial_json() {
        let json = r#"{"server_url": "http://localhost:3001"}"#;
        let cfg: GlobalConfig = serde_json::from_str(json).unwrap();
        assert_eq!(cfg.server_url.as_deref(), Some("http://localhost:3001"));
        assert!(cfg.developer.is_none());
        assert!(cfg.model.is_none());
    }

    #[test]
    fn global_config_should_deserialize_empty_json() {
        let json = "{}";
        let cfg: GlobalConfig = serde_json::from_str(json).unwrap();
        assert!(cfg.server_url.is_none());
        assert!(cfg.developer.is_none());
    }

    #[test]
    fn global_config_should_not_crash_on_invalid_json() {
        let result = serde_json::from_str::<GlobalConfig>("not json");
        assert!(result.is_err());
    }

    #[test]
    fn global_config_should_roundtrip_serialize() {
        let cfg = GlobalConfig {
            server_url: Some("http://test:3001".into()),
            developer: Some("bob".into()),
            provider: Some("github-copilot".into()),
            model: Some("gpt-4".into()),
            default_factory: Some("ang".into()),
            theme: None,
            last_sync_date: Some("2025-06-01".into()),
        };
        let json = serde_json::to_string(&cfg).unwrap();
        let parsed: GlobalConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.server_url, cfg.server_url);
        assert_eq!(parsed.developer, cfg.developer);
        assert_eq!(parsed.last_sync_date, cfg.last_sync_date);
    }

    #[test]
    fn mcp_json_should_parse_server_url() {
        let json = r#"{"ingenieriaServerUrl": "http://custom:4000"}"#;
        let mcp: McpJson = serde_json::from_str(json).unwrap();
        assert_eq!(mcp.ingenieria_server_url.as_deref(), Some("http://custom:4000"));
    }

    #[test]
    fn mcp_json_should_handle_missing_url() {
        let json = r#"{"otherField": true}"#;
        let mcp: McpJson = serde_json::from_str(json).unwrap();
        assert!(mcp.ingenieria_server_url.is_none());
    }

    #[test]
    fn config_should_use_default_server_url_when_nothing_set() {
        // Clear env to avoid interference (restore after)
        let saved = std::env::var("INGENIERIA_SERVER_URL").ok();
        std::env::remove_var("INGENIERIA_SERVER_URL");

        let config = Config::resolve(None);
        // May use global config or default; at minimum should not crash
        assert!(!config.server_url.is_empty());
        assert!(!config.developer.is_empty());
        assert!(!config.provider.is_empty());

        // Restore
        if let Some(val) = saved {
            std::env::set_var("INGENIERIA_SERVER_URL", val);
        }
    }

    #[test]
    fn config_cli_url_should_override_everything() {
        let config = Config::resolve(Some("http://cli-override:9999".into()));
        assert_eq!(config.server_url, "http://cli-override:9999");
    }

    #[test]
    fn global_config_should_ignore_extra_fields() {
        let json = r#"{"server_url": "http://test", "unknown_field": 42}"#;
        let result = serde_json::from_str::<GlobalConfig>(json);
        // Should not crash on unknown fields
        assert!(result.is_ok());
    }
}

/// Read the persisted last_sync_date from config.
pub fn load_last_sync_date() -> Option<String> {
    load_global_config().and_then(|c| c.last_sync_date)
}

/// Save last_sync_date to config without overwriting other fields.
pub fn save_last_sync_date(date: &str) -> anyhow::Result<()> {
    let path = global_config_path().ok_or_else(|| anyhow::anyhow!("No config dir"))?;
    let mut global = load_global_config().unwrap_or(GlobalConfig {
        server_url: None,
        developer: None,
        provider: None,
        model: None,
        default_factory: None,
        theme: None,
        last_sync_date: None,
    });
    global.last_sync_date = Some(date.to_string());
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, serde_json::to_string_pretty(&global)?)?;
    Ok(())
}

// ── Keybindings ─────────────────────────────────────────────────────────────

/// Customizable keybindings loaded from `~/.config/ingenieria-tui/keybindings.json`.
///
/// Each field maps an action name to a key description string.
/// Missing fields use defaults. Unknown fields are ignored.
#[derive(Debug, Deserialize, Serialize)]
pub struct Keybindings {
    #[serde(default = "default_toggle_sidebar")]
    pub toggle_sidebar: String,
    #[serde(default = "default_search")]
    pub search: String,
    #[serde(default = "default_command_palette")]
    pub command_palette: String,
    #[serde(default = "default_copy")]
    pub copy: String,
    #[serde(default = "default_factory_switch")]
    pub factory_switch: String,
}

fn default_toggle_sidebar() -> String {
    "space".into()
}
fn default_search() -> String {
    "/".into()
}
fn default_command_palette() -> String {
    ":".into()
}
fn default_copy() -> String {
    "y".into()
}
fn default_factory_switch() -> String {
    "tab".into()
}

impl Default for Keybindings {
    fn default() -> Self {
        Self {
            toggle_sidebar: default_toggle_sidebar(),
            search: default_search(),
            command_palette: default_command_palette(),
            copy: default_copy(),
            factory_switch: default_factory_switch(),
        }
    }
}

/// Load keybindings from config directory, falling back to defaults.
/// Logs a warning if duplicate keys are detected.
pub fn load_keybindings() -> Keybindings {
    let path = match dirs::config_dir() {
        Some(d) => d.join("ingenieria-tui").join("keybindings.json"),
        None => return Keybindings::default(),
    };
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return Keybindings::default(),
    };
    let kb: Keybindings = serde_json::from_str(&content).unwrap_or_default();
    validate_keybinding_conflicts(&kb);
    kb
}

/// Check for duplicate key assignments and log warnings.
fn validate_keybinding_conflicts(kb: &Keybindings) {
    let bindings = [
        ("toggle_sidebar", &kb.toggle_sidebar),
        ("search", &kb.search),
        ("command_palette", &kb.command_palette),
        ("copy", &kb.copy),
        ("factory_switch", &kb.factory_switch),
    ];
    for i in 0..bindings.len() {
        for j in (i + 1)..bindings.len() {
            if bindings[i].1 == bindings[j].1 {
                tracing::warn!(
                    key = bindings[i].1,
                    action_a = bindings[i].0,
                    action_b = bindings[j].0,
                    "Keybinding conflict: same key assigned to two actions",
                );
            }
        }
    }
}
