//! Carga y parseo de `hooks.json` (ubicado en `$XDG_CONFIG_HOME/ingenieria-tui/`).
//!
//! Formato:
//! ```json
//! {
//!   "hooks": [
//!     {
//!       "name": "audit-bash",
//!       "trigger": "PreToolUse",
//!       "match": { "tool": "Bash*" },
//!       "command": "echo [$INGENIERIA_TOOL_NAME] >> /tmp/audit.log",
//!       "timeout_secs": 5,
//!       "on_failure": "warn"
//!     }
//!   ]
//! }
//! ```
//!
//! Defaults: `timeout_secs=5`, `on_failure="warn"`, `match.tool` ausente → aplica a todas.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::types::HookTrigger;

/// Politica ante fallos de un hook. `Warn` y `Ignore` no afectan flujo;
/// `Block` reservado para futuro enforcement en hooks Pre*.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum HookFailurePolicy {
    #[default]
    Warn,
    Ignore,
    #[allow(dead_code, reason = "reservada para futuro bloqueo sincrono en PreToolUse")]
    Block,
}

/// Matcher de un hook. Solo soporta `tool` (glob simple) por ahora.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HookMatch {
    #[serde(default)]
    pub tool: Option<String>,
}

/// Entrada raw del archivo JSON. Se convierte a [`HookDef`] tras validar.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HookRaw {
    pub name: String,
    pub trigger: String,
    #[serde(rename = "match", default)]
    pub match_: HookMatch,
    pub command: String,
    #[serde(default = "default_timeout_secs")]
    pub timeout_secs: u32,
    #[serde(default)]
    pub on_failure: HookFailurePolicy,
}

fn default_timeout_secs() -> u32 {
    5
}

/// Hook validado (trigger parseado). Lo que usa el runner en caliente.
#[derive(Debug, Clone)]
pub struct HookDef {
    pub name: String,
    pub trigger: HookTrigger,
    pub match_tool: Option<String>,
    pub command: String,
    pub timeout_secs: u32,
    pub on_failure: HookFailurePolicy,
}

impl HookDef {
    fn from_raw(raw: HookRaw) -> Result<Self, String> {
        let trigger = HookTrigger::from_label(&raw.trigger).ok_or_else(|| {
            format!("trigger desconocido en hook '{}': {}", raw.name, raw.trigger)
        })?;
        if raw.command.trim().is_empty() {
            return Err(format!("hook '{}' tiene command vacio", raw.name));
        }
        let timeout = if raw.timeout_secs == 0 { 5 } else { raw.timeout_secs.min(300) };
        Ok(Self {
            name: raw.name,
            trigger,
            match_tool: raw.match_.tool,
            command: raw.command,
            timeout_secs: timeout,
            on_failure: raw.on_failure,
        })
    }
}

/// Raiz del archivo `hooks.json`.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct HooksFile {
    #[serde(default)]
    pub hooks: Vec<HookRaw>,
}

/// Carga y valida `hooks.json`. Retorna `(defs, warnings)`.
/// Errores no-fatales (hook individual invalido) se acumulan en warnings.
/// Si no existe el archivo, retorna lista vacia sin warning.
pub fn load_hooks() -> (Vec<HookDef>, Vec<String>) {
    let Some(path) = hooks_config_path() else {
        return (Vec::new(), Vec::new());
    };
    load_from_path(&path)
}

fn load_from_path(path: &std::path::Path) -> (Vec<HookDef>, Vec<String>) {
    let Ok(data) = std::fs::read_to_string(path) else {
        return (Vec::new(), Vec::new());
    };
    let parsed: HooksFile = match serde_json::from_str(&data) {
        Ok(p) => p,
        Err(e) => return (Vec::new(), vec![format!("hooks.json invalido: {e}")]),
    };
    let mut defs = Vec::new();
    let mut warnings = Vec::new();
    for raw in parsed.hooks {
        match HookDef::from_raw(raw) {
            Ok(def) => defs.push(def),
            Err(e) => warnings.push(e),
        }
    }
    (defs, warnings)
}

