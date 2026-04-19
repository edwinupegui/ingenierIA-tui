//! Permission enforcement pipeline.
//!
//! Three-level check: persistent rules → workspace boundary → mode-based.
//! For bash tools, delegates to BashValidator for command-level analysis.

use std::path::Path;

use super::policy::{PermissionMode, PermissionOverride, PermissionRules, RequiredMode};
use crate::services::bash::BashValidation;
use crate::services::tools::ToolPermission;

// ── Enforcement result ──────────────────────────────────────────────────

/// Result of the enforcement pipeline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnforcementResult {
    /// Tool execution is allowed without prompting.
    Allow,
    /// User must approve (show permission modal with context).
    PromptUser { reason: String },
    /// Tool execution is denied outright.
    Deny { reason: String },
}

// ── Validator detail (passed to UI) ─────────────────────────────────────

/// Detail from bash validation to show in the permission modal.
#[derive(Debug, Clone)]
pub struct ValidationDetail {
    pub risk_level: RiskLevel,
    pub reasons: Vec<String>,
}

/// Risk level for UI coloring.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RiskLevel {
    Safe,
    Medium,
    High,
    Critical,
}

impl RiskLevel {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Safe => "safe",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Critical => "critical",
        }
    }
}

// ── Enforcer ────────────────────────────────────────────────────────────

/// Stateless permission enforcer. Evaluates the 3-level pipeline.
pub struct PermissionEnforcer {
    mode: PermissionMode,
    workspace_root: Option<String>,
}

impl PermissionEnforcer {
    pub fn new(mode: PermissionMode, workspace_root: Option<String>) -> Self {
        Self { mode, workspace_root }
    }

    /// Main enforcement pipeline for any tool call.
    ///
    /// 1. Check persistent allow/deny rules
    /// 2. Check workspace boundary (for file operations)
    /// 3. Check mode vs required permission
    /// 4. If bash: run BashValidator pipeline
    pub fn check(
        &self,
        tool_name: &str,
        tool_permission: ToolPermission,
        arguments: &str,
    ) -> (EnforcementResult, Option<ValidationDetail>) {
        // Level 0: ReadOnly es un techo duro — bloquea operaciones de escritura
        // antes de que las reglas persistentes de allow puedan sobrescribirlo.
        let required = required_mode_for(tool_permission);
        if self.mode == PermissionMode::ReadOnly && !self.mode.satisfies(required.to_mode()) {
            return (
                EnforcementResult::Deny {
                    reason: format!(
                        "{tool_name}: modo ReadOnly no permite operaciones de escritura"
                    ),
                },
                None,
            );
        }

        let rules = PermissionRules::load();

        // Level 1: persistent per-tool rules
        match rules.check(tool_name) {
            Some(PermissionOverride::Allow) => return (EnforcementResult::Allow, None),
            Some(PermissionOverride::Deny) => {
                return (
                    EnforcementResult::Deny {
                        reason: format!("{tool_name}: regla persistente → deny"),
                    },
                    None,
                );
            }
            None => {}
        }

        // Level 2: workspace boundary check (for file-path tools)
        if let Some(detail) = self.check_workspace_boundary(tool_name, arguments) {
            return (EnforcementResult::Deny { reason: detail }, None);
        }

        // Level 3: mode-based check
        if self.mode == PermissionMode::FullAccess {
            return (EnforcementResult::Allow, None);
        }

        // Level 4: bash-specific validation
        if is_bash_tool(tool_name) {
            return self.check_bash(arguments);
        }

        // Standard mode check
        if self.mode.satisfies(required.to_mode()) {
            match tool_permission {
                ToolPermission::Safe => (EnforcementResult::Allow, None),
                ToolPermission::Ask => (
                    EnforcementResult::PromptUser {
                        reason: format!("{tool_name}: requiere aprobación"),
                    },
                    None,
                ),
                ToolPermission::Dangerous => (
                    EnforcementResult::PromptUser {
                        reason: format!("{tool_name}: operación peligrosa"),
                    },
                    Some(ValidationDetail {
                        risk_level: RiskLevel::High,
                        reasons: vec!["Operación potencialmente destructiva".into()],
                    }),
                ),
            }
        } else {
            (
                EnforcementResult::Deny {
                    reason: format!(
                        "{tool_name}: modo actual ({:?}) insuficiente, requiere {:?}",
                        self.mode, required
                    ),
                },
                None,
            )
        }
    }

    /// Bash-specific validation delegating to BashValidator.
    fn check_bash(&self, arguments: &str) -> (EnforcementResult, Option<ValidationDetail>) {
        let command = extract_command_from_args(arguments);
        let validation = crate::services::bash::validate_command(
            &command,
            self.workspace_root.as_deref(),
            self.mode,
        );

        match validation {
            BashValidation::Allow => (
                EnforcementResult::Allow,
                Some(ValidationDetail {
                    risk_level: RiskLevel::Safe,
                    reasons: vec!["Comando de solo lectura".into()],
                }),
            ),
            BashValidation::Warn { reasons } => (
                EnforcementResult::PromptUser {
                    reason: reasons.first().cloned().unwrap_or_default(),
                },
                Some(ValidationDetail { risk_level: RiskLevel::Medium, reasons }),
            ),
            BashValidation::Block { reasons } => (
                EnforcementResult::Deny { reason: reasons.first().cloned().unwrap_or_default() },
                Some(ValidationDetail { risk_level: RiskLevel::Critical, reasons }),
            ),
        }
    }

