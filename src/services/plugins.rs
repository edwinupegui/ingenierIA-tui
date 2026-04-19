//! Plugin registry — manages loaded plugins and dispatches lifecycle hooks (E28).
//!
//! The registry holds `Box<dyn Plugin>` instances and provides methods
//! that the App calls at the appropriate lifecycle points.

use ingenieria_domain::plugin::{Plugin, PluginEffect, PluginResponse};

/// Max plugins that can be loaded simultaneously.
const MAX_PLUGINS: usize = 16;

/// Manages a set of loaded plugins.
#[derive(Default)]
pub struct PluginRegistry {
    plugins: Vec<Box<dyn Plugin>>,
}

impl PluginRegistry {
    /// Register a plugin. Returns false if the limit has been reached.
    pub fn register(&mut self, plugin: Box<dyn Plugin>) -> bool {
        if self.plugins.len() >= MAX_PLUGINS {
            return false;
        }
        self.plugins.push(plugin);
        true
    }

    /// Number of loaded plugins.
    pub fn len(&self) -> usize {
        self.plugins.len()
    }

    pub fn is_empty(&self) -> bool {
        self.plugins.is_empty()
    }

    /// Collect init effects from all plugins.
    pub fn on_init(&self) -> Vec<PluginEffect> {
        self.plugins.iter().flat_map(|p| p.on_init()).collect()
    }

    /// Run pre-action hooks. Returns `Block(reason)` if any plugin blocks.
    pub fn on_pre_action(&self, action_tag: &str) -> PluginResponse {
        for plugin in &self.plugins {
            let response = plugin.on_pre_action(action_tag);
            if let PluginResponse::Block(_) = &response {
                return response;
            }
        }
        PluginResponse::Continue
    }

    /// Run post-action hooks on all plugins.
    pub fn on_post_action(&self, action_tag: &str) {
        for plugin in &self.plugins {
            plugin.on_post_action(action_tag);
        }
    }

    /// Collect status hints from all plugins.
    pub fn status_hints(&self) -> Vec<String> {
        self.plugins.iter().flat_map(|p| p.status_hints()).collect()
    }

    /// Shutdown all plugins (best-effort).
    pub fn on_shutdown(&self) {
        for plugin in &self.plugins {
            plugin.on_shutdown();
        }
    }

    /// List plugin names and versions.
    pub fn list(&self) -> Vec<(&str, &str)> {
        self.plugins.iter().map(|p| (p.name(), p.version())).collect()
    }
}

impl std::fmt::Debug for PluginRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PluginRegistry").field("count", &self.plugins.len()).finish()
    }
}

// ── ManifestPlugin ─────────────────────────────────────────────────────────

use ingenieria_domain::plugin::{NotifyLevel, PluginManifest};

/// Plugin basado en un manifest JSON leido de disco.
///
/// En `on_init` emite un toast informativo. Los hooks de shell
/// (`pre_tool_use`, `post_tool_use`) se ejecutan via el sistema de hooks
/// existente (E16) — este struct solo aporta metadata al registry.
pub struct ManifestPlugin {
    manifest: PluginManifest,
}

impl ManifestPlugin {
    pub fn new(manifest: PluginManifest) -> Self {
        Self { manifest }
    }
}

impl Plugin for ManifestPlugin {
    fn name(&self) -> &str {
        &self.manifest.name
    }

    fn version(&self) -> &str {
        &self.manifest.version
    }

    fn on_init(&self) -> Vec<PluginEffect> {
        vec![PluginEffect::Notify {
            message: format!("Plugin {} v{} cargado", self.manifest.name, self.manifest.version),
            level: NotifyLevel::Info,
        }]
    }

    fn status_hints(&self) -> Vec<String> {
        vec![format!("[{}]", self.manifest.name)]
    }
}

