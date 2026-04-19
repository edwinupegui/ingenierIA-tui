use serde::Deserialize;

// Re-export auth types from the auth module for backwards compatibility.
pub use super::auth::copilot_auth::{delete_saved_auth, load_saved_auth, save_auth, CopilotAuth};

// ── GitHub OAuth App (Copilot) ───────────────────────────────────────────────

// Register your own GitHub OAuth App at https://github.com/settings/developers
const COPILOT_CLIENT_ID: &str = "YOUR_COPILOT_OAUTH_CLIENT_ID";
pub(crate) const EDITOR_VERSION: &str = "vscode/1.90.0";
pub(crate) const EDITOR_PLUGIN_VERSION: &str = "copilot-chat/0.17.0";

// ── API responses ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
#[expect(
    dead_code,
    reason = "all fields populated by GitHub OAuth deserialization, read by auth flow"
)]
pub struct DeviceCodeResponse {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub interval: u64,
}

#[derive(Debug, Clone, Deserialize)]
struct OAuthTokenResponse {
    access_token: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct CopilotTokenResponse {
    token: String,
    #[expect(dead_code, reason = "field populated by Copilot token API deserialization")]
    expires_at: u64,
}

#[derive(Debug, Clone, Deserialize)]
struct ModelsApiResponse {
    data: Vec<ModelEntry>,
}

#[derive(Debug, Clone, Deserialize)]
struct ModelEntry {
    id: String,
}

// ── Public model type ────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct CopilotModel {
    pub id: String,
    pub display_name: String,
}

// ── GitHub host helpers ──────────────────────────────────────────────────────

fn github_base(host: &str) -> String {
    format!("https://{host}")
}

const COPILOT_USER_AGENT: &str = "GithubCopilot/1.155.0";
const COPILOT_HTTP_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

pub(crate) fn http_client() -> reqwest::Client {
    use std::sync::OnceLock;
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT
        .get_or_init(|| {
            reqwest::Client::builder()
                .user_agent(COPILOT_USER_AGENT)
                .timeout(COPILOT_HTTP_TIMEOUT)
                .build()
                .expect("static Copilot HTTP client config must be valid")
        })
        .clone()
}

fn api_base(host: &str) -> String {
    if host == "github.com" {
        "https://api.github.com".to_string()
    } else {
        format!("https://{host}/api/v3")
    }
}

// ── Device code flow ─────────────────────────────────────────────────────────

/// Step 1: Request a device code from GitHub.
pub async fn request_device_code(host: &str) -> anyhow::Result<DeviceCodeResponse> {
    let url = format!("{}/login/device/code", github_base(host));
    let client = http_client();
    let resp = client
        .post(&url)
        .header("Accept", "application/json")
        .json(&serde_json::json!({
            "client_id": COPILOT_CLIENT_ID
        }))
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_else(|_| String::new());
        anyhow::bail!("GitHub device code request failed: {status} {body}");
    }

    Ok(resp.json().await?)
}

/// Step 2: Poll GitHub for the OAuth access token.
/// Returns `Ok(Some(token))` when authorized, `Ok(None)` if still pending,
/// or `Err` on a terminal error.
pub async fn poll_for_token(host: &str, device_code: &str) -> anyhow::Result<Option<String>> {
    let url = format!("{}/login/oauth/access_token", github_base(host));
    let client = http_client();
    let resp: OAuthTokenResponse = client
        .post(&url)
        .header("Accept", "application/json")
        .json(&serde_json::json!({
            "client_id": COPILOT_CLIENT_ID,
            "device_code": device_code,
            "grant_type": "urn:ietf:params:oauth:grant-type:device_code"
        }))
        .send()
        .await?
        .json()
        .await?;

    if let Some(token) = resp.access_token {
        return Ok(Some(token));
    }

    match resp.error.as_deref() {
        Some("authorization_pending") | Some("slow_down") => Ok(None),
        Some(err) => {
            let desc = resp.error_description.unwrap_or_default();
            anyhow::bail!("{err}: {desc}")
        }
        None => anyhow::bail!("Unexpected empty response from GitHub"),
    }
}

