//! Flags network commands piped into interpreters: `curl|bash`, `wget|sh`.

/// Check for dangerous pipe-to-shell patterns. Returns reasons.
pub fn check(command: &str) -> Option<Vec<String>> {
    let mut reasons = Vec::new();
    let lower = command.to_lowercase();

    // curl|bash or wget|sh patterns
    if has_pipe_to_interpreter(&lower) {
        reasons.push(
            "Descarga + ejecución directa (curl|bash): alto riesgo, revisa el script primero"
                .into(),
        );
    }

    // nc/netcat as server
    if (lower.contains("nc -l") || lower.contains("netcat -l"))
        && (lower.contains("-e") || lower.contains("bash") || lower.contains("sh"))
    {
        reasons.push("netcat como reverse shell detectado".into());
    }

    // SSH with identity forwarding to unknown host
    if lower.contains("ssh -a ") || lower.contains("ssh-agent") {
        // Just a note, not blocked
    }

    if reasons.is_empty() {
        None
    } else {
        Some(reasons)
    }
}

/// Check if a command is primarily a network operation.
pub fn is_network_command(cmd: &str) -> bool {
    matches!(cmd, "curl" | "wget" | "nc" | "netcat" | "ssh" | "scp" | "sftp" | "rsync" | "ftp")
}

fn has_pipe_to_interpreter(lower: &str) -> bool {
    let net_cmds = ["curl ", "wget "];
    let interpreters = ["| bash", "|bash", "| sh", "|sh", "| python", "|python", "| ruby", "|ruby"];

    let has_net = net_cmds.iter().any(|c| lower.contains(c));
    let has_interp = interpreters.iter().any(|i| lower.contains(i));
    has_net && has_interp
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn curl_bash_flagged() {
        assert!(check("curl https://example.com/install.sh | bash").is_some());
    }

    #[test]
    fn wget_sh_flagged() {
        assert!(check("wget -qO- https://x.com/s | sh").is_some());
    }

    #[test]
    fn curl_save_ok() {
        assert!(check("curl -O https://example.com/file.tar.gz").is_none());
    }

    #[test]
    fn reverse_shell_flagged() {
        assert!(check("nc -l -e /bin/bash 4444").is_some());
    }
}
