//! Detects operations that expose secrets: .env, credentials, tokens.

/// Check for operations on secret files. Returns warn reasons.
pub fn check(command: &str) -> Option<Vec<String>> {
    let mut reasons = Vec::new();
    let lower = command.to_lowercase();

    // .env files
    for pattern in SECRET_FILE_PATTERNS {
        if lower.contains(pattern) {
            reasons.push(format!("Archivo con secretos detectado: {}", pattern));
            break;
        }
    }

    // Inline credentials in args
    if has_inline_credential(&lower) {
        reasons.push("Credencial detectada en el comando (usa variables de entorno)".into());
    }

    // Displaying or piping env to network
    if lower.contains("env |") || lower.contains("printenv |") {
        reasons.push("Volcado de variables de entorno a pipe".into());
    }

    if reasons.is_empty() {
        None
    } else {
        Some(reasons)
    }
}

const SECRET_FILE_PATTERNS: &[&str] = &[
    ".env",
    "credentials.json",
    "credentials.yaml",
    "id_rsa",
    "id_ed25519",
    "id_ecdsa",
    ".netrc",
    ".pgpass",
    "service-account.json",
    ".npmrc",
    ".pypirc",
];

fn has_inline_credential(lower: &str) -> bool {
    // Heuristics for inline tokens
    let patterns = ["password=", "token=", "api_key=", "apikey=", "secret=", "-p password"];
    patterns.iter().any(|p| lower.contains(p))
        && !lower.contains("password=${")
        && !lower.contains("password=\"$")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_file_flagged() {
        assert!(check("cat .env").is_some());
        assert!(check("cp .env /tmp/").is_some());
    }

    #[test]
    fn ssh_key_flagged() {
        assert!(check("cat ~/.ssh/id_rsa").is_some());
    }

    #[test]
    fn inline_password_flagged() {
        assert!(check("mysql -u root password=secret123").is_some());
    }

    #[test]
    fn env_var_ref_ok() {
        assert!(check("mysql password=${DB_PASS}").is_none());
    }

    #[test]
    fn safe_command_passes() {
        assert!(check("cargo test").is_none());
    }
}
