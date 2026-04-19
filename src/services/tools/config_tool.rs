//! ConfigTool (E20) — permite al AI consultar y ajustar configuracion del
//! TUI sobre campos no sensibles.
//!
//! Diseno:
//! - **No vive en `ToolRegistry`** porque necesita una Sender<Action> para
//!   aplicar cambios en estado runtime (factory/permission_mode/theme).
//! - La definicion se expone via `config_tool_definition()` para que
//!   `build_tool_defs` la incluya en el schema enviado al provider.
//! - `chat_tools.rs` detecta `tc.name == CONFIG_TOOL_NAME` y despacha a
//!   `handle_config_tool_request` que retorna el String del tool result y
//!   (para "set") envia una `Action::ApplyConfigChange`.
//! - Lista negra de campos: `api_key`, `server_url`, cualquier nombre con
//!   "key", "token", "secret", "url" (rechazado con mensaje humano).
//!
//! Permisos: el tool defaults a `ToolPermission::Ask` porque no esta en el
//! registry (comportamiento del enforcer para tools desconocidos).

use tokio::sync::mpsc::Sender;

use crate::actions::Action;
use crate::services::chat::ToolDefinition;

/// Nombre canonico del tool que se expone al AI.
pub const CONFIG_TOOL_NAME: &str = "update_config";

/// Campos permitidos para modificar via ConfigTool.
pub const ALLOWED_FIELDS: &[&str] = &["model", "factory", "permission_mode", "theme"];

/// Fragmentos prohibidos en nombres de campo (case-insensitive). Si el field
/// contiene cualquiera de estos, el tool responde con error sin tocar estado.
pub const FORBIDDEN_FRAGMENTS: &[&str] =
    &["key", "token", "secret", "url", "password", "credential", "auth"];

/// Snapshot de la configuracion visible al AI.
#[derive(Debug, Clone)]
pub struct ConfigSnapshot {
    pub model: String,
    pub factory: String,
    pub permission_mode: String,
    pub theme: String,
}

/// Peticion parseada desde los arguments JSON del tool.
#[derive(Debug, Clone, PartialEq)]
pub enum ConfigToolRequest {
    Get,
    Set { field: String, value: String },
}

/// Definicion OpenAI-compatible para enviar al provider.
pub fn config_tool_definition() -> ToolDefinition {
    ToolDefinition {
        json: serde_json::json!({
            "type": "function",
            "function": {
                "name": CONFIG_TOOL_NAME,
                "description": "Consulta o modifica configuracion runtime del TUI. \
                    Campos permitidos: model (string libre), factory (net|ang|nest|all), \
                    permission_mode (Standard|Permissive|Strict), theme (tokyonight|solarized|high-contrast|gruvbox|monokai|matrix). \
                    NUNCA acepta api_key, server_url ni secretos. Cada cambio se auditea.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "action": {
                            "type": "string",
                            "enum": ["get", "set"],
                            "description": "get devuelve snapshot actual; set aplica el cambio."
                        },
                        "field": {
                            "type": "string",
                            "description": "Nombre del campo (requerido si action=set)."
                        },
                        "value": {
                            "type": "string",
                            "description": "Nuevo valor (requerido si action=set)."
                        }
                    },
                    "required": ["action"]
                }
            }
        }),
    }
}

/// Parsea argumentos JSON a una peticion estructurada. Errores legibles para
/// devolver al AI como tool result.
pub fn parse_request(arguments: &str) -> anyhow::Result<ConfigToolRequest> {
    let v: serde_json::Value = serde_json::from_str(arguments)
        .map_err(|e| anyhow::anyhow!("arguments JSON invalido: {e}"))?;
    let obj = v.as_object().ok_or_else(|| anyhow::anyhow!("arguments debe ser un objeto JSON"))?;
    let action = obj
        .get("action")
        .and_then(|a| a.as_str())
        .ok_or_else(|| anyhow::anyhow!("campo 'action' requerido (get|set)"))?;
    match action {
        "get" => Ok(ConfigToolRequest::Get),
        "set" => {
            let field = obj
                .get("field")
                .and_then(|f| f.as_str())
                .ok_or_else(|| anyhow::anyhow!("campo 'field' requerido en set"))?;
            let value = obj
                .get("value")
                .and_then(|f| f.as_str())
                .ok_or_else(|| anyhow::anyhow!("campo 'value' requerido en set"))?;
            Ok(ConfigToolRequest::Set { field: field.to_string(), value: value.to_string() })
        }
        other => anyhow::bail!("action invalida '{other}': usa get|set"),
    }
}

