//! Bash tool: ejecuta shell commands con validadores de seguridad.
//!
//! El pipeline de validación vive en `crates/ingenieria-tools/src/bash/` y
//! se invoca desde `PermissionEnforcer::check_bash` ANTES de llegar aquí.
//! Por eso este módulo asume que cualquier comando que llega ya pasó por
//! los validators (destructive, network, workspace boundary, secrets, etc.)
//! o fue explícitamente aprobado por el usuario.
//!
//! Responsabilidad: spawn del proceso, timeout, captura stdout/stderr, exit
//! code, truncación de output. Sin lógica de política.
use std::time::Duration;

use serde::Deserialize;
use tokio::process::Command;
use tokio::time::timeout;

use super::{Tool, ToolPermission};
use crate::services::chat::ToolDefinition;

const DEFAULT_TIMEOUT_MS: u64 = 120_000; // 2 min
const MAX_TIMEOUT_MS: u64 = 600_000; // 10 min
const MAX_OUTPUT_BYTES: usize = 30_000;

pub struct BashTool;

#[derive(Deserialize)]
struct BashArgs {
    command: String,
    timeout_ms: Option<u64>,
    #[serde(default)]
    description: Option<String>,
}

#[async_trait::async_trait]
impl Tool for BashTool {
    fn name(&self) -> &str {
        "bash"
    }

    fn permission(&self) -> ToolPermission {
        ToolPermission::Ask
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            json: serde_json::json!({
                "type": "function",
                "function": {
                    "name": "bash",
                    "description": "Ejecuta un comando shell en el workspace. Bloqueado para comandos destructivos, network egress fuera del workspace, secretos o paths fuera del sandbox. Usa `description` para explicar brevemente qué hace el comando.",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "command": { "type": "string", "description": "Comando a ejecutar (se pasa a `sh -c`)" },
                            "timeout_ms": { "type": "integer", "description": "Timeout en ms (default 120000, max 600000)" },
                            "description": { "type": "string", "description": "Descripción corta del propósito del comando" }
                        },
                        "required": ["command"]
                    }
                }
            }),
        }
    }

    async fn execute(&self, arguments: &str) -> String {
        let args: BashArgs = match serde_json::from_str(arguments) {
            Ok(a) => a,
            Err(e) => return format!("Error parsing arguments: {e}"),
        };
        let requested = args.timeout_ms.unwrap_or(DEFAULT_TIMEOUT_MS).min(MAX_TIMEOUT_MS);
        run_bash(&args.command, Duration::from_millis(requested), args.description.as_deref()).await
    }
}

async fn run_bash(command: &str, to: Duration, description: Option<&str>) -> String {
    let spawn_result = Command::new("sh")
        .arg("-c")
        .arg(command)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true)
        .output();

    let output = match timeout(to, spawn_result).await {
        Ok(Ok(out)) => out,
        Ok(Err(e)) => return format!("Error ejecutando comando: {e}"),
        Err(_) => {
            return format!("Error: timeout después de {}ms ejecutando el comando", to.as_millis());
        }
    };

    let stdout = truncate_output(&String::from_utf8_lossy(&output.stdout));
    let stderr = truncate_output(&String::from_utf8_lossy(&output.stderr));
    let code = output.status.code().map(|c| c.to_string()).unwrap_or_else(|| "signal".into());
    format_output(command, description, &stdout, &stderr, &code, output.status.success())
}

fn truncate_output(s: &str) -> String {
    if s.len() <= MAX_OUTPUT_BYTES {
        return s.to_string();
    }
    let end = s
        .char_indices()
        .map(|(i, _)| i)
        .take_while(|&i| i <= MAX_OUTPUT_BYTES.saturating_sub(3))
        .last()
        .unwrap_or(0);
    format!("{}…\n[output truncado a {MAX_OUTPUT_BYTES} bytes]", &s[..end])
}

fn format_output(
    command: &str,
    description: Option<&str>,
    stdout: &str,
    stderr: &str,
    code: &str,
    success: bool,
) -> String {
    let status = if success { "✓ ok" } else { "✗ fallo" };
    let mut out = String::with_capacity(stdout.len() + stderr.len() + 256);
    out.push_str(&format!("$ {command}\n"));
    if let Some(desc) = description {
        if !desc.trim().is_empty() {
            out.push_str(&format!("# {desc}\n"));
        }
    }
    out.push_str(&format!("status: {status} (exit={code})\n"));
    if !stdout.trim().is_empty() {
        out.push_str("---stdout---\n");
        out.push_str(stdout);
        if !stdout.ends_with('\n') {
            out.push('\n');
        }
    }
    if !stderr.trim().is_empty() {
        out.push_str("---stderr---\n");
        out.push_str(stderr);
        if !stderr.ends_with('\n') {
            out.push('\n');
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn echo_returns_stdout() {
        let tool = BashTool;
        let args = r#"{"command":"echo hello"}"#;
        let out = tool.execute(args).await;
        assert!(out.contains("hello"));
        assert!(out.contains("exit=0"));
    }

    #[tokio::test]
    async fn timeout_kills_long_command() {
        let tool = BashTool;
        let args = r#"{"command":"sleep 5","timeout_ms":200}"#;
        let out = tool.execute(args).await;
        assert!(out.to_lowercase().contains("timeout"), "expected timeout, got: {out}");
    }

    #[tokio::test]
    async fn nonzero_exit_reports_failure() {
        let tool = BashTool;
        let args = r#"{"command":"false"}"#;
        let out = tool.execute(args).await;
        assert!(out.contains("fallo"));
        assert!(out.contains("exit=1"));
    }

    #[tokio::test]
    async fn invalid_json_returns_error() {
        let tool = BashTool;
        let out = tool.execute("{not json").await;
        assert!(out.starts_with("Error parsing arguments"));
    }

    #[test]
    fn truncate_keeps_short_strings() {
        let s = "x".repeat(100);
        assert_eq!(truncate_output(&s).len(), 100);
    }

    #[test]
    fn truncate_limits_long_strings() {
        let s = "x".repeat(MAX_OUTPUT_BYTES + 1000);
        let out = truncate_output(&s);
        assert!(out.contains("truncado"));
        assert!(out.len() < s.len());
    }
}
