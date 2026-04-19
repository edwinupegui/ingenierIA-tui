//! Detects path traversal attempts: `../..`, `/etc/`, `~/.ssh/`, etc.

/// Check for suspicious path traversal patterns. Returns warn reasons.
pub fn check(command: &str) -> Option<Vec<String>> {
    let mut reasons = Vec::new();

    // Parent directory traversal with depth
    if count_parent_refs(command) >= 3 {
        reasons.push("Multiple '../' references (posible path traversal)".into());
    }

    // Sensitive system paths
    for path in SENSITIVE_PATHS {
        if command.contains(path) {
            reasons.push(format!("Acceso a path sensible: {}", path));
        }
    }

    // User dotfiles with secrets
    if command.contains("~/.ssh/") || command.contains("$HOME/.ssh/") || command.contains("/.ssh/")
    {
        reasons.push("Acceso a ~/.ssh/ (claves SSH)".into());
    }

    if command.contains("~/.aws/") || command.contains("/.aws/credentials") {
        reasons.push("Acceso a ~/.aws/ (credenciales AWS)".into());
    }

    if command.contains("~/.gnupg/") || command.contains("/.gnupg/") {
        reasons.push("Acceso a ~/.gnupg/ (claves GPG)".into());
    }

    if reasons.is_empty() {
        None
    } else {
        Some(reasons)
    }
}

const SENSITIVE_PATHS: &[&str] =
    &["/etc/shadow", "/etc/passwd", "/etc/sudoers", "/root/", "/boot/", "/sys/", "/proc/1/"];

fn count_parent_refs(command: &str) -> usize {
    command.matches("../").count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ssh_keys_flagged() {
        assert!(check("cat ~/.ssh/id_rsa").is_some());
    }

    #[test]
    fn etc_shadow_flagged() {
        assert!(check("cat /etc/shadow").is_some());
    }

    #[test]
    fn deep_traversal_flagged() {
        assert!(check("cat ../../../etc/passwd").is_some());
    }

    #[test]
    fn normal_path_passes() {
        assert!(check("cat src/main.rs").is_none());
    }

    #[test]
    fn single_parent_ok() {
        assert!(check("cat ../README.md").is_none());
    }
}
