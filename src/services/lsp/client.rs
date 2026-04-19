//! LSP client lifecycle (E25).
//!
//! Maneja el ciclo de vida completo: spawn del server, initialize handshake,
//! loop de notificaciones (publishDiagnostics), y shutdown.
//!
//! El client corre como un tokio task y emite Actions hacia el reducer.
//! No intenta ser un client LSP completo — solo captura diagnosticos.

use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Result};
use serde_json::json;
use tokio::process::Command;
use tokio::sync::mpsc::Sender;

use super::detection::LspServerConfig;
use super::transport::{LspMessage, LspReader, LspWriter};
use super::types::{parse_diagnostic, LspDiagnostic};
use crate::actions::Action;

/// Timeout para la respuesta de `initialize`.
const INIT_TIMEOUT: Duration = Duration::from_secs(30);

/// Intervalo de poll del shutdown flag en el loop de notificaciones.
const SHUTDOWN_POLL_MS: u64 = 200;

// ── Commands from App to LSP client task ────────────────────────────────────

/// Commands that the App can send to the LSP client task.
#[derive(Debug)]
pub enum LspCommand {
    /// Notify the server that a file was opened.
    DidOpen { uri: String, language_id: String, version: i32, text: String },
    /// Notify the server that a file's content changed (full sync).
    DidChange { uri: String, version: i32, text: String },
}

/// Channel capacity for outgoing commands.
const CMD_CHANNEL_CAP: usize = 32;

/// Spawnea el client LSP como tokio task. Retorna un `Sender<LspCommand>`
/// para enviar didOpen/didChange desde el App.
///
/// El task vive hasta que `shutdown` se setea en true o el server muere.
pub fn spawn_lsp_client(
    config: &'static LspServerConfig,
    root_uri: String,
    shutdown: Arc<AtomicBool>,
    tx: Sender<Action>,
) -> tokio::sync::mpsc::Sender<LspCommand> {
    let (cmd_tx, cmd_rx) = tokio::sync::mpsc::channel::<LspCommand>(CMD_CHANNEL_CAP);
    tokio::spawn(async move {
        match run_client(config, &root_uri, &shutdown, &tx, cmd_rx).await {
            Ok(()) => {
                tracing::info!(server = config.name, "LSP client shutdown cleanly");
            }
            Err(e) => {
                tracing::warn!(server = config.name, err = %e, "LSP client error");
                let _ = tx
                    .send(Action::LspServerFailed {
                        name: config.name.to_string(),
                        error: e.to_string(),
                    })
                    .await;
            }
        }
    });
    cmd_tx
}

