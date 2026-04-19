//! Plugin trait and types for extending ingenierIA TUI (E28).
//!
//! The trait is **object-safe** (`dyn Plugin`) and lives in ingenieria-domain
//! so that plugin crates only depend on the domain layer — not on the full
//! TUI binary. The App integrates plugins via a `PluginRegistry`.
//!
//! Lifecycle:
//!   on_init → (on_pre_action / on_post_action)* → on_shutdown

use serde::{Deserialize, Serialize};

// ── Plugin trait ────────────────────────────────────────────────────────────

/// Extension point for ingenierIA TUI.
///
/// Implementors can observe lifecycle events and inject behavior.
/// All methods have default no-op implementations so plugins only
/// override what they need.
pub trait Plugin: Send + Sync {
    /// Unique identifier (e.g. `"my-linter-plugin"`).
    fn name(&self) -> &str;

    /// Human-readable version string.
    fn version(&self) -> &str {
        "0.0.0"
    }

    /// Called once during App startup. Return actions to execute.
    fn on_init(&self) -> Vec<PluginEffect> {
        vec![]
    }

    /// Called **before** the reducer processes an action.
    /// `action_tag` is the variant name (e.g. `"KeyEnter"`, `"ChatStreamDelta"`).
    fn on_pre_action(&self, action_tag: &str) -> PluginResponse {
        let _ = action_tag;
        PluginResponse::Continue
    }

    /// Called **after** the reducer has processed an action.
    fn on_post_action(&self, action_tag: &str) {
        let _ = action_tag;
    }

    /// Provide status-bar hints (short strings shown in the footer).
    fn status_hints(&self) -> Vec<String> {
        vec![]
    }

    /// Called once during App shutdown (best-effort, not guaranteed on crash).
    fn on_shutdown(&self) {}
}

// ── Supporting types ────────────────────────────────────────────────────────

/// What a plugin wants the App to do.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PluginEffect {
    /// Show a toast notification.
    Notify { message: String, level: NotifyLevel },
    /// Inject a user message into the chat.
    InjectMessage { content: String },
    /// Log a structured entry to the audit trail.
    AuditLog { kind: String, detail: String },
}

/// Toast severity for `PluginEffect::Notify`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NotifyLevel {
    Info,
    Warning,
    Error,
}

/// Pre-action hook response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginResponse {
    /// Let the action pass through to the reducer.
    Continue,
    /// Block the action (reducer won't see it). Carries a reason.
    Block(String),
}

// ── Plugin metadata ─────────────────────────────────────────────────────────

/// Declarative metadata loaded from plugin manifests.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub name: String,
    pub version: String,
    pub description: String,
    #[serde(default)]
    pub hooks: PluginHooks,
}

/// Hook script paths (shell commands to run at lifecycle points).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PluginHooks {
    /// Scripts to run on `on_init`.
    #[serde(default)]
    pub init: Vec<String>,
    /// Scripts to run on `on_shutdown`.
    #[serde(default)]
    pub shutdown: Vec<String>,
    /// Scripts to run before a tool call.
    #[serde(default)]
    pub pre_tool_use: Vec<String>,
    /// Scripts to run after a tool call.
    #[serde(default)]
    pub post_tool_use: Vec<String>,
}

impl PluginHooks {
    pub fn is_empty(&self) -> bool {
        self.init.is_empty()
            && self.shutdown.is_empty()
            && self.pre_tool_use.is_empty()
            && self.post_tool_use.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal plugin for testing the trait defaults.
    struct NoopPlugin;

    impl Plugin for NoopPlugin {
        fn name(&self) -> &str {
            "noop"
        }
    }

    #[test]
    fn default_methods_return_sensible_values() {
        let p = NoopPlugin;
        assert_eq!(p.version(), "0.0.0");
        assert!(p.on_init().is_empty());
        assert_eq!(p.on_pre_action("KeyEnter"), PluginResponse::Continue);
        assert!(p.status_hints().is_empty());
    }

    #[test]
    fn plugin_is_object_safe() {
        let p: Box<dyn Plugin> = Box::new(NoopPlugin);
        assert_eq!(p.name(), "noop");
    }

    #[test]
    fn plugin_hooks_empty_when_default() {
        let hooks = PluginHooks::default();
        assert!(hooks.is_empty());
    }

    #[test]
    fn plugin_manifest_deserializes() {
        let json = r#"{
            "name": "test-plugin",
            "version": "1.0.0",
            "description": "A test plugin",
            "hooks": { "init": ["echo hello"] }
        }"#;
        let manifest: PluginManifest = serde_json::from_str(json).unwrap();
        assert_eq!(manifest.name, "test-plugin");
        assert_eq!(manifest.hooks.init.len(), 1);
        assert!(manifest.hooks.shutdown.is_empty());
    }

    #[test]
    fn plugin_effect_serializes() {
        let effect = PluginEffect::Notify { message: "hello".into(), level: NotifyLevel::Info };
        let json = serde_json::to_string(&effect).unwrap();
        assert!(json.contains("Notify"));
        assert!(json.contains("hello"));
    }

    #[test]
    fn plugin_response_block_carries_reason() {
        let r = PluginResponse::Block("not allowed".into());
        match r {
            PluginResponse::Block(reason) => assert_eq!(reason, "not allowed"),
            _ => panic!("expected Block"),
        }
    }
}