/// Valida el nombre del campo contra la lista negra y whitelist.
/// Retorna `Ok(field_normalized)` o `Err(mensaje)`.
pub fn validate_field(field: &str) -> anyhow::Result<&'static str> {
    let lowered = field.to_lowercase();
    for forbidden in FORBIDDEN_FRAGMENTS {
        if lowered.contains(forbidden) {
            anyhow::bail!(
                "campo '{field}' rechazado: contiene '{forbidden}' (prohibido por seguridad)"
            );
        }
    }
    ALLOWED_FIELDS.iter().find(|f| **f == lowered).copied().ok_or_else(|| {
        anyhow::anyhow!("campo '{field}' no permitido. Usa: {}", ALLOWED_FIELDS.join(", "))
    })
}

/// Valida el valor segun el campo.
pub fn validate_value(field: &str, value: &str) -> anyhow::Result<()> {
    match field {
        "model" => {
            if value.is_empty() || value.len() > 100 {
                anyhow::bail!("model: valor vacio o >100 chars");
            }
            Ok(())
        }
        "factory" => match value {
            "net" | "ang" | "nest" | "all" => Ok(()),
            other => anyhow::bail!("factory '{other}' invalido. Usa: net|ang|nest|all"),
        },
        "permission_mode" => match value {
            "Standard" | "Permissive" | "Strict" => Ok(()),
            other => anyhow::bail!(
                "permission_mode '{other}' invalido. Usa: Standard|Permissive|Strict"
            ),
        },
        "theme" => match value {
            "tokyonight" | "solarized" | "high-contrast" | "gruvbox" | "monokai" | "matrix" => {
                Ok(())
            }
            other => anyhow::bail!(
                "theme '{other}' invalido. Usa: tokyonight|solarized|high-contrast|gruvbox|monokai|matrix"
            ),
        },
        _ => anyhow::bail!("field '{field}' sin validador"),
    }
}

/// Formatea un snapshot como JSON legible para enviar al AI.
pub fn format_snapshot(snap: &ConfigSnapshot) -> String {
    let v = serde_json::json!({
        "model": snap.model,
        "factory": snap.factory,
        "permission_mode": snap.permission_mode,
        "theme": snap.theme,
    });
    serde_json::to_string_pretty(&v).unwrap_or_else(|_| "{}".into())
}

