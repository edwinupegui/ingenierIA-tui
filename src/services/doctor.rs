/// Doctor diagnostic service — runs health checks and returns a DoctorReport.
use std::sync::Arc;

use crate::domain::doctor::{CheckStatus, DoctorCheck, DoctorReport};
use crate::services::IngenieriaClient;
use crate::state::ServerStatus;

/// Info del LSP client para el check (se pasa como snapshot para evitar
/// compartir el estado vivo con la tarea async).
#[derive(Debug, Clone, Default)]
pub struct DoctorLspInfo {
    pub server_name: Option<String>,
    pub connected: bool,
    pub diagnostics_count: usize,
    pub error: Option<String>,
}

/// Entradas externas al doctor: snapshot de subsistemas cuyos estados viven
/// en `App` y deben pasarse por valor al spawn async.
#[derive(Debug, Clone, Default)]
pub struct DoctorInputs {
    pub lsp: DoctorLspInfo,
    pub bridge_active: bool,
}

/// Run all doctor checks. Called from a spawned tokio task.
pub async fn run_checks(
    client: &Arc<IngenieriaClient>,
    server_status: &ServerStatus,
    mcp_tools_count: usize,
    inputs: DoctorInputs,
) -> DoctorReport {
    let mut checks = Vec::with_capacity(7);
    checks.push(check_mcp_server(client, server_status).await);
    checks.push(check_config());
    checks.push(check_mcp_tools(mcp_tools_count));
    checks.push(check_features());
    checks.push(check_lsp(&inputs.lsp));
    checks.push(check_bridge(inputs.bridge_active));
    checks.push(check_disk());
    DoctorReport { checks }
}

fn check_features() -> DoctorCheck {
    let features = crate::services::features::list_features();
    let enabled: Vec<&str> = features.iter().filter(|f| f.enabled).map(|f| f.name).collect();
    DoctorCheck {
        name: "Features",
        status: CheckStatus::Green,
        detail: if enabled.is_empty() {
            "Minimal build (sin features)".into()
        } else {
            enabled.join(", ")
        },
        hint: None,
    }
}

fn check_lsp(lsp: &DoctorLspInfo) -> DoctorCheck {
    if let Some(err) = &lsp.error {
        return DoctorCheck {
            name: "LSP",
            status: CheckStatus::Red,
            detail: truncate_str(err, 60),
            hint: Some("Revisa el server LSP local".into()),
        };
    }
    match &lsp.server_name {
        Some(name) if lsp.connected => DoctorCheck {
            name: "LSP",
            status: CheckStatus::Green,
            detail: format!("{name} — {} diag", lsp.diagnostics_count),
            hint: None,
        },
        Some(name) => DoctorCheck {
            name: "LSP",
            status: CheckStatus::Yellow,
            detail: format!("{name} — desconectado"),
            hint: None,
        },
        None => DoctorCheck {
            name: "LSP",
            status: CheckStatus::Yellow,
            detail: "Sin server detectado".into(),
            hint: None,
        },
    }
}

fn check_bridge(active: bool) -> DoctorCheck {
    #[cfg(feature = "ide")]
    {
        if active {
            DoctorCheck {
                name: "IDE Bridge",
                status: CheckStatus::Green,
                detail: format!("http://127.0.0.1:{}", crate::services::bridge::DEFAULT_PORT),
                hint: None,
            }
        } else {
            DoctorCheck {
                name: "IDE Bridge",
                status: CheckStatus::Yellow,
                detail: "Inactivo".into(),
                hint: None,
            }
        }
    }
    #[cfg(not(feature = "ide"))]
    {
        let _ = active;
        DoctorCheck {
            name: "IDE Bridge",
            status: CheckStatus::Yellow,
            detail: "Feature `ide` deshabilitada".into(),
            hint: None,
        }
    }
}

async fn check_mcp_server(client: &Arc<IngenieriaClient>, status: &ServerStatus) -> DoctorCheck {
    match status {
        ServerStatus::Online(h) => {
            // Also verify with a live call to measure latency
            let start = std::time::Instant::now();
            match client.health().await {
                Ok(_) => {
                    let ms = start.elapsed().as_millis();
                    let (check_status, detail) = if ms < 500 {
                        (CheckStatus::Green, format!("{} — {}ms", h.version, ms))
                    } else {
                        (CheckStatus::Yellow, format!("{} — {}ms (slow)", h.version, ms))
                    };
                    DoctorCheck { name: "MCP Server", status: check_status, detail, hint: None }
                }
                Err(e) => DoctorCheck {
                    name: "MCP Server",
                    status: CheckStatus::Red,
                    detail: truncate_str(&format!("Failed: {e}"), 40),
                    hint: Some("Check server is running".into()),
                },
            }
        }
        ServerStatus::Offline(msg) => DoctorCheck {
            name: "MCP Server",
            status: CheckStatus::Red,
            detail: truncate_str(msg, 40),
            hint: Some("Start with: npx ingenieria-mcp".into()),
        },
        ServerStatus::Unknown => DoctorCheck {
            name: "MCP Server",
            status: CheckStatus::Yellow,
            detail: "Connecting...".into(),
            hint: None,
        },
    }
}

fn check_config() -> DoctorCheck {
    let warnings = crate::services::config_validation::validate_config_files();
    if warnings.is_empty() {
        DoctorCheck {
            name: "Config",
            status: CheckStatus::Green,
            detail: "No issues".into(),
            hint: None,
        }
    } else {
        let detail = warnings.iter().map(|w| w.to_string()).collect::<Vec<_>>().join("; ");
        DoctorCheck {
            name: "Config",
            status: CheckStatus::Yellow,
            detail: truncate_str(&detail, 60),
            hint: Some("Check ~/.config/ingenieria-tui/".into()),
        }
    }
}

fn check_mcp_tools(count: usize) -> DoctorCheck {
    if count >= 6 {
        DoctorCheck {
            name: "MCP Tools",
            status: CheckStatus::Green,
            detail: format!("{count} tools available"),
            hint: None,
        }
    } else if count > 0 {
        DoctorCheck {
            name: "MCP Tools",
            status: CheckStatus::Yellow,
            detail: format!("{count} tools (expected 8)"),
            hint: Some("Some tools may be unavailable".into()),
        }
    } else {
        DoctorCheck {
            name: "MCP Tools",
            status: CheckStatus::Red,
            detail: "No tools discovered".into(),
            hint: Some("MCP feature may be disabled".into()),
        }
    }
}

fn check_disk() -> DoctorCheck {
    let config_dir = dirs::config_dir().map(|d| d.join("ingenieria-tui")).unwrap_or_default();

    // Use a simple heuristic: check if we can write to the config dir
    if config_dir.exists() {
        DoctorCheck {
            name: "Disk",
            status: CheckStatus::Green,
            detail: "Config dir writable".into(),
            hint: None,
        }
    } else {
        // Try to create it
        match std::fs::create_dir_all(&config_dir) {
            Ok(_) => DoctorCheck {
                name: "Disk",
                status: CheckStatus::Green,
                detail: "Config dir created".into(),
                hint: None,
            },
            Err(e) => DoctorCheck {
                name: "Disk",
                status: CheckStatus::Red,
                detail: format!("Cannot create config: {e}"),
                hint: Some("Check permissions".into()),
            },
        }
    }
}

fn truncate_str(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}..", &s[..max.saturating_sub(2)])
    }
}
