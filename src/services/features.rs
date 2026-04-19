/// Compile-time feature gate check.
///
/// Returns `true` if the named feature is enabled in this build.
///
/// ```rust
/// if feature_enabled!("autoskill") {
///     self.spawn_autoskill_scan();
/// }
/// ```
#[macro_export]
macro_rules! feature_enabled {
    ("copilot") => {
        cfg!(feature = "copilot")
    };
    ("mcp") => {
        cfg!(feature = "mcp")
    };
    ("autoskill") => {
        cfg!(feature = "autoskill")
    };
    ($name:expr) => {
        false
    };
}

/// Feature flag descriptor for runtime introspection.
pub struct FeatureFlag {
    pub name: &'static str,
    pub enabled: bool,
}

/// List all feature flags and their compile-time status.
pub fn list_features() -> Vec<FeatureFlag> {
    vec![
        FeatureFlag { name: "copilot", enabled: cfg!(feature = "copilot") },
        FeatureFlag { name: "mcp", enabled: cfg!(feature = "mcp") },
        FeatureFlag { name: "autoskill", enabled: cfg!(feature = "autoskill") },
    ]
}