/// Entrypoint llamado por `chat_tools.rs`. Retorna el tool result como
/// String y, para `set`, envia `Action::ApplyConfigChange` por `tx`.
pub async fn handle_request(
    arguments: &str,
    snapshot: &ConfigSnapshot,
    tx: &Sender<Action>,
) -> String {
    let request = match parse_request(arguments) {
        Ok(r) => r,
        Err(e) => return format!("Error: {e}"),
    };
    match request {
        ConfigToolRequest::Get => format_snapshot(snapshot),
        ConfigToolRequest::Set { field, value } => {
            let canonical = match validate_field(&field) {
                Ok(f) => f,
                Err(e) => return format!("Error: {e}"),
            };
            if let Err(e) = validate_value(canonical, &value) {
                return format!("Error: {e}");
            }
            let dispatched = tx
                .send(Action::ApplyConfigChange {
                    field: canonical.to_string(),
                    value: value.clone(),
                })
                .await;
            match dispatched {
                Ok(()) => format!("OK: {canonical} = {value} (cambio programado + auditado)"),
                Err(_) => "Error: canal de actions cerrado; el TUI se esta cerrando".into(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_request_get() {
        assert_eq!(parse_request(r#"{"action":"get"}"#).unwrap(), ConfigToolRequest::Get);
    }

    #[test]
    fn parse_request_set_needs_field_and_value() {
        assert!(parse_request(r#"{"action":"set"}"#).is_err());
        assert!(parse_request(r#"{"action":"set","field":"model"}"#).is_err());
    }

    #[test]
    fn parse_request_set_complete() {
        let r = parse_request(r#"{"action":"set","field":"model","value":"claude-4-6"}"#).unwrap();
        match r {
            ConfigToolRequest::Set { field, value } => {
                assert_eq!(field, "model");
                assert_eq!(value, "claude-4-6");
            }
            _ => panic!("expected Set"),
        }
    }

    #[test]
    fn parse_request_rejects_non_object() {
        assert!(parse_request("\"not-object\"").is_err());
    }

    #[test]
    fn parse_request_rejects_unknown_action() {
        assert!(parse_request(r#"{"action":"delete"}"#).is_err());
    }

    #[test]
    fn validate_field_rejects_api_key() {
        assert!(validate_field("api_key").is_err());
        assert!(validate_field("API_KEY").is_err());
        assert!(validate_field("user_token").is_err());
    }

    #[test]
    fn validate_field_rejects_server_url() {
        assert!(validate_field("server_url").is_err());
        assert!(validate_field("backend_url").is_err());
    }

    #[test]
    fn validate_field_accepts_whitelisted() {
        assert_eq!(validate_field("model").unwrap(), "model");
        assert_eq!(validate_field("FACTORY").unwrap(), "factory");
        assert_eq!(validate_field("permission_mode").unwrap(), "permission_mode");
        assert_eq!(validate_field("theme").unwrap(), "theme");
    }

    #[test]
    fn validate_field_rejects_unknown() {
        assert!(validate_field("developer").is_err());
        assert!(validate_field("random_field").is_err());
    }

    #[test]
    fn validate_value_factory_whitelists() {
        assert!(validate_value("factory", "net").is_ok());
        assert!(validate_value("factory", "all").is_ok());
        assert!(validate_value("factory", "unknown").is_err());
    }

    #[test]
    fn validate_value_permission_mode_whitelists() {
        assert!(validate_value("permission_mode", "Standard").is_ok());
        assert!(validate_value("permission_mode", "LowerCase").is_err());
    }

    #[test]
    fn validate_value_theme_whitelists() {
        assert!(validate_value("theme", "tokyonight").is_ok());
        assert!(validate_value("theme", "high-contrast").is_ok());
        assert!(validate_value("theme", "gruvbox").is_ok());
        assert!(validate_value("theme", "dark").is_err()); // dark ya no existe
        assert!(validate_value("theme", "purple").is_err());
    }

    #[test]
    fn validate_value_model_rejects_empty() {
        assert!(validate_value("model", "").is_err());
    }

    #[test]
    fn format_snapshot_is_valid_json() {
        let snap = ConfigSnapshot {
            model: "claude".into(),
            factory: "net".into(),
            permission_mode: "Standard".into(),
            theme: "dark".into(),
        };
        let s = format_snapshot(&snap);
        let parsed: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(parsed["model"], "claude");
        assert_eq!(parsed["factory"], "net");
    }

    #[test]
    fn config_tool_definition_exposes_required_shape() {
        let def = config_tool_definition();
        assert_eq!(def.json["function"]["name"], CONFIG_TOOL_NAME);
        assert!(def.json["function"]["parameters"]["properties"]["action"].is_object());
    }

    #[tokio::test]
    async fn handle_request_get_returns_snapshot() {
        let (tx, _rx) = tokio::sync::mpsc::channel::<Action>(4);
        let snap = ConfigSnapshot {
            model: "m".into(),
            factory: "net".into(),
            permission_mode: "Standard".into(),
            theme: "dark".into(),
        };
        let out = handle_request(r#"{"action":"get"}"#, &snap, &tx).await;
        let parsed: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(parsed["model"], "m");
    }

    #[tokio::test]
    async fn handle_request_set_dispatches_action() {
        let (tx, mut rx) = tokio::sync::mpsc::channel::<Action>(4);
        let snap = ConfigSnapshot {
            model: "m".into(),
            factory: "net".into(),
            permission_mode: "Standard".into(),
            theme: "dark".into(),
        };
        let out =
            handle_request(r#"{"action":"set","field":"theme","value":"solarized"}"#, &snap, &tx)
                .await;
        assert!(out.starts_with("OK"));
        match rx.try_recv() {
            Ok(Action::ApplyConfigChange { field, value }) => {
                assert_eq!(field, "theme");
                assert_eq!(value, "solarized");
            }
            other => panic!("expected ApplyConfigChange, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn handle_request_set_rejects_api_key_without_dispatch() {
        let (tx, mut rx) = tokio::sync::mpsc::channel::<Action>(4);
        let snap = ConfigSnapshot {
            model: "m".into(),
            factory: "net".into(),
            permission_mode: "Standard".into(),
            theme: "dark".into(),
        };
        let out =
            handle_request(r#"{"action":"set","field":"api_key","value":"sk-leaked"}"#, &snap, &tx)
                .await;
        assert!(out.starts_with("Error"));
        assert!(rx.try_recv().is_err(), "no se debe despachar Action al rechazar");
    }

    #[tokio::test]
    async fn handle_request_set_rejects_invalid_value() {
        let (tx, mut rx) = tokio::sync::mpsc::channel::<Action>(4);
        let snap = ConfigSnapshot {
            model: "m".into(),
            factory: "net".into(),
            permission_mode: "Standard".into(),
            theme: "dark".into(),
        };
        let out =
            handle_request(r#"{"action":"set","field":"factory","value":"xyz"}"#, &snap, &tx).await;
        assert!(out.starts_with("Error"));
        assert!(rx.try_recv().is_err());
    }
}
