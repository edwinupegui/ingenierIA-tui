//! Metadata sidecar (`<session_id>.meta.json`) para listar sesiones sin
//! parsear todo el JSONL.
//!
//! El JSONL es el source-of-truth. Si un `.meta.json` falta o esta corrupto,
//! se reconstruye escaneando el JSONL.

use std::path::Path;

use serde::{Deserialize, Serialize};

/// Metadata ligera de una sesion. Se actualiza en cada auto-save.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMeta {
    pub id: String,
    pub title: String,
    pub factory: String,
    pub model: String,
    /// Nombre del provider que servía la sesion (ej: "anthropic", "github-copilot",
    /// "mock"). Vacio si la sesion fue creada antes de persistir provider.
    #[serde(default)]
    pub provider: String,
    pub created_at: String,
    #[serde(default)]
    pub updated_at: String,
    /// Si la sesion es un fork, id del padre. None para sesiones root.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    /// Label humano del fork ("probar opus", "rama experimental").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fork_label: Option<String>,
    #[serde(default)]
    pub turn_count: usize,
    #[serde(default)]
    pub message_count: usize,
    #[serde(default)]
    pub total_input_tokens: u32,
    #[serde(default)]
    pub total_output_tokens: u32,
    #[serde(default)]
    pub total_cost: f64,
    #[serde(default = "default_mode")]
    pub mode: String,
}

fn default_mode() -> String {
    "normal".to_string()
}

impl SessionMeta {
    /// Construye un meta minimo para una sesion recien creada.
    pub fn new(id: String, title: String, factory: String, model: String) -> Self {
        Self::new_with_provider(id, title, factory, model, String::new())
    }

    /// Variante que acepta `provider` explicito. `new()` delega aqui con "".
    pub fn new_with_provider(
        id: String,
        title: String,
        factory: String,
        model: String,
        provider: String,
    ) -> Self {
        let now = ingenieria_domain::time::now_iso();
        Self {
            id,
            title,
            factory,
            model,
            provider,
            created_at: now.clone(),
            updated_at: now,
            parent_id: None,
            fork_label: None,
            turn_count: 0,
            message_count: 0,
            total_input_tokens: 0,
            total_output_tokens: 0,
            total_cost: 0.0,
            mode: default_mode(),
        }
    }

    /// Lee un `.meta.json` desde disco. Devuelve `None` si no existe o
    /// esta corrupto (para permitir reconstruccion desde JSONL).
    pub fn load(path: &Path) -> Option<Self> {
        let data = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&data).ok()
    }

    /// Escribe el meta a disco de forma atomica (tmp + rename).
    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let tmp = path.with_extension("meta.json.tmp");
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(&tmp, json)?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn meta_round_trip() {
        let tmp = std::env::temp_dir().join(format!("ingenieria-meta-{}.json", std::process::id()));
        let meta = SessionMeta::new("abc".into(), "Hola".into(), "Net".into(), "claude".into());
        meta.save(&tmp).unwrap();
        let loaded = SessionMeta::load(&tmp).unwrap();
        assert_eq!(loaded.id, "abc");
        assert_eq!(loaded.title, "Hola");
        assert_eq!(loaded.factory, "Net");
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn load_missing_returns_none() {
        let missing = std::env::temp_dir().join("never-existed-12345.meta.json");
        assert!(SessionMeta::load(&missing).is_none());
    }

    #[test]
    fn load_corrupt_returns_none() {
        let tmp =
            std::env::temp_dir().join(format!("ingenieria-corrupt-{}.json", std::process::id()));
        std::fs::write(&tmp, "{not json").unwrap();
        assert!(SessionMeta::load(&tmp).is_none());
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn new_with_provider_round_trip() {
        let tmp =
            std::env::temp_dir().join(format!("ingenieria-meta-prov-{}.json", std::process::id()));
        let meta = SessionMeta::new_with_provider(
            "sid".into(),
            "t".into(),
            "Net".into(),
            "claude-sonnet-4-6".into(),
            "anthropic".into(),
        );
        meta.save(&tmp).unwrap();
        let loaded = SessionMeta::load(&tmp).unwrap();
        assert_eq!(loaded.provider, "anthropic");
        assert_eq!(loaded.model, "claude-sonnet-4-6");
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn backward_compat_missing_provider_defaults_empty() {
        let tmp =
            std::env::temp_dir().join(format!("ingenieria-meta-bc-{}.json", std::process::id()));
        // Formato anterior sin `provider`.
        let legacy = r#"{
            "id":"x","title":"t","factory":"All","model":"m",
            "created_at":"2026-04-17T00:00:00Z","updated_at":"2026-04-17T00:00:00Z",
            "turn_count":0,"message_count":0,
            "total_input_tokens":0,"total_output_tokens":0,"total_cost":0.0,
            "mode":"normal"
        }"#;
        std::fs::write(&tmp, legacy).unwrap();
        let loaded = SessionMeta::load(&tmp).expect("legacy meta must still load");
        assert_eq!(loaded.provider, "");
        let _ = std::fs::remove_file(&tmp);
    }
}
