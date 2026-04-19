// ── External skill installer (skills.sh) ────────────────────────────────────
//
// Installs skills from skills.sh via `npx skills add owner/repo --skill name`.
// Equivalent to autoskills-main installer.mjs, adapted for async Rust.

use tokio::process::Command;

/// Result of installing a single skill.
#[derive(Debug, Clone)]
pub struct InstallResult {
    pub skill_name: String,
    pub success: bool,
    pub output: String,
}

/// Result of installing all skills.
#[derive(Debug, Clone)]
pub struct InstallSummary {
    pub results: Vec<InstallResult>,
    pub installed: usize,
    pub failed: usize,
}

/// Detect which AI agents are available on this machine.
pub fn detect_agents() -> Vec<&'static str> {
    let mut agents = vec!["universal"];
    let Some(home) = dirs::home_dir() else {
        return agents;
    };

    let agent_folders: &[(&str, &str)] = &[
        (".claude", "claude-code"),
        (".cursor", "cursor"),
        (".cline", "cline"),
        (".codex", "codex"),
        (".copilot", "github-copilot"),
        (".augment", "augment"),
        (".gemini", "gemini-cli"),
        (".amp", "amp"),
    ];

    for &(folder, agent_name) in agent_folders {
        if home.join(folder).join("skills").is_dir() {
            agents.push(agent_name);
        }
    }

    agents
}

/// Parse "owner/repo/skill-name" into (repo, skill_name).
fn parse_skill_path(path: &str) -> (&str, &str) {
    let parts: Vec<&str> = path.splitn(3, '/').collect();
    if parts.len() == 3 {
        let repo_end = path.find('/').unwrap_or(0) + 1 + parts[1].len();
        (&path[..repo_end], parts[2])
    } else {
        (path, "")
    }
}

/// Build npx command args for installing a skill.
fn build_install_args(skill_path: &str, agents: &[&str]) -> Vec<String> {
    let (repo, skill_name) = parse_skill_path(skill_path);
    let mut args =
        vec!["-y".to_string(), "skills".to_string(), "add".to_string(), repo.to_string()];
    if !skill_name.is_empty() {
        args.push("--skill".to_string());
        args.push(skill_name.to_string());
    }
    args.push("-y".to_string());
    if !agents.is_empty() {
        args.push("-a".to_string());
        for agent in agents {
            args.push((*agent).to_string());
        }
    }
    args
}

/// Install a single skill via npx.
async fn install_one(skill_path: &str, agents: &[&str]) -> InstallResult {
    let args = build_install_args(skill_path, agents);
    let skill_name = skill_path.rsplit('/').next().unwrap_or(skill_path).to_string();

    let result = Command::new("npx").args(&args).output().await;

    match result {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let combined = format!("{stdout}{stderr}");
            InstallResult { skill_name, success: output.status.success(), output: combined }
        }
        Err(e) => {
            InstallResult { skill_name, success: false, output: format!("Error spawning npx: {e}") }
        }
    }
}

/// Install multiple skills concurrently (up to 4 at a time).
pub async fn install_all(skill_paths: &[String]) -> InstallSummary {
    let agents = detect_agents();
    let mut results = Vec::with_capacity(skill_paths.len());

    // Process in chunks of 4 for controlled concurrency
    for chunk in skill_paths.chunks(4) {
        let futures: Vec<_> = chunk.iter().map(|path| install_one(path, &agents)).collect();
        let chunk_results = futures_util::future::join_all(futures).await;
        results.extend(chunk_results);
    }

    let installed = results.iter().filter(|r| r.success).count();
    let failed = results.len() - installed;

    InstallSummary { results, installed, failed }
}

/// Format install summary as markdown.
pub fn format_summary(summary: &InstallSummary) -> String {
    let mut out = format!(
        "### Instalacion completada — {} instalados, {} fallidos\n\n",
        summary.installed, summary.failed
    );

    for r in &summary.results {
        if r.success {
            out.push_str(&format!("- **{}** instalado\n", r.skill_name));
        } else {
            out.push_str(&format!(
                "- **{}** fallo: {}\n",
                r.skill_name,
                r.output.lines().next().unwrap_or("error desconocido")
            ));
        }
    }

    out
}