    /// Check if a file path is within the workspace root.
    fn check_workspace_boundary(&self, tool_name: &str, arguments: &str) -> Option<String> {
        if !is_file_tool(tool_name) {
            return None;
        }
        let root = self.workspace_root.as_ref()?;
        let path = extract_file_path(arguments)?;
        let resolved = resolve_path(&path);
        if !resolved.starts_with(root) {
            Some(format!("{tool_name}: path '{}' fuera del workspace '{}'", path, root))
        } else {
            None
        }
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────

fn required_mode_for(perm: ToolPermission) -> RequiredMode {
    match perm {
        ToolPermission::Safe => RequiredMode::ReadOnly,
        ToolPermission::Ask => RequiredMode::WorkspaceWrite,
        ToolPermission::Dangerous => RequiredMode::Prompt,
    }
}

fn is_bash_tool(name: &str) -> bool {
    matches!(name, "run_command" | "bash" | "execute" | "shell")
}

fn is_file_tool(name: &str) -> bool {
    matches!(
        name,
        "read_file" | "write_file" | "list_directory" | "search_files" | "grep_files" | "edit_file"
    )
}

/// Extract the command string from tool arguments JSON.
fn extract_command_from_args(arguments: &str) -> String {
    serde_json::from_str::<serde_json::Value>(arguments)
        .ok()
        .and_then(|v| v.get("command").and_then(|c| c.as_str()).map(String::from))
        .unwrap_or_default()
}

/// Extract file_path from tool arguments JSON.
fn extract_file_path(arguments: &str) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(arguments).ok().and_then(|v| {
        v.get("file_path")
            .or_else(|| v.get("path"))
            .or_else(|| v.get("directory"))
            .and_then(|p| p.as_str())
            .map(String::from)
    })
}

/// Resolve relative paths and normalize.
fn resolve_path(path: &str) -> String {
    let p = Path::new(path);
    if p.is_absolute() {
        normalize_path(p)
    } else {
        let cwd = std::env::current_dir().unwrap_or_default();
        normalize_path(&cwd.join(p))
    }
}

/// Normalize path by resolving `.` and `..` components.
fn normalize_path(path: &Path) -> String {
    let mut parts: Vec<String> = Vec::new();
    for component in path.components() {
        match component {
            std::path::Component::RootDir => {} // handled by is_absolute prefix
            std::path::Component::ParentDir => {
                parts.pop();
            }
            std::path::Component::CurDir => {}
            other => parts.push(other.as_os_str().to_string_lossy().to_string()),
        }
    }
    if path.is_absolute() {
        format!("/{}", parts.join("/"))
    } else {
        parts.join("/")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_access_allows_everything() {
        let enforcer = PermissionEnforcer::new(PermissionMode::FullAccess, None);
        let (result, _) = enforcer.check("write_file", ToolPermission::Ask, "{}");
        assert_eq!(result, EnforcementResult::Allow);
    }

    #[test]
    fn read_only_denies_write_tools() {
        let enforcer = PermissionEnforcer::new(PermissionMode::ReadOnly, None);
        let (result, _) = enforcer.check("write_file", ToolPermission::Ask, "{}");
        assert!(matches!(result, EnforcementResult::Deny { .. }));
    }

    #[test]
    fn workspace_boundary_blocks_outside_paths() {
        let enforcer = PermissionEnforcer::new(
            PermissionMode::WorkspaceWrite,
            Some("/home/user/project".into()),
        );
        let args = r#"{"file_path":"/etc/passwd"}"#;
        let (result, _) = enforcer.check("read_file", ToolPermission::Safe, args);
        assert!(matches!(result, EnforcementResult::Deny { .. }));
    }

    #[test]
    fn workspace_boundary_allows_inside_paths() {
        let enforcer = PermissionEnforcer::new(
            PermissionMode::WorkspaceWrite,
            Some("/home/user/project".into()),
        );
        let args = r#"{"file_path":"/home/user/project/src/main.rs"}"#;
        let (result, _) = enforcer.check("read_file", ToolPermission::Safe, args);
        assert_eq!(result, EnforcementResult::Allow);
    }

    #[test]
    fn safe_tool_auto_allowed_in_workspace_mode() {
        let enforcer = PermissionEnforcer::new(PermissionMode::WorkspaceWrite, None);
        let (result, _) = enforcer.check("read_file", ToolPermission::Safe, "{}");
        assert_eq!(result, EnforcementResult::Allow);
    }

    #[test]
    fn normalize_path_resolves_parent() {
        assert_eq!(normalize_path(Path::new("/a/b/../c")), "/a/c");
        assert_eq!(normalize_path(Path::new("/a/./b/c")), "/a/b/c");
    }
}
