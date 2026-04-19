//! Catches unsafe sed usage: `sed -i` without backup extension.

/// Check for unsafe sed invocations. Returns warn reasons.
pub fn check(command: &str) -> Option<Vec<String>> {
    if !contains_sed(command) {
        return None;
    }

    let mut reasons = Vec::new();

    // sed -i without backup (.bak, "", etc.)
    if has_inplace_no_backup(command) {
        reasons.push("sed -i sin backup: cambios irreversibles. Considera usar sed -i.bak".into());
    }

    // sed with dangerous regex on sensitive files
    if command.contains("sed") && (command.contains("/etc/") || command.contains("/boot/")) {
        reasons.push("sed sobre archivos del sistema".into());
    }

    if reasons.is_empty() {
        None
    } else {
        Some(reasons)
    }
}

fn contains_sed(command: &str) -> bool {
    command.split_whitespace().any(|w| w == "sed")
        || command.contains(" sed ")
        || command.starts_with("sed ")
}

fn has_inplace_no_backup(command: &str) -> bool {
    // Match -i followed by space (no suffix) or -i followed by nothing
    // But NOT -i.bak or -i''
    let tokens: Vec<&str> = command.split_whitespace().collect();
    let mut i = 0;
    while i < tokens.len() {
        if tokens[i] == "sed" {
            // Look at next tokens
            for token in tokens.iter().skip(i + 1) {
                if *token == "-i" {
                    return true; // -i alone = no backup
                }
                if token.starts_with("-i") && token.len() > 2 {
                    // -i.bak, -i'' etc. — safe
                    return false;
                }
                if !token.starts_with('-') {
                    break;
                }
            }
        }
        i += 1;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sed_inplace_no_backup_flagged() {
        assert!(check("sed -i 's/a/b/' file.txt").is_some());
    }

    #[test]
    fn sed_inplace_with_backup_ok() {
        assert!(check("sed -i.bak 's/a/b/' file.txt").is_none());
    }

    #[test]
    fn sed_no_inplace_ok() {
        assert!(check("sed 's/a/b/' file.txt").is_none());
    }

    #[test]
    fn non_sed_command_ignored() {
        assert!(check("cargo build").is_none());
    }
}