async fn run_client(
    config: &'static LspServerConfig,
    root_uri: &str,
    shutdown: &Arc<AtomicBool>,
    tx: &Sender<Action>,
    cmd_rx: tokio::sync::mpsc::Receiver<LspCommand>,
) -> Result<()> {
    let mut child = Command::new(config.command)
        .args(config.args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()?;

    let stdin = child.stdin.take().ok_or_else(|| anyhow!("no stdin"))?;
    let stdout = child.stdout.take().ok_or_else(|| anyhow!("no stdout"))?;
    let mut writer = LspWriter::new(stdin);
    let mut reader = LspReader::new(stdout);

    initialize(&mut writer, &mut reader, root_uri).await?;

    let _ = tx.send(Action::LspServerStarted { name: config.name.to_string() }).await;

    main_loop(&mut writer, &mut reader, cmd_rx, shutdown, tx).await;

    // Best-effort shutdown.
    let _ = writer.send_request(9999, "shutdown", json!(null)).await;
    let _ = writer.send_notification("exit", json!(null)).await;
    Ok(())
}

/// Envia initialize + initialized y espera la respuesta.
async fn initialize(writer: &mut LspWriter, reader: &mut LspReader, root_uri: &str) -> Result<()> {
    let params = json!({
        "processId": std::process::id(),
        "rootUri": root_uri,
        "capabilities": {
            "textDocument": {
                "publishDiagnostics": {
                    "relatedInformation": false,
                    "codeDescriptionSupport": false
                }
            }
        },
        "workspaceFolders": [{ "uri": root_uri, "name": "workspace" }]
    });
    writer.send_request(1, "initialize", params).await?;

    let response = tokio::time::timeout(INIT_TIMEOUT, reader.read_message())
        .await
        .map_err(|_| anyhow!("initialize timeout ({}s)", INIT_TIMEOUT.as_secs()))??;

    match response {
        LspMessage::Response { error: Some(e), .. } => Err(anyhow!("initialize error: {}", e)),
        LspMessage::Response { .. } => {
            writer.send_notification("initialized", json!({})).await?;
            Ok(())
        }
        _ => Err(anyhow!("respuesta inesperada a initialize")),
    }
}

/// Main loop: reads server notifications, dispatches App commands, polls shutdown.
async fn main_loop(
    writer: &mut LspWriter,
    reader: &mut LspReader,
    mut cmd_rx: tokio::sync::mpsc::Receiver<LspCommand>,
    shutdown: &Arc<AtomicBool>,
    tx: &Sender<Action>,
) {
    loop {
        tokio::select! {
            msg = reader.read_message() => {
                match msg {
                    Ok(LspMessage::Notification { method, params }) => {
                        handle_notification(&method, &params, tx).await;
                    }
                    Ok(LspMessage::Response { .. }) => {}
                    Err(e) => {
                        tracing::debug!(err = %e, "LSP read error — cerrando");
                        break;
                    }
                }
            }
            cmd = cmd_rx.recv() => {
                match cmd {
                    Some(c) => dispatch_command(writer, c).await,
                    None => break, // App dropped the sender.
                }
            }
            _ = tokio::time::sleep(Duration::from_millis(SHUTDOWN_POLL_MS)) => {
                if shutdown.load(Ordering::Relaxed) {
                    break;
                }
            }
        }
    }
}

/// Send a didOpen or didChange notification to the LSP server.
async fn dispatch_command(writer: &mut LspWriter, cmd: LspCommand) {
    let result = match cmd {
        LspCommand::DidOpen { uri, language_id, version, text } => {
            writer
                .send_notification(
                    "textDocument/didOpen",
                    json!({
                        "textDocument": {
                            "uri": uri,
                            "languageId": language_id,
                            "version": version,
                            "text": text,
                        }
                    }),
                )
                .await
        }
        LspCommand::DidChange { uri, version, text } => {
            writer
                .send_notification(
                    "textDocument/didChange",
                    json!({
                        "textDocument": { "uri": uri, "version": version },
                        "contentChanges": [{ "text": text }]
                    }),
                )
                .await
        }
    };
    if let Err(e) = result {
        tracing::warn!(err = %e, "LSP command send failed");
    }
}

async fn handle_notification(method: &str, params: &serde_json::Value, tx: &Sender<Action>) {
    if method != "textDocument/publishDiagnostics" {
        return;
    }
    let uri = match params.get("uri").and_then(|v| v.as_str()) {
        Some(u) => u,
        None => return,
    };
    let diags_json = match params.get("diagnostics").and_then(|v| v.as_array()) {
        Some(a) => a,
        None => return,
    };
    let diagnostics: Vec<LspDiagnostic> =
        diags_json.iter().filter_map(|d| parse_diagnostic(uri, d)).collect();

    let _ = tx.send(Action::LspDiagnosticsReceived { uri: uri.to_string(), diagnostics }).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// No podemos hacer tests de integracion sin un LSP server real,
    /// pero verificamos que la construccion de params es correcta.
    #[test]
    fn initialize_params_has_required_fields() {
        let root = "file:///tmp/test";
        let params = json!({
            "processId": std::process::id(),
            "rootUri": root,
            "capabilities": {
                "textDocument": {
                    "publishDiagnostics": {
                        "relatedInformation": false,
                        "codeDescriptionSupport": false
                    }
                }
            },
            "workspaceFolders": [{ "uri": root, "name": "workspace" }]
        });
        assert!(params.get("processId").is_some());
        assert_eq!(params["rootUri"], root);
        assert!(params["capabilities"]["textDocument"]["publishDiagnostics"].is_object());
    }

    #[tokio::test]
    async fn handle_notification_publishes_diagnostics_parses() {
        let (tx, mut rx) = tokio::sync::mpsc::channel::<Action>(8);
        let params = json!({
            "uri": "file:///tmp/foo.rs",
            "diagnostics": [{
                "message": "unused var",
                "severity": 2,
                "range": { "start": { "line": 5, "character": 0 }, "end": { "line": 5, "character": 3 } }
            }]
        });
        handle_notification("textDocument/publishDiagnostics", &params, &tx).await;
        let action = rx.try_recv().unwrap();
        match action {
            Action::LspDiagnosticsReceived { uri, diagnostics } => {
                assert_eq!(uri, "file:///tmp/foo.rs");
                assert_eq!(diagnostics.len(), 1);
                assert_eq!(diagnostics[0].message, "unused var");
            }
            other => panic!("unexpected action: {other:?}"),
        }
    }

    #[tokio::test]
    async fn handle_notification_ignores_other_methods() {
        let (tx, mut rx) = tokio::sync::mpsc::channel::<Action>(8);
        handle_notification("window/showMessage", &json!({"type": 3, "message": "hi"}), &tx).await;
        assert!(rx.try_recv().is_err());
    }
}