/// Escanea un directorio buscando `manifest.json` en subdirectorios.
///
/// Layout esperado:
/// ```text
/// ~/.config/ingenieria-tui/plugins/
///   my-plugin/
///     manifest.json
///   other-plugin/
///     manifest.json
/// ```
///
/// Plugins con manifests invalidos se omiten con un warning en logs.
pub fn load_from_dir(registry: &mut PluginRegistry, dir: &std::path::Path) -> usize {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return 0,
    };
    let mut count = 0;
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let manifest_path = path.join("manifest.json");
        if !manifest_path.exists() {
            continue;
        }
        match load_manifest(&manifest_path) {
            Ok(manifest) => {
                if registry.register(Box::new(ManifestPlugin::new(manifest))) {
                    count += 1;
                }
            }
            Err(e) => {
                tracing::warn!(path = %manifest_path.display(), error = %e, "plugin manifest inválido");
            }
        }
    }
    count
}

fn load_manifest(path: &std::path::Path) -> anyhow::Result<PluginManifest> {
    let content = std::fs::read_to_string(path).map_err(anyhow::Error::from)?;
    serde_json::from_str(&content).map_err(anyhow::Error::from)
}

/// Ruta por defecto del directorio de plugins.
pub fn default_plugins_dir() -> Option<std::path::PathBuf> {
    dirs::config_dir().map(|d| d.join("ingenieria-tui").join("plugins"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ingenieria_domain::plugin::NotifyLevel;

    struct TestPlugin {
        name: &'static str,
    }

    impl Plugin for TestPlugin {
        fn name(&self) -> &str {
            self.name
        }
        fn version(&self) -> &str {
            "1.0.0"
        }

        fn on_init(&self) -> Vec<PluginEffect> {
            vec![PluginEffect::Notify {
                message: format!("{} loaded", self.name),
                level: NotifyLevel::Info,
            }]
        }

        fn status_hints(&self) -> Vec<String> {
            vec![format!("[{}]", self.name)]
        }
    }

    struct BlockingPlugin;

    impl Plugin for BlockingPlugin {
        fn name(&self) -> &str {
            "blocker"
        }

        fn on_pre_action(&self, action_tag: &str) -> PluginResponse {
            if action_tag == "KeyDelete" {
                PluginResponse::Block("delete not allowed".into())
            } else {
                PluginResponse::Continue
            }
        }
    }

    #[test]
    fn register_and_list() {
        let mut reg = PluginRegistry::default();
        reg.register(Box::new(TestPlugin { name: "alpha" }));
        reg.register(Box::new(TestPlugin { name: "beta" }));
        assert_eq!(reg.len(), 2);
        let list = reg.list();
        assert_eq!(list[0], ("alpha", "1.0.0"));
        assert_eq!(list[1], ("beta", "1.0.0"));
    }

    #[test]
    fn on_init_collects_effects() {
        let mut reg = PluginRegistry::default();
        reg.register(Box::new(TestPlugin { name: "alpha" }));
        reg.register(Box::new(TestPlugin { name: "beta" }));
        let effects = reg.on_init();
        assert_eq!(effects.len(), 2);
    }

    #[test]
    fn pre_action_blocks_when_plugin_says_no() {
        let mut reg = PluginRegistry::default();
        reg.register(Box::new(BlockingPlugin));
        assert_eq!(
            reg.on_pre_action("KeyDelete"),
            PluginResponse::Block("delete not allowed".into())
        );
        assert_eq!(reg.on_pre_action("KeyEnter"), PluginResponse::Continue);
    }

    #[test]
    fn status_hints_aggregated() {
        let mut reg = PluginRegistry::default();
        reg.register(Box::new(TestPlugin { name: "alpha" }));
        reg.register(Box::new(TestPlugin { name: "beta" }));
        let hints = reg.status_hints();
        assert_eq!(hints, vec!["[alpha]", "[beta]"]);
    }

    #[test]
    fn max_plugins_limit() {
        let mut reg = PluginRegistry::default();
        for i in 0..MAX_PLUGINS {
            assert!(reg.register(Box::new(TestPlugin {
                name: Box::leak(format!("p{i}").into_boxed_str()),
            })));
        }
        assert!(!reg.register(Box::new(TestPlugin { name: "overflow" })));
        assert_eq!(reg.len(), MAX_PLUGINS);
    }
}