/// Ruta estandar: `$XDG_CONFIG_HOME/ingenieria-tui/hooks.json`.
pub fn hooks_config_path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("ingenieria-tui").join("hooks.json"))
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
    fn empty_file_returns_no_hooks() {
        let f = write_tmp(r#"{ "hooks": [] }"#);
        let (defs, warns) = load_from_path(f.path());
        assert!(defs.is_empty());
        assert!(warns.is_empty());
    }

    #[test]
    fn nonexistent_file_is_silent() {
        let (defs, warns) = load_from_path(std::path::Path::new("/does/not/exist.json"));
        assert!(defs.is_empty());
        assert!(warns.is_empty());
    }

    #[test]
    fn parses_valid_hook() {
        let f = write_tmp(
            r#"{
            "hooks": [{
                "name": "audit",
                "trigger": "PreToolUse",
                "match": { "tool": "Bash*" },
                "command": "echo hi",
                "timeout_secs": 3
            }]
        }"#,
        );
        let (defs, warns) = load_from_path(f.path());
        assert!(warns.is_empty());
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0].name, "audit");
        assert_eq!(defs[0].trigger, HookTrigger::PreToolUse);
        assert_eq!(defs[0].match_tool.as_deref(), Some("Bash*"));
        assert_eq!(defs[0].timeout_secs, 3);
    }

    #[test]
    fn defaults_applied_for_missing_fields() {
        let f = write_tmp(
            r#"{
            "hooks": [{
                "name": "x",
                "trigger": "PostToolUse",
                "command": "true"
            }]
        }"#,
        );
        let (defs, warns) = load_from_path(f.path());
        assert!(warns.is_empty());
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0].timeout_secs, 5);
        assert_eq!(defs[0].on_failure, HookFailurePolicy::Warn);
        assert!(defs[0].match_tool.is_none());
    }

    #[test]
    fn invalid_trigger_becomes_warning_not_fatal() {
        let f = write_tmp(
            r#"{
            "hooks": [
                { "name": "bad", "trigger": "Nope", "command": "x" },
                { "name": "good", "trigger": "PreCodeApply", "command": "y" }
            ]
        }"#,
        );
        let (defs, warns) = load_from_path(f.path());
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0].name, "good");
        assert_eq!(warns.len(), 1);
        assert!(warns[0].contains("bad"));
    }

    #[test]
    fn empty_command_rejected() {
        let f = write_tmp(
            r#"{
            "hooks": [{
                "name": "empty",
                "trigger": "PreToolUse",
                "command": "   "
            }]
        }"#,
        );
        let (defs, warns) = load_from_path(f.path());
        assert!(defs.is_empty());
        assert_eq!(warns.len(), 1);
    }

    #[test]
    fn timeout_zero_falls_back_to_default() {
        let f = write_tmp(
            r#"{
            "hooks": [{
                "name": "x",
                "trigger": "PreToolUse",
                "command": "true",
                "timeout_secs": 0
            }]
        }"#,
        );
        let (defs, _) = load_from_path(f.path());
        assert_eq!(defs[0].timeout_secs, 5);
    }

    #[test]
    fn timeout_clamped_to_max() {
        let f = write_tmp(
            r#"{
            "hooks": [{
                "name": "x",
                "trigger": "PreToolUse",
                "command": "true",
                "timeout_secs": 9999
            }]
        }"#,
        );
        let (defs, _) = load_from_path(f.path());
        assert_eq!(defs[0].timeout_secs, 300);
    }

    #[test]
    fn malformed_json_is_warning() {
        let f = write_tmp("not json");
        let (defs, warns) = load_from_path(f.path());
        assert!(defs.is_empty());
        assert_eq!(warns.len(), 1);
    }
}
