//! Read-only command whitelist: auto-allow safe inspection commands.

/// Returns true if a command is read-only and safe to auto-execute.
pub fn is_read_only(cmd: &str) -> bool {
    READ_ONLY_COMMANDS.contains(&cmd)
}

const READ_ONLY_COMMANDS: &[&str] = &[
    // File inspection
    "ls",
    "ll",
    "la",
    "cat",
    "head",
    "tail",
    "less",
    "more",
    "file",
    "stat",
    "wc",
    "md5sum",
    "sha256sum",
    "sha1sum",
    "cksum",
    // Text search
    "grep",
    "egrep",
    "fgrep",
    "rg",
    "ag",
    "ack",
    "find",
    "fd",
    "locate",
    // Path
    "pwd",
    "realpath",
    "readlink",
    "basename",
    "dirname",
    "which",
    "whereis",
    // System info
    "whoami",
    "id",
    "hostname",
    "uname",
    "uptime",
    "date",
    "cal",
    "df",
    "du",
    "free",
    "ps",
    "top",
    "htop",
    "env",
    "printenv",
    // Git read-only
    "git-log",
    "git-status",
    "git-diff",
    "git-show",
    "git-branch",
    // Package managers (list only)
    "npm-list",
    "pip-list",
    "cargo-tree",
    "cargo-search",
    // Process info
    "jobs",
    "bg",
    "fg",
    // Printing
    "echo",
    "printf",
    "yes",
    // Pagers
    "man",
    "info",
];

/// Check if a git subcommand is read-only.
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "used by intent classifier for git-aware routing")
)]
pub fn is_readonly_git_subcommand(sub: &str) -> bool {
    matches!(
        sub,
        "log"
            | "status"
            | "diff"
            | "show"
            | "branch"
            | "tag"
            | "remote"
            | "blame"
            | "config"
            | "rev-parse"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn common_reads_allowed() {
        assert!(is_read_only("ls"));
        assert!(is_read_only("cat"));
        assert!(is_read_only("grep"));
        assert!(is_read_only("pwd"));
    }

    #[test]
    fn write_commands_not_readonly() {
        assert!(!is_read_only("rm"));
        assert!(!is_read_only("mv"));
        assert!(!is_read_only("cp"));
        assert!(!is_read_only("cargo"));
    }

    #[test]
    fn git_subcommand_check() {
        assert!(is_readonly_git_subcommand("log"));
        assert!(!is_readonly_git_subcommand("push"));
    }
}
