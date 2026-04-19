//! Tipos base para el sistema de hooks configurable (E16).
//!
//! Un hook es un comando shell que se dispara ante eventos del TUI:
//! - `PreToolUse` / `PostToolUse`: antes/despues de ejecutar una tool.
//! - `PreCodeApply`: antes de aplicar un code block al filesystem.
//! - `OnFactorySwitch`: al cambiar de factory context.
//!
//! Filosofia: **observabilidad + automatizacion ligera**, no enforcement.
//! La politica de permisos la gestiona `PermissionEnforcer`; los hooks
//! reciben contexto via env vars y pueden loggear/notificar/side-effectear.

use std::collections::HashMap;

/// Evento que dispara un hook. La serializacion lowercase se usa tanto para
/// matching del campo `trigger` en `hooks.json` como para el valor de la env
/// var `INGENIERIA_HOOK_TRIGGER`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HookTrigger {
    PreToolUse,
    PostToolUse,
    PreCodeApply,
    OnFactorySwitch,
}

impl HookTrigger {
    pub fn label(&self) -> &'static str {
        match self {
            Self::PreToolUse => "PreToolUse",
            Self::PostToolUse => "PostToolUse",
            Self::PreCodeApply => "PreCodeApply",
            Self::OnFactorySwitch => "OnFactorySwitch",
        }
    }

    /// Parse case-insensitive. Acepta variantes con/sin separadores.
    pub fn from_label(s: &str) -> Option<Self> {
        let normalized = s.to_lowercase().replace(['_', '-'], "");
        match normalized.as_str() {
            "pretooluse" => Some(Self::PreToolUse),
            "posttooluse" => Some(Self::PostToolUse),
            "precodeapply" => Some(Self::PreCodeApply),
            "onfactoryswitch" => Some(Self::OnFactorySwitch),
            _ => None,
        }
    }
}

/// Contexto del evento, inyectado como env vars al comando del hook.
#[derive(Debug, Clone, Default)]
pub struct HookContext {
    pub tool_name: Option<String>,
    pub tool_arguments: Option<String>,
    pub file_path: Option<String>,
    pub factory_from: Option<String>,
    pub factory_to: Option<String>,
    pub tool_success: Option<bool>,
    pub tool_duration_ms: Option<u64>,
}

impl HookContext {
    pub fn for_tool(name: &str, arguments: &str) -> Self {
        Self {
            tool_name: Some(name.to_string()),
            tool_arguments: Some(arguments.to_string()),
            ..Self::default()
        }
    }

    pub fn for_tool_result(name: &str, arguments: &str, success: bool, duration_ms: u64) -> Self {
        Self {
            tool_name: Some(name.to_string()),
            tool_arguments: Some(arguments.to_string()),
            tool_success: Some(success),
            tool_duration_ms: Some(duration_ms),
            ..Self::default()
        }
    }

    pub fn for_code_apply(path: &str) -> Self {
        Self { file_path: Some(path.to_string()), ..Self::default() }
    }

    pub fn for_factory_switch(from: &str, to: &str) -> Self {
        Self {
            factory_from: Some(from.to_string()),
            factory_to: Some(to.to_string()),
            ..Self::default()
        }
    }

    /// Materializa env vars con prefijo `INGENIERIA_` para el proceso hijo.
    pub fn env_vars(&self, trigger: HookTrigger) -> HashMap<String, String> {
        let mut env = HashMap::new();
        env.insert("INGENIERIA_HOOK_TRIGGER".to_string(), trigger.label().to_string());
        if let Some(v) = &self.tool_name {
            env.insert("INGENIERIA_TOOL_NAME".to_string(), v.clone());
        }
        if let Some(v) = &self.tool_arguments {
            env.insert("INGENIERIA_TOOL_ARGS".to_string(), v.clone());
        }
        if let Some(v) = &self.file_path {
            env.insert("INGENIERIA_FILE_PATH".to_string(), v.clone());
        }
        if let Some(v) = &self.factory_from {
            env.insert("INGENIERIA_FACTORY_FROM".to_string(), v.clone());
        }
        if let Some(v) = &self.factory_to {
            env.insert("INGENIERIA_FACTORY_TO".to_string(), v.clone());
        }
        if let Some(v) = self.tool_success {
            env.insert("INGENIERIA_TOOL_SUCCESS".to_string(), v.to_string());
        }
        if let Some(v) = self.tool_duration_ms {
            env.insert("INGENIERIA_TOOL_DURATION_MS".to_string(), v.to_string());
        }
        env
    }

