use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Persisted GitHub Copilot authentication credentials.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CopilotAuth {
    pub oauth_token: String,
    pub github_host: String,
}

fn copilot_auth_path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("ingenieria-tui").join("copilot_auth.json"))
}

pub fn load_saved_auth() -> Option<CopilotAuth> {
    let path = copilot_auth_path()?;
    let content = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

pub fn delete_saved_auth() -> anyhow::Result<()> {
    if let Some(path) = copilot_auth_path() {
        if path.exists() {
            std::fs::remove_file(&path)?;
        }
    }
    Ok(())
}

pub fn save_auth(auth: &CopilotAuth) -> anyhow::Result<()> {
    let path = copilot_auth_path().ok_or_else(|| anyhow::anyhow!("No config dir"))?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, serde_json::to_string_pretty(auth)?)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o600);
        std::fs::set_permissions(&path, perms)?;
    }

    Ok(())
}
