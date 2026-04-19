use std::time::Duration;

use crate::{
    actions::Action,
    config,
    services::{copilot as copilot_service, init as init_service, sync::sync_via_mcp},
};

use super::local_search;
use super::App;

const SEARCH_DEBOUNCE: Duration = Duration::from_millis(150);
const WIZARD_URL_CHECK_TIMEOUT: Duration = Duration::from_secs(5);
const COPILOT_POLL_INTERVAL: Duration = Duration::from_secs(5);

impl App {
    pub(crate) fn spawn_load_documents(&self) {
        let client = self.client.clone();
        let tx = self.tx.clone();
        tokio::spawn(async move {
            match client.documents(None, None).await {
                Ok(docs) => {
                    let _ = tx.send(Action::DocumentsLoaded(docs)).await;
                }
                Err(e) => {
                    let _ = tx.send(Action::DocumentsLoadFailed(e.to_string())).await;
                }
            }
        });
    }

    pub(crate) fn spawn_fetch_document(&self, doc_type: String, factory: String, name: String) {
        // Check in-memory cache first (peek = no mutable borrow)
        let uri = format!("ingenieria://{doc_type}/{factory}/{name}");
        if let Some(doc) = self.state.caches.doc_details.peek(&uri) {
            let _ = self.tx.try_send(Action::DocumentLoaded(doc.clone()));
            return;
        }
        let client = self.client.clone();
        let tx = self.tx.clone();
        tokio::spawn(async move {
            match client.document(&doc_type, &factory, &name).await {
                Ok(doc) => {
                    let _ = tx.send(Action::DocumentLoaded(doc)).await;
                }
                Err(e) => {
                    let _ = tx.send(Action::DocumentLoadFailed(e.to_string())).await;
                }
            }
        });
    }

    /// Debounce 150ms: cancela la tarea anterior y lanza una nueva.
    pub(crate) fn spawn_search_debounced(&mut self, query: String) {
        self.cancel_search();

        if query.is_empty() {
            self.state.search.results.clear();
            self.state.search.loading = false;
            return;
        }

        let all_docs = self.state.dashboard.sidebar.all_docs.clone();

        if query.len() < 3 {
            self.state.search.results = local_search(&all_docs, &query);
            self.state.search.cursor = 0;
            self.state.search.loading = false;
            return;
        }

        // Check search cache before hitting server
        let factory_key = self.state.factory.filter_key().unwrap_or("");
        let cache_key = format!("{query}\0{factory_key}");
        if let Some(results) = self.state.caches.search.get(&cache_key) {
            self.state.search.results = results.clone();
            self.state.search.cursor = 0;
            self.state.search.loading = false;
            return;
        }

        let client = self.client.clone();
        let tx = self.tx.clone();
        let factory = self.state.factory.filter_key().map(String::from);
        let handle = tokio::spawn(async move {
            tokio::time::sleep(SEARCH_DEBOUNCE).await;
            match client.search(&query, factory.as_deref()).await {
                Ok(resp) => {
                    let _ = tx.send(Action::SearchResultsReceived(resp.results)).await;
                }
                Err(e) => {
                    let _ = tx.send(Action::SearchFailed(e.to_string())).await;
                }
            }
        });

        self.search_abort = Some(handle.abort_handle());
        self.state.search.loading = true;
    }

    pub(crate) fn cancel_search(&mut self) {
        if let Some(h) = self.search_abort.take() {
            h.abort();
        }
    }

