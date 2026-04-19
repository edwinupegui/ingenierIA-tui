//! Permission policy: mode-based access control with persistent rules.
//!
//! Manages permission modes (ReadOnly → Prompt → FullAccess) and
//! per-tool allow/deny rules persisted to `permissions.json`.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

// ── Permission mode hierarchy ────────────────────────────────────────────

/// Global permission mode. Higher modes allow more actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum PermissionMode {
    /// Only read operations. Write/exec blocked.
    ReadOnly,
    /// Write within workspace. Destructive/system commands prompt.
    #[default]
    WorkspaceWrite,
    /// Everything prompts except explicit allows.
    Prompt,
    /// Full access without prompts (use with caution).
    FullAccess,
}

impl PermissionMode {
    /// Numeric level for comparison (higher = more permissive).
    fn level(self) -> u8 {
        match self {
            Self::ReadOnly => 0,
            Self::WorkspaceWrite => 1,
            Self::Prompt => 2,
            Self::FullAccess => 3,
        }
    }

    /// Returns true if this mode is at least as permissive as `required`.
    pub fn satisfies(self, required: Self) -> bool {
        self.level() >= required.level()
    }
}

// ── Per-tool overrides ──────────────────────────────────────────────────

/// Override for a specific tool (persisted in permissions.json).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionOverride {
    Allow,
    Deny,
}

// ── Persistent rules ────────────────────────────────────────────────────

/// Persistent permission rules loaded from disk.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PermissionRules {
    #[serde(default)]
    pub always_allow: Vec<String>,
    #[serde(default)]
    pub always_deny: Vec<String>,
}

impl PermissionRules {
    /// Load rules from disk. Returns defaults if file doesn't exist.
    pub fn load() -> Self {
        let Some(path) = config_path() else {
            return Self::default();
        };
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|data| serde_json::from_str(&data).ok())
            .unwrap_or_default()
    }

    /// Save rules to disk with restricted permissions.
    pub fn save(&self) -> anyhow::Result<()> {
        let path = config_path().ok_or_else(|| anyhow::anyhow!("No config dir"))?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, json)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
        }
        Ok(())
    }

    /// Check if a tool has a persistent override.
    pub fn check(&self, tool_name: &str) -> Option<PermissionOverride> {
        if self.always_allow.iter().any(|t| t == tool_name) {
            Some(PermissionOverride::Allow)
        } else if self.always_deny.iter().any(|t| t == tool_name) {
            Some(PermissionOverride::Deny)
        } else {
            None
        }
    }

    /// Add a tool to always_allow (removes from always_deny).
    pub fn add_allow(&mut self, tool_name: &str) {
        self.always_deny.retain(|t| t != tool_name);
        if !self.always_allow.iter().any(|t| t == tool_name) {
            self.always_allow.push(tool_name.to_string());
        }
    }

    /// Add a tool to always_deny (removes from always_allow).
    pub fn add_deny(&mut self, tool_name: &str) {
        self.always_allow.retain(|t| t != tool_name);
        if !self.always_deny.iter().any(|t| t == tool_name) {
            self.always_deny.push(tool_name.to_string());
        }
    }
}

fn config_path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("ingenieria-tui").join("permissions.json"))
}

// ── Required permission for tool types ──────────────────────────────────

/// Minimum permission mode required to execute a tool type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequiredMode {
    /// Read-only tools (Read, Glob, Grep, list_directory).
    ReadOnly,
    /// Write tools (Edit, Write, file modifications).
    WorkspaceWrite,
    /// Dangerous tools (Bash, shell execution).
    Prompt,
}

impl RequiredMode {
    /// Map to the minimum `PermissionMode` that satisfies this requirement.
    pub fn to_mode(self) -> PermissionMode {
        match self {
            Self::ReadOnly => PermissionMode::ReadOnly,
            Self::WorkspaceWrite => PermissionMode::WorkspaceWrite,
            Self::Prompt => PermissionMode::Prompt,
        }
    }
}
