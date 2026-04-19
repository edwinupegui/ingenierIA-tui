use std::fs;
use std::path::{Path, PathBuf};

use crate::domain::init_types::{InitClient, InitFileResult, ProjectType};
use crate::services::init_templates::{generate_claude_md, generate_copilot_md};

// ── Ejecución del init ───────────────────────────────────────────────────────

pub fn run_init(
    dir: &Path,
    server_url: &str,
    client: &InitClient,
    project_type: &ProjectType,
) -> anyhow::Result<Vec<InitFileResult>> {
    let mcp_url = format!("{}/claude/sse", server_url.trim_end_matches('/'));
    let mut results = Vec::new();

    let includes_claude = !matches!(client, InitClient::Copilot);
    let includes_copilot = !matches!(client, InitClient::Claude);

    // ── Claude Code files ────────────────────────────────────────────────────
    if includes_claude {
        generate_claude_files(dir, &mcp_url, project_type, &mut results);
    }

    // ── GitHub Copilot files ─────────────────────────────────────────────────
    if includes_copilot {
        generate_copilot_files(dir, &mcp_url, project_type, &mut results);
    }

    // ── Directorios .cloud/ ──────────────────────────────────────────────────
    create_cloud_dirs(dir, &mut results)?;

    // ── .gitignore ───────────────────────────────────────────────────────────
    update_gitignore(dir, includes_claude, &mut results)?;

    Ok(results)
}

fn generate_claude_files(
    dir: &Path,
    mcp_url: &str,
    project_type: &ProjectType,
    results: &mut Vec<InitFileResult>,
) {
    // .mcp.json
    let mcp_json = format!(
        r#"{{
  "mcpServers": {{
    "ingenieria": {{
      "type": "sse",
      "url": "{mcp_url}"
    }}
  }}
}}"#
    );
    results.push(write_safe(dir, ".mcp.json", &mcp_json, "Conexión MCP para Claude Code"));

    // CLAUDE.md
    let claude_md = generate_claude_md(project_type, mcp_url);
    results.push(write_safe(dir, "CLAUDE.md", &claude_md, "ingenierIA seed para Claude Code"));

    // .claude/commands/ingenieria-sync.md
    let sync_cmd = r#"Check for updates from the ingenierIA MCP Server and re-bootstrap if needed.

## Steps

1. Call MCP tool: sync_project(factory: "<current factory>")
   - If you don't know the current factory, check the existing CLAUDE.md or ask the user
2. Review the sync report
3. If there are updates:
   - Inform the user what changed (new/updated policies, ADRs, skills)
   - Ask: "Do you want to re-bootstrap to get the latest changes?"
   - If yes: call bootstrap_project and write all returned files
   - If no: just call get_factory_context to load the latest context for this session
4. If everything is up to date, confirm to the user
"#;
    results.push(write_safe(
        dir,
        ".claude/commands/ingenieria-sync.md",
        sync_cmd,
        "Comando /ingenieria-sync",
    ));
}

fn generate_copilot_files(
    dir: &Path,
    mcp_url: &str,
    project_type: &ProjectType,
    results: &mut Vec<InitFileResult>,
) {
    // .vscode/mcp.json
    let vscode_mcp = format!(
        r#"{{
  "servers": {{
    "ingenieria": {{
      "type": "sse",
      "url": "{mcp_url}"
    }}
  }}
}}"#
    );
    results.push(write_safe(
        dir,
        ".vscode/mcp.json",
        &vscode_mcp,
        "Conexión MCP para GitHub Copilot",
    ));

    // .github/copilot-instructions.md
    let copilot_md = generate_copilot_md(project_type, mcp_url);
    results.push(write_safe(
        dir,
        ".github/copilot-instructions.md",
        &copilot_md,
        "ingenierIA seed para GitHub Copilot",
    ));

    // .github/copilot/ingenieria-sync.prompt.md
    let copilot_sync = r#"---
description: Sincronizar con ingenierIA MCP Server y recargar contexto
mode: agent
---

Check for updates from the ingenierIA MCP Server and reload context if needed.

## Steps

1. Call MCP tool: sync_project(factory: "<current factory>")
   - If you don't know the current factory, check .github/copilot-instructions.md or ask the user
2. Review the sync report
3. If there are updates:
   - Inform the user what changed (new/updated policies, ADRs, skills)
   - Ask: "Do you want to reload the latest changes?"
   - If yes: call get_factory_context(factory) to load the latest context for this session
4. If everything is up to date, confirm to the user
"#;
    results.push(write_safe(
        dir,
        ".github/copilot/ingenieria-sync.prompt.md",
        copilot_sync,
        "Prompt ingenieria-sync para Copilot",
    ));
}

fn create_cloud_dirs(dir: &Path, results: &mut Vec<InitFileResult>) -> anyhow::Result<()> {
    let cloud_dirs =
        [".cloud/contracts", ".cloud/planning", ".cloud/architecture/decisions", ".cloud/audit"];
    for d in &cloud_dirs {
        let full = dir.join(d);
        if !full.exists() {
            fs::create_dir_all(&full).map_err(|e| anyhow::anyhow!("Error creando {d}: {e}"))?;
            let _ = fs::write(full.join(".gitkeep"), ""); // .gitkeep es opcional, ignorar fallo
        }
    }
    results.push(InitFileResult {
        path: ".cloud/".to_string(),
        created: true,
        description: "Directorios de artefactos".to_string(),
    });
    Ok(())
}

fn update_gitignore(
    dir: &Path,
    includes_claude: bool,
    results: &mut Vec<InitFileResult>,
) -> anyhow::Result<()> {
    let gi_path = dir.join(".gitignore");
    let mut gi_addition = "\n# ingenierIA local settings\n".to_string();
    if includes_claude {
        gi_addition.push_str(".claude/settings.local.json\n");
    }

    if gi_path.exists() {
        let current = fs::read_to_string(&gi_path).unwrap_or_default();
        if !current.contains("ingenierIA local settings") {
            fs::write(&gi_path, format!("{current}{gi_addition}"))
                .map_err(|e| anyhow::anyhow!("Error actualizando .gitignore: {e}"))?;
            results.push(InitFileResult {
                path: ".gitignore".to_string(),
                created: true,
                description: "Actualizado".to_string(),
            });
        } else {
            results.push(InitFileResult {
                path: ".gitignore".to_string(),
                created: false,
                description: "Ya configurado".to_string(),
            });
        }
    } else {
        fs::write(&gi_path, gi_addition.trim_start())
            .map_err(|e| anyhow::anyhow!("Error creando .gitignore: {e}"))?;
        results.push(InitFileResult {
            path: ".gitignore".to_string(),
            created: true,
            description: "Creado".to_string(),
        });
    }
    Ok(())
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn write_safe(dir: &Path, rel_path: &str, content: &str, description: &str) -> InitFileResult {
    let full: PathBuf = dir.join(rel_path);
    if full.exists() {
        return InitFileResult {
            path: rel_path.to_string(),
            created: false,
            description: format!("{description} (ya existe)"),
        };
    }
    if let Some(parent) = full.parent() {
        let _ = fs::create_dir_all(parent);
    }
    match fs::write(&full, content) {
        Ok(()) => InitFileResult {
            path: rel_path.to_string(),
            created: true,
            description: description.to_string(),
        },
        Err(e) => InitFileResult {
            path: rel_path.to_string(),
            created: false,
            description: format!("Error: {e}"),
        },
    }
}