    /// Detect project type at startup (non-blocking).
    pub(crate) fn spawn_project_detect(&self) {
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let dir = std::env::current_dir().unwrap_or_default();
            let detected = init_service::detect_project_type(&dir);
            let _ = tx.send(Action::ProjectTypeDetected(detected)).await;
        });
    }

    /// Discover MCP tools via `tools/list` (non-blocking, silent on failure).
    #[cfg(feature = "mcp")]
    pub(crate) fn spawn_discover_mcp_tools(&self) {
        let base_url = self.client.base_url();
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let Ok(mcp) = crate::services::mcp::McpClient::connect(&base_url).await else {
                return;
            };
            if let Ok(tools) = mcp.list_tools().await {
                let _ = tx.send(Action::McpToolsDiscovered(tools)).await;
            }
        });
    }

    /// Run extended autoskill scan (tech detect + skill mapping + install check).
    #[cfg(feature = "autoskill")]
    pub(crate) fn spawn_autoskill_scan(&self) {
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let dir = std::env::current_dir().unwrap_or_default();
            let scan = crate::services::autoskill_map::detect(&dir);
            let _ = tx.send(Action::AutoSkillScanDone(scan)).await;
        });
    }

    /// Install pending external skills from skills.sh.
    #[cfg(feature = "autoskill")]
    pub(crate) fn spawn_install_skills(&self, skill_paths: Vec<String>) {
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let summary = crate::services::skill_installer::install_all(&skill_paths).await;
            let _ = tx.send(Action::SkillInstallDone(summary)).await;
        });
    }

    pub(crate) fn spawn_init_detect(&self) {
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let dir = std::env::current_dir()
                .map(|d| d.to_string_lossy().to_string())
                .unwrap_or_else(|_| ".".to_string());
            let dir_path = std::path::PathBuf::from(&dir);
            let detected = init_service::detect_project_type(&dir_path);
            let _ = tx.send(Action::InitDetected(detected, dir)).await;
        });
    }

    pub(crate) fn spawn_init_run(&self) {
        let tx = self.tx.clone();
        let dir = self.state.init.project_dir.clone();
        let server_url = self.state.wizard.server_url_input.clone();
        let server_url =
            if server_url.is_empty() { self.client.base_url().to_string() } else { server_url };
        let type_cursor = self.state.init.type_cursor;
        let client_cursor = self.state.init.client_cursor;

        tokio::spawn(async move {
            let project_type = init_service::ProjectType::ALL
                .get(type_cursor)
                .cloned()
                .unwrap_or(init_service::ProjectType::Unknown);
            let client = init_service::InitClient::ALL
                .get(client_cursor)
                .cloned()
                .unwrap_or(init_service::InitClient::Claude);
            let dir_path = std::path::PathBuf::from(&dir);

            match init_service::run_init(&dir_path, &server_url, &client, &project_type) {
                Ok(results) => {
                    let _ = tx.send(Action::InitComplete(results)).await;
                }
                Err(msg) => {
                    let _ = tx.send(Action::InitFailed(msg.to_string())).await;
                }
            }
        });
    }

    pub(crate) fn spawn_wizard_url_check(&self, url: String) {
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let trimmed = url.trim_end_matches('/');
            if !trimmed.starts_with("http://") && !trimmed.starts_with("https://") {
                let _ = tx
                    .send(Action::WizardUrlInvalid(
                        "URL debe empezar con http:// o https://".to_string(),
                    ))
                    .await;
                return;
            }
            let test_url = format!("{trimmed}/api/health");
            let client = reqwest::Client::builder()
                .timeout(WIZARD_URL_CHECK_TIMEOUT)
                .build()
                .unwrap_or_default();
            match client.get(&test_url).send().await {
                Ok(resp) if resp.status().is_success() => {
                    let _ = tx.send(Action::WizardUrlValid).await;
                }
                Ok(resp) => {
                    let _ =
                        tx.send(Action::WizardUrlInvalid(format!("HTTP {}", resp.status()))).await;
                }
                Err(e) => {
                    let msg = if e.is_timeout() {
                        "Timeout — el servidor no responde".to_string()
                    } else if e.is_connect() {
                        "No se pudo conectar al servidor".to_string()
                    } else {
                        e.to_string()
                    };
                    let _ = tx.send(Action::WizardUrlInvalid(msg)).await;
                }
            }
        });
    }

    pub(crate) fn spawn_health_check(&self) {
        let client = self.client.clone();
        let tx = self.tx.clone();
        tokio::spawn(async move {
            match client.health().await {
                Ok(h) => {
                    let _ = tx.send(Action::HealthUpdated(h)).await;
                }
                Err(e) => {
                    let _ = tx.send(Action::HealthFetchFailed(e.to_string())).await;
                }
            }
        });
    }

    pub(crate) fn spawn_copilot_device_code(&self, host: String) {
        let tx = self.tx.clone();
        tokio::spawn(async move {
            match copilot_service::request_device_code(&host).await {
                Ok(resp) => {
                    let _ = tx
                        .send(Action::CopilotDeviceCode {
                            user_code: resp.user_code,
                            verification_uri: resp.verification_uri,
                            device_code: crate::actions::Redacted(resp.device_code),
                        })
                        .await;
                }
                Err(e) => {
                    let _ = tx.send(Action::CopilotDeviceCodeFailed(e.to_string())).await;
                }
            }
        });
    }

    pub(crate) fn spawn_copilot_poll(&self, host: String, device_code: String) {
        let tx = self.tx.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(COPILOT_POLL_INTERVAL).await;
                match copilot_service::poll_for_token(&host, &device_code).await {
                    Ok(Some(token)) => {
                        let _ = tx
                            .send(Action::CopilotAuthSuccess(crate::actions::Redacted(token)))
                            .await;
                        return;
                    }
                    Ok(None) => {}
                    Err(e) => {
                        let _ = tx.send(Action::CopilotAuthFailed(e.to_string())).await;
                        return;
                    }
                }
            }
        });
    }

    pub(crate) fn spawn_copilot_models(&self, host: String, token: String) {
        let tx = self.tx.clone();
        tokio::spawn(async move {
            match copilot_service::fetch_models_with_retry(&host, &token).await {
                Ok(models) => {
                    let _ = tx.send(Action::CopilotModelsLoaded(models)).await;
                }
                Err(e) => {
                    let _ = tx.send(Action::CopilotModelsFailed(e.to_string())).await;
                }
            }
        });
    }

    /// Call sync_project via MCP to detect docs changed since last sync.
    pub(crate) fn spawn_sync_check(&self) {
        let base_url = self.client.base_url().to_string();
        let factory = self.state.factory.api_key().unwrap_or("net").to_string();
        let last_sync = config::load_last_sync_date();
        let tx = self.tx.clone();

        tokio::spawn(async move {
            let result = sync_via_mcp(&base_url, &factory, last_sync.as_deref()).await;
            match result {
                Ok((uris, server_ts)) => {
                    let _ = tx
                        .send(Action::SyncResult {
                            updated_uris: uris,
                            server_last_update: server_ts,
                        })
                        .await;
                }
                Err(e) => {
                    tracing::debug!(error = %e, "Sync check failed");
                    let _ = tx.send(Action::SyncFailed(e.to_string())).await;
                }
            }
        });
    }

    /// Load a workflow definition via MCP get_workflow tool.
    pub(crate) fn spawn_load_workflow(&self, workflow_name: String) {
        let base_url = self.client.base_url().to_string();
        let factory = self.state.factory.api_key().unwrap_or("net").to_string();
        let tx = self.tx.clone();

        tokio::spawn(async move {
            match load_workflow_via_mcp(&base_url, &workflow_name, &factory).await {
                Ok(content) => {
                    let _ = tx.send(Action::WorkflowLoaded { workflow_name, content }).await;
                }
                Err(e) => {
                    let _ = tx.send(Action::WorkflowFailed(e.to_string())).await;
                }
            }
        });
    }

    /// Run validate_compliance via MCP for the given factory.
    pub(crate) fn spawn_compliance_check(&self, factory: String) {
        let base_url = self.client.base_url().to_string();
        let tx = self.tx.clone();

        tokio::spawn(async move {
            match crate::services::compliance::validate_all(&base_url, &factory).await {
                Ok(report) => {
                    let _ = tx.send(Action::ComplianceResult(report)).await;
                }
                Err(e) => {
                    let _ = tx.send(Action::ComplianceFailed(e.to_string())).await;
                }
            }
        });
    }

    /// Run all doctor checks asynchronously and report results.
    pub(crate) fn spawn_doctor_checks(&self) {
        let client = self.client.clone();
        let tx = self.tx.clone();
        let server_status = self.state.server_status.clone();
        #[cfg(feature = "mcp")]
        let mcp_tools_count = self.state.mcp_tools.len();
        #[cfg(not(feature = "mcp"))]
        let mcp_tools_count = 0usize;

        let inputs = crate::services::doctor::DoctorInputs {
            lsp: crate::services::doctor::DoctorLspInfo {
                server_name: self.state.lsp.server_name.clone(),
                connected: self.state.lsp.connected,
                diagnostics_count: self.state.lsp.diagnostics.len(),
                error: self.state.lsp.error.clone(),
            },
            #[cfg(feature = "ide")]
            bridge_active: self.bridge_state_tx.is_some(),
            #[cfg(not(feature = "ide"))]
            bridge_active: false,
        };

        tokio::spawn(async move {
            let report = crate::services::doctor::run_checks(
                &client,
                &server_status,
                mcp_tools_count,
                inputs,
            )
            .await;
            let _ = tx.send(Action::DoctorReportReady(report)).await;
        });
    }
}

// ── Workflow helper ─────────────────────────────────────────────────────

async fn load_workflow_via_mcp(
    base_url: &str,
    workflow: &str,
    factory: &str,
) -> anyhow::Result<String> {
    use crate::services::mcp::McpClient;
    let mcp = McpClient::connect(base_url).await?;
    let content = mcp
        .call_tool("get_workflow", serde_json::json!({ "workflow": workflow, "factory": factory }))
        .await?;
    Ok(content)
}