// ── Copilot token + models ───────────────────────────────────────────────────

/// Exchange the GitHub OAuth token for a short-lived Copilot token.
pub(crate) async fn get_copilot_token(host: &str, oauth_token: &str) -> anyhow::Result<String> {
    let url = format!("{}/copilot_internal/v2/token", api_base(host));
    let client = http_client();
    let resp = client
        .get(&url)
        .header("Authorization", format!("token {oauth_token}"))
        .header("Accept", "application/json")
        .header("Editor-Version", EDITOR_VERSION)
        .header("Editor-Plugin-Version", EDITOR_PLUGIN_VERSION)
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_else(|_| String::new());
        anyhow::bail!("Copilot token request failed: {status} {body}");
    }

    let body: CopilotTokenResponse = resp.json().await?;
    Ok(body.token)
}

/// Fetch available models using the Copilot token.
pub async fn fetch_models(host: &str, oauth_token: &str) -> anyhow::Result<Vec<CopilotModel>> {
    let copilot_token = get_copilot_token(host, oauth_token).await?;
    let models = fetch_models_with_token(&copilot_token).await?;
    Ok(models)
}

async fn fetch_models_with_token(copilot_token: &str) -> anyhow::Result<Vec<CopilotModel>> {
    let client = http_client();
    let resp = client
        .get("https://api.githubcopilot.com/models")
        .header("Authorization", format!("Bearer {copilot_token}"))
        .header("Accept", "application/json")
        .header("Copilot-Integration-Id", "vscode-chat")
        .header("Editor-Version", EDITOR_VERSION)
        .header("Editor-Plugin-Version", EDITOR_PLUGIN_VERSION)
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_else(|_| String::new());
        anyhow::bail!("Copilot models request failed: {status} {body}");
    }

    let body: ModelsApiResponse = resp.json().await?;

    let mut models: Vec<CopilotModel> = body
        .data
        .into_iter()
        .map(|m| {
            let display_name = pretty_model_name(&m.id);
            CopilotModel { id: m.id, display_name }
        })
        .collect();

    models.sort_by(|a, b| a.display_name.cmp(&b.display_name));
    Ok(models)
}

/// Fetch models con retry: si el Copilot token esta expirado, obtiene uno nuevo.
pub async fn fetch_models_with_retry(
    host: &str,
    oauth_token: &str,
) -> anyhow::Result<Vec<CopilotModel>> {
    match fetch_models(host, oauth_token).await {
        Ok(models) => Ok(models),
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("401") || msg.contains("403") {
                // Token expirado, re-intentar con nuevo Copilot token
                tracing::info!("Copilot token expired, refreshing...");
                let new_token = get_copilot_token(host, oauth_token).await?;
                fetch_models_with_token(&new_token).await
            } else {
                Err(e)
            }
        }
    }
}

/// Convert model IDs like "claude-sonnet-4.6" to "Claude Sonnet 4.6".
fn pretty_model_name(id: &str) -> String {
    let parts: Vec<&str> = id.splitn(2, '-').collect();
    if parts.len() < 2 {
        return capitalize(id);
    }

    let vendor = parts[0];
    let rest = parts[1];

    match vendor.to_lowercase().as_str() {
        "claude" => format!("Claude {}", titlecase_rest(rest)),
        "gpt" => format!("GPT-{}", rest),
        "o1" | "o3" | "o4" => format!("{}-{}", vendor, rest),
        _ => titlecase_rest(id),
    }
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
}

fn titlecase_rest(s: &str) -> String {
    s.split('-').map(capitalize).collect::<Vec<_>>().join(" ")
}

/// Try to open the verification URL in the default browser.
pub fn open_browser(url: &str) {
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open").arg(url).spawn();
    }
    #[cfg(target_os = "linux")]
    {
        let _ = std::process::Command::new("xdg-open").arg(url).spawn();
    }
    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("cmd").args(["/C", "start", url]).spawn();
    }
}
