//! Ensures write operations stay within the workspace root.

/// Check if command writes outside the workspace. Returns warn reasons.
pub fn check(command: &str, workspace_root: Option<&str>) -> Option<Vec<String>> {
    let root = workspace_root?;

    let mut reasons = Vec::new();

    // Heuristic: look for absolute paths that don't start with root
    for word in command.split_whitespace() {
        let path = word.trim_matches(|c: char| c == '"' || c == '\'' || c == ';');
        if path.starts_with('/')
            && !path.starts_with(root)
            && !path.starts_with("/tmp/")
            && !path.starts_with("/dev/null")
            && !path.starts_with("/dev/stdin")
            && !path.starts_with("/dev/stdout")
            && !path.starts_with("/dev/stderr")
            && path.len() > 1
        {
            // Only flag if it looks like a filesystem op target
            if is_write_context(command) {
                reasons.push(format!("Path '{}' fuera del workspace '{}'", path, root));
                break;
            }
        }
    }

    if reasons.is_empty() {
        None
    } else {
        Some(reasons)
    }
}

fn is_write_context(command: &str) -> bool {
    let write_cmds = ["cp", "mv", "rm", "mkdir", "touch", "chmod", "chown", "dd", "tee", ">"];
    write_cmds.iter().any(|w| command.contains(w))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_outside_workspace_flagged() {
        let r = check("cp file.txt /etc/somewhere", Some("/home/user/proj"));
        assert!(r.is_some());
    }

    #[test]
    fn write_inside_workspace_ok() {
        let r = check("cp file.txt /home/user/proj/dest", Some("/home/user/proj"));
        assert!(r.is_none());
    }

    #[test]
    fn no_workspace_root_ok() {
        assert!(check("cp a /etc/b", None).is_none());
    }

    #[test]
    fn read_op_outside_not_flagged() {
        // Read ops don't trigger this validator (path_traversal handles sensitive reads)
        let r = check("cat /etc/hosts", Some("/home/user/proj"));
        assert!(r.is_none());
    }

    #[test]
    fn tmp_path_allowed() {
        assert!(check("cp a /tmp/b", Some("/home/user/proj")).is_none());
    }
}