    /// Nombre de la tool (si aplica). Usado para `match_tool` patterns.
    pub fn matches_tool(&self, pattern: Option<&str>) -> bool {
        match pattern {
            None => true,
            Some(pat) => matches_glob(pat, self.tool_name.as_deref().unwrap_or("")),
        }
    }
}

/// Match simple estilo glob: soporta `*` como wildcard al final o inicio.
/// No es regex: suficiente para `Bash*`, `*Read`, `Edit`.
pub fn matches_glob(pattern: &str, value: &str) -> bool {
    if pattern == "*" || pattern.is_empty() {
        return true;
    }
    if let Some(suffix) = pattern.strip_prefix('*') {
        return value.ends_with(suffix);
    }
    if let Some(prefix) = pattern.strip_suffix('*') {
        return value.starts_with(prefix);
    }
    pattern == value
}

/// Resultado de un hook ejecutado. Se dispatcha al reducer como Action.
#[derive(Debug, Clone)]
pub struct HookOutcome {
    pub name: String,
    pub trigger: HookTrigger,
    pub exit_code: i32,
    pub duration_ms: u64,
    /// Ultimas lineas de stderr (trunc). Vacio en exito silencioso.
    pub stderr_tail: String,
}

impl HookOutcome {
    pub fn is_success(&self) -> bool {
        self.exit_code == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trigger_roundtrip() {
        for t in [
            HookTrigger::PreToolUse,
            HookTrigger::PostToolUse,
            HookTrigger::PreCodeApply,
            HookTrigger::OnFactorySwitch,
        ] {
            assert_eq!(HookTrigger::from_label(t.label()), Some(t));
        }
    }

    #[test]
    fn trigger_from_label_accepts_variants() {
        assert_eq!(HookTrigger::from_label("pre_tool_use"), Some(HookTrigger::PreToolUse));
        assert_eq!(HookTrigger::from_label("POST-TOOL-USE"), Some(HookTrigger::PostToolUse));
        assert_eq!(HookTrigger::from_label("wat"), None);
    }

    #[test]
    fn glob_matches_wildcards() {
        assert!(matches_glob("*", "anything"));
        assert!(matches_glob("Bash*", "BashSafe"));
        assert!(matches_glob("*Read", "FileRead"));
        assert!(matches_glob("Edit", "Edit"));
        assert!(!matches_glob("Edit", "Write"));
        assert!(!matches_glob("Bash*", "EditBash"));
    }

    #[test]
    fn env_vars_include_trigger_and_context() {
        let ctx = HookContext::for_tool("Bash", "ls -la");
        let env = ctx.env_vars(HookTrigger::PreToolUse);
        assert_eq!(env.get("INGENIERIA_HOOK_TRIGGER").map(String::as_str), Some("PreToolUse"));
        assert_eq!(env.get("INGENIERIA_TOOL_NAME").map(String::as_str), Some("Bash"));
        assert_eq!(env.get("INGENIERIA_TOOL_ARGS").map(String::as_str), Some("ls -la"));
    }

    #[test]
    fn factory_switch_env_has_from_and_to() {
        let ctx = HookContext::for_factory_switch("net", "ang");
        let env = ctx.env_vars(HookTrigger::OnFactorySwitch);
        assert_eq!(env.get("INGENIERIA_FACTORY_FROM").map(String::as_str), Some("net"));
        assert_eq!(env.get("INGENIERIA_FACTORY_TO").map(String::as_str), Some("ang"));
    }

    #[test]
    fn matches_tool_with_pattern() {
        let ctx = HookContext::for_tool("Bash", "");
        assert!(ctx.matches_tool(None));
        assert!(ctx.matches_tool(Some("*")));
        assert!(ctx.matches_tool(Some("Bash")));
        assert!(ctx.matches_tool(Some("Bash*")));
        assert!(!ctx.matches_tool(Some("Read")));
    }
}
