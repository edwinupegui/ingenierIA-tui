//! Permission system: policy + enforcement pipeline.
//!
//! Policy types are in `ingenieria-runtime::permissions::policy`.
//! The enforcer stays here because it depends on bash/tools modules.

pub mod enforcer;

// Re-export policy from runtime for backward compatibility.
pub use ingenieria_runtime::permissions::policy;

pub use enforcer::{EnforcementResult, PermissionEnforcer};
pub use policy::{PermissionMode, PermissionRules};
