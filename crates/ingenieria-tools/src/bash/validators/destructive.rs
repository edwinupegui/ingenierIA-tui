//! Detects destructive commands that should be blocked.
//!
//! Patterns: `rm -rf /`, `git reset --hard`, `drop table`, fork bombs, etc.

/// Check if a command matches destructive patterns. Returns reasons if blocked.
pub fn check(command: &str) -> Option<Vec<String>> {
    let mut reasons = Vec::new();
    let lower = command.to_lowercase();

    // Always-destructive commands
    let first = extract_first_word(&lower);
    if is_always_destructive(&first) {
        reasons.push(format!("'{}' es un comando destructivo", first));
    }

    // rm -rf with dangerous targets
    if lower.contains("rm ")
        && (lower.contains("-rf") || lower.contains("-r -f"))
        && has_dangerous_rm_target(&lower)
    {
        reasons.push("rm -rf sobre directorio raíz o sistema".into());
    }

    // git destructive operations
    if lower.contains("git") {
        check_git_destructive(&lower, &mut reasons);
    }

    // SQL destructive
    if lower.contains("drop ") && (lower.contains("table") || lower.contains("database")) {
        reasons.push("DROP TABLE/DATABASE detectado".into());
    }

    // Fork bomb patterns
    if lower.contains(":(){ :|:& };:") || lower.contains("./$0|./$0") {
        reasons.push("Fork bomb detectado".into());
    }

    // Disk wipe
    if lower.contains("/dev/sda") || lower.contains("/dev/nvme") || lower.contains("/dev/null >") {
        reasons.push("Escritura directa a dispositivo de bloque".into());
    }

    if reasons.is_empty() {
        None
    } else {
        Some(reasons)
    }
}

/// Commands that are always destructive regardless of arguments.
pub fn is_always_destructive(cmd: &str) -> bool {
    matches!(cmd, "shred" | "wipefs" | "mkfs" | "fdisk" | "parted" | "dd")
}

fn check_git_destructive(lower: &str, reasons: &mut Vec<String>) {
    if lower.contains("reset --hard") {
        reasons.push("git reset --hard: descarta cambios no commiteados".into());
    }
    if lower.contains("push --force") || lower.contains("push -f") {
        reasons.push("git push --force: puede sobrescribir historia remota".into());
    }
    if lower.contains("clean -f") || lower.contains("clean -df") {
        reasons.push("git clean -f: elimina archivos sin tracking".into());
    }
    if lower.contains("branch -D") {
        reasons.push("git branch -D: elimina rama sin merge check".into());
    }
}

fn has_dangerous_rm_target(lower: &str) -> bool {
    let dangerous = [" /", " /*", " ~/", " ~/*", " $HOME", " /etc", " /usr", " /var"];
    dangerous.iter().any(|t| lower.contains(t))
}

fn extract_first_word(s: &str) -> String {
    s.split_whitespace().next().unwrap_or("").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rm_rf_root_is_destructive() {
        assert!(check("rm -rf /").is_some());
        assert!(check("rm -rf /*").is_some());
    }

    #[test]
    fn rm_rf_local_not_blocked() {
        assert!(check("rm -rf ./target").is_none());
    }

    #[test]
    fn git_force_push_detected() {
        let r = check("git push --force origin main").unwrap();
        assert!(r.iter().any(|s| s.contains("push --force")));
    }

    #[test]
    fn shred_always_destructive() {
        assert!(check("shred secret.txt").is_some());
    }

    #[test]
    fn safe_command_passes() {
        assert!(check("cargo test").is_none());
    }
}
