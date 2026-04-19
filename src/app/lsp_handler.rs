//! Handlers de LSP integration (E25).
//!
//! Maneja las Actions emitidas por el client task:
//!   LspServerStarted, LspServerFailed, LspDiagnosticsReceived.
//!
//! La presentacion via comando del usuario esta en `doctor` (palette `:`).

use crate::services::lsp::LspDiagnostic;
use crate::state::ToastLevel;

use super::App;

impl App {
    /// Auto-start del LSP client si se detecta un server adecuado.
    /// Llamado desde startup (en enter_chat o post-wizard).
    pub(crate) fn try_start_lsp(&mut self) {
        if self.state.lsp.connected || self.state.lsp.server_name.is_some() {
            return;
        }
        let cwd = match std::env::current_dir() {
            Ok(p) => p,
            Err(_) => return,
        };
        let Some(config) = crate::services::lsp::detect(&cwd) else {
            return;
        };
        let root_uri = format!("file://{}", cwd.display());
        let shutdown = self.state.lsp.shutdown.clone();
        self.state.lsp.server_name = Some(config.name.to_string());

        let cmd_tx =
            crate::services::lsp::spawn_lsp_client(config, root_uri, shutdown, self.tx.clone());
        self.state.lsp.cmd_tx = Some(cmd_tx);
        tracing::info!(server = config.name, "LSP client spawn iniciado");
    }

    pub(crate) fn handle_lsp_server_started(&mut self, name: String) {
        self.state.lsp.connected = true;
        self.state.lsp.error = None;
        self.notify(format!("LSP: {name} conectado"));
    }

    pub(crate) fn handle_lsp_server_failed(&mut self, name: String, error: String) {
        self.state.lsp.connected = false;
        self.state.lsp.error = Some(error.clone());
        self.notify_level(&format!("LSP: {name} fallo — {error}"), ToastLevel::Warning);
    }

    pub(crate) fn handle_lsp_diagnostics_received(
        &mut self,
        uri: String,
        diagnostics: Vec<LspDiagnostic>,
    ) {
        let is_validation = self.state.lsp.pending_validation.remove(&uri);
        let error_count = diagnostics
            .iter()
            .filter(|d| d.severity == crate::services::lsp::Severity::Error)
            .count();
        let warn_count = diagnostics
            .iter()
            .filter(|d| d.severity == crate::services::lsp::Severity::Warning)
            .count();

        self.state.lsp.update_diagnostics(&uri, diagnostics);

        // Post-apply validation toast for recently modified files.
        if is_validation {
            let path = crate::services::lsp::types::uri_to_relative_path(&uri);
            if error_count > 0 {
                self.notify_level(
                    &format!("LSP: {path} — {error_count} error(s) detectados"),
                    ToastLevel::Error,
                );
            } else if warn_count > 0 {
                self.notify(format!("LSP: {path} — {warn_count} warning(s)"));
            } else {
                self.notify(format!("LSP: {path} — sin errores"));
            }
        }
    }
}
