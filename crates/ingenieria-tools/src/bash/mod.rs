//! Bash command validation pipeline.
//!
//! Classifies commands by intent and runs a chain of validators
//! to determine if execution should be allowed, warned, or blocked.

mod validators;

use ingenieria_runtime::permissions::PermissionMode;

// Re-export domain type for backward compatibility.
pub use ingenieria_domain::permissions::BashValidation;

// ── Command intent classification ───────────────────────────────────────

/// Semantic classification of a shell command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandIntent {
    ReadOnly,
    Write,
    Destructive,
    Network,
    ProcessManagement,
    PackageManagement,
    Unknown,
}

/// Run the full validation pipeline on a command string.
pub fn validate_command(
    command: &str,
    workspace_root: Option<&str>,
    mode: PermissionMode,
) -> BashValidation {
    let command = command.trim();
    if command.is_empty() {
        return BashValidation::Block { reasons: vec!["Comando vacío".into()] };
    }

    let first_cmd = extract_first_command(command);

    // In ReadOnly mode, only allow read-only commands
    if mode == PermissionMode::ReadOnly && !validators::readonly::is_read_only(&first_cmd) {
        return BashValidation::Block {
            reasons: vec![format!(
                "Modo ReadOnly: '{}' no es un comando de solo lectura",
                first_cmd
            )],
        };
    }

    let mut all_reasons = Vec::new();
    let mut is_blocked = false;

    // Validator 1: destructive commands
    if let Some(reasons) = validators::destructive::check(command) {
        is_blocked = true;
        all_reasons.extend(reasons);
    }

    // Validator 2: path traversal
    if let Some(reasons) = validators::path_traversal::check(command) {
        all_reasons.extend(reasons);
    }

    // Validator 3: workspace boundary
    if let Some(reasons) = validators::workspace_boundary::check(command, workspace_root) {
        all_reasons.extend(reasons);
    }

    // Validator 4: sed safety
    if let Some(reasons) = validators::sed_safety::check(command) {
        all_reasons.extend(reasons);
    }

    // Validator 5: network safety
    if let Some(reasons) = validators::network_safety::check(command) {
        all_reasons.extend(reasons);
    }

    // Validator 6: secrets detection
    if let Some(reasons) = validators::secrets_detect::check(command) {
        all_reasons.extend(reasons);
    }

    if is_blocked {
        BashValidation::Block { reasons: all_reasons }
    } else if all_reasons.is_empty() {
        // Validator 7: auto-allow read-only commands
        if validators::readonly::is_read_only(&first_cmd) {
            BashValidation::Allow
        } else {
            BashValidation::Warn { reasons: vec![format!("'{}': requiere aprobación", first_cmd)] }
        }
    } else {
        BashValidation::Warn { reasons: all_reasons }
    }
}

/// Extract the first command from a pipeline, ignoring env vars.
fn extract_first_command(command: &str) -> String {
    let cmd = command.trim();

    // Skip sudo
    let cmd = cmd.strip_prefix("sudo ").unwrap_or(cmd).trim();

    // Skip env var assignments (FOO=bar cmd ...)
    let parts: Vec<&str> = cmd.split_whitespace().collect();
    for part in &parts {
        if !part.contains('=') || part.starts_with('-') {
            return part.to_lowercase();
        }
    }
    parts.first().map(|s| s.to_lowercase()).unwrap_or_default()
}

/// Classify a command's intent based on the first command word.
pub fn classify_command(command: &str) -> CommandIntent {
    let first = extract_first_command(command);
    if validators::readonly::is_read_only(&first) {
        CommandIntent::ReadOnly
    } else if validators::destructive::is_always_destructive(&first) {
        CommandIntent::Destructive
    } else if validators::network_safety::is_network_command(&first) {
        CommandIntent::Network
    } else if is_process_command(&first) {
        CommandIntent::ProcessManagement
    } else if is_package_command(&first) {
        CommandIntent::PackageManagement
    } else if is_write_command(&first) {
        CommandIntent::Write
    } else {
        CommandIntent::Unknown
    }
}

fn is_process_command(cmd: &str) -> bool {
    matches!(cmd, "kill" | "killall" | "pkill" | "nohup" | "disown")
}

fn is_package_command(cmd: &str) -> bool {
    matches!(cmd, "npm" | "yarn" | "pnpm" | "pip" | "cargo" | "apt" | "brew" | "dnf" | "pacman")
}

fn is_write_command(cmd: &str) -> bool {
    matches!(cmd, "cp" | "mv" | "mkdir" | "touch" | "chmod" | "chown" | "ln" | "install")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_first_skips_env_vars() {
        assert_eq!(extract_first_command("FOO=bar cargo test"), "cargo");
        assert_eq!(extract_first_command("RUST_LOG=debug cargo run"), "cargo");
    }

    #[test]
    fn extract_first_strips_sudo() {
        assert_eq!(extract_first_command("sudo rm -rf /"), "rm");
    }

    #[test]
    fn empty_command_is_blocked() {
        let result = validate_command("", None, PermissionMode::WorkspaceWrite);
        assert!(matches!(result, BashValidation::Block { .. }));
    }

    #[test]
    fn ls_is_allowed() {
        let result = validate_command("ls -la", None, PermissionMode::WorkspaceWrite);
        assert_eq!(result, BashValidation::Allow);
    }

    #[test]
    fn read_only_mode_blocks_writes() {
        let result = validate_command("cargo build", None, PermissionMode::ReadOnly);
        assert!(matches!(result, BashValidation::Block { .. }));
    }
}
