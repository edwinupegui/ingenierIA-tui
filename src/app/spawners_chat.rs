use std::sync::Arc;

use tokio::sync::mpsc::Sender;

#[cfg(feature = "copilot")]
use crate::services::chat::copilot_provider::CopilotProvider;
use crate::{
    actions::Action,
    services::{
        chat::{
            claude_provider::ClaudeProvider,
            model_fallback::ModelFallbackChain,
            retry::{RetryConfig, RetryDecision, RetryManager},
            stall_detector::{PostToolStallDetector, StallAction},
            stream_monitor::StreamMonitor,
            ChatProvider, ToolDefinition,
        },
        copilot as copilot_service, copilot_chat,
        tools::ToolRegistry,
    },
    state::{ChatMessage, ChatRole},
};

/// Threshold de fallos consecutivos antes de sugerir cambio de modelo.
const FALLBACK_THRESHOLD: u32 = 3;
/// Modelo de fallback por defecto (mas barato/rapido) cuando el primario falla.
const DEFAULT_FALLBACK_MODEL: &str = "claude-haiku-4-5-20251001";

use super::App;

impl App {
    /// Fetch a document and inject it into chat as context.
    pub(crate) fn spawn_fetch_doc_for_chat(&self, doc_type: String, factory: String, name: String) {
        let client = self.client.clone();
        let tx = self.tx.clone();
        tokio::spawn(async move {
            match client.document(&doc_type, &factory, &name).await {
                Ok(doc) => {
                    let _ = tx.send(Action::ChatDocLoaded(doc)).await;
                }
                Err(e) => {
                    let _ = tx.send(Action::ChatDocLoadFailed(e.to_string())).await;
                }
            }
        });
    }

    pub(crate) fn spawn_chat_context(&self) {
        let client = self.client.clone();
        let tx = self.tx.clone();
        let factory_key = self.state.factory.api_key().map(String::from);
        let factory_label = self.state.factory.label().to_string();
        let developer = self.state.developer.clone();
        // E25: captura diagnosticos LSP ya formateados como markdown (sync).
        let lsp_ctx = crate::services::lsp::format_diagnostics_context(&self.state.lsp.diagnostics);

        tokio::spawn(async move {
            // Collect smart context (git diff, recent files, compiler errors)
            // in a blocking task since it runs shell commands
            let smart_ctx = tokio::task::spawn_blocking(|| {
                let dir = std::env::current_dir().unwrap_or_default();
                crate::services::context::collect(&dir)
            })
            .await
            .unwrap_or_else(|e| {
                tracing::error!(error = %e, "Context collection panicked");
                crate::services::context::SmartContext {
                    git_diff: None,
                    recent_files: Vec::new(),
                    compiler_errors: None,
                    git_branch: None,
                }
            });

            match copilot_chat::load_context(
                &client,
                factory_key.as_deref(),
                &factory_label,
                &developer,
                smart_ctx,
            )
            .await
            {
                Ok(mut prompt) => {
                    // E25: append LSP diagnostics to system prompt.
                    if let Some(lsp_section) = lsp_ctx {
                        prompt.push_str("\n\n");
                        prompt.push_str(&lsp_section);
                    }
                    let system_msg = ChatMessage::new(ChatRole::System, prompt);
                    let _ = tx.send(Action::ChatContextLoaded(vec![system_msg])).await;
                }
                Err(e) => {
                    let _ = tx.send(Action::ChatContextFailed(e.to_string())).await;
                }
            }
        });
    }

    pub(crate) fn spawn_chat_completion(&mut self) {
        let messages: Arc<[ChatMessage]> = self.state.chat.messages.clone().into();
        let tx = self.tx.clone();
        let model = self.state.model.clone();
        let claude_key = crate::app::wizard::load_claude_api_key();
        let copilot_auth = copilot_service::load_saved_auth();
        let mock_provider = self.mock_provider;
        #[cfg(feature = "mcp")]
        let mcp_tool_defs = self.state.mcp_tools.clone();

        tracing::info!(
            msg_count = messages.len(),
            has_claude = claude_key.is_some(),
            has_copilot = copilot_auth.is_some(),
            mock = mock_provider,
            model = %model,
            "spawn_chat_completion"
        );

        // Abortar turn previo si quedo colgado (defensivo: normalmente Done/Failure
        // lo limpia, pero re-entry al provider no debe acumular tasks zombies).
        if let Some(h) = self.chat_abort.take() {
            h.abort();
        }

        let handle = tokio::spawn(async move {
            let provider =
                match resolve_provider(claude_key, copilot_auth, mock_provider, &tx).await {
                    Some(p) => p,
                    None => return,
                };

            #[cfg(feature = "mcp")]
            let tools = build_tool_defs(&mcp_tool_defs);
            #[cfg(not(feature = "mcp"))]
            let tools = build_tool_defs();

            run_with_retry(provider, model, messages, tools, tx).await;
        });
        self.chat_abort = Some(handle.abort_handle());
    }
}

// ── Helpers internos ────────────────────────────────────────────────────────

/// Construye el provider apropiado o emite ChatStreamFailure si no hay credenciales.
/// E21: si `mock_provider=true` usa MockChatProvider sin leer credenciales.
async fn resolve_provider(
    claude_key: Option<String>,
    copilot_auth: Option<crate::services::auth::CopilotAuth>,
    mock_provider: bool,
    tx: &Sender<Action>,
) -> Option<Box<dyn ChatProvider>> {
    if mock_provider {
        return Some(Box::new(crate::services::chat::mock_provider::MockChatProvider::from_env()));
    }
    if let Some(key) = claude_key {
        return Some(Box::new(ClaudeProvider::new(key)));
    }
    #[cfg(feature = "copilot")]
    if let Some(auth) = copilot_auth {
        return Some(Box::new(CopilotProvider::new(auth)));
    }
    #[cfg(not(feature = "copilot"))]
    let _ = copilot_auth;
    let failure = crate::domain::failure::StructuredFailure::new(
        crate::domain::failure::FailureCategory::ApiKeyInvalid,
        "No hay proveedor configurado. Usa :config para configurar.",
    );
    let _ = tx.send(Action::ChatStreamFailure(failure)).await;
    None
}

/// Construye la lista de tool definitions (registry interno + ConfigTool + MCP si aplica).
#[cfg(feature = "mcp")]
fn build_tool_defs(mcp_tools: &[crate::services::mcp::McpToolInfo]) -> Vec<ToolDefinition> {
    let registry = ToolRegistry::new();
    let mut tools = registry.definitions();
    // E20: exponer ConfigTool al AI fuera del registry (vive en chat_tools.rs
    // porque necesita Sender<Action> para aplicar cambios de estado).
    tools.push(crate::services::tools::config_tool::config_tool_definition());
    tools.push(crate::services::tools::todowrite::todo_write_definition());
    for mcp_def in mcp_tools {
        tools.push(crate::services::chat::tool_def_from_mcp(mcp_def));
    }
    tools
}

#[cfg(not(feature = "mcp"))]
fn build_tool_defs() -> Vec<ToolDefinition> {
    let mut tools = ToolRegistry::new().definitions();
    tools.push(crate::services::tools::config_tool::config_tool_definition());
    tools.push(crate::services::tools::todowrite::todo_write_definition());
    tools
}

/// Loop de retry con exponential backoff y countdown sobre la llamada al provider.
async fn run_with_retry(
    provider: Box<dyn ChatProvider>,
    model: String,
    messages: Arc<[ChatMessage]>,
    tools: Vec<ToolDefinition>,
    tx: Sender<Action>,
) {
    let mut retry_mgr = RetryManager::new(RetryConfig::default());
    let max_attempts = retry_mgr.config().max_retries;
    let mut fallback = ModelFallbackChain::new(
        model.clone(),
        vec![DEFAULT_FALLBACK_MODEL.to_string()],
        FALLBACK_THRESHOLD,
    );

    loop {
        match attempt_stream(provider.as_ref(), &model, &messages, &tools, &tx).await {
            AttemptResult::Completed => {
                fallback.record_success();
                break;
            }
            AttemptResult::Failed(error) => match retry_mgr.on_error(&error) {
                RetryDecision::Retry { attempt, delay, reason } => {
                    let delay_secs = delay.as_secs().max(1).min(u16::MAX as u64) as u16;
                    let _ = tx
                        .send(Action::ChatRetryScheduled {
                            attempt,
                            max_attempts,
                            delay_secs,
                            reason,
                        })
                        .await;
                    tokio::time::sleep(delay).await;
                    continue;
                }
                RetryDecision::GiveUp { reason } => {
                    if let Some(next) = fallback.record_failure() {
                        let _ = tx
                            .send(Action::ChatFallbackSuggested {
                                previous_model: model.clone(),
                                suggested_model: next.to_string(),
                            })
                            .await;
                    }
                    let failure = crate::domain::failure::StructuredFailure::from_error(reason);
                    let _ = tx.send(Action::ChatStreamFailure(failure)).await;
                    return;
                }
            },
        }
    }
    let _ = tx.send(Action::ChatStreamDone).await;
}

enum AttemptResult {
    Completed,
    Failed(String),
}

/// Ejecuta una sola tentativa: abre el stream y drena eventos.
async fn attempt_stream(
    provider: &dyn ChatProvider,
    model: &str,
    messages: &[ChatMessage],
    tools: &[ToolDefinition],
    tx: &Sender<Action>,
) -> AttemptResult {
    match provider.stream_chat(model, messages, tools).await {
        Ok(stream) => drive_stream(stream, tx.clone()).await,
        Err(e) => AttemptResult::Failed(e.to_string()),
    }
}

/// Drena el stream de eventos del provider hasta `Done` o EOF, integrando
/// `StreamMonitor` (heartbeat/warning/timeout) y `PostToolStallDetector`.
///
/// Trackea `got_content` para distinguir stream con contenido real (Delta o
/// ToolCall) vs stream vacío. Un stream que termina sin emitir nada se trata
/// como fallo (el provider devolvió respuesta vacía) para que el retry +
/// `ChatStreamFailure` se disparen y el user no quede mirando silencio.
async fn drive_stream(
    mut stream: crate::services::chat::ChatStream,
    tx: Sender<Action>,
) -> AttemptResult {
    use crate::services::chat::ChatEvent;
    use futures_util::StreamExt;

    let mut monitor = StreamMonitor::new(tx.clone());
    let mut stall = PostToolStallDetector::new();
    let mut got_content = false;

    loop {
        let interval = monitor.next_interval();
        tokio::select! {
            event = stream.next() => {
                match event {
                    Some(ChatEvent::Delta(text)) => {
                        got_content = true;
                        monitor.on_delta();
                        stall.on_stream_event();
                        let _ = tx.send(Action::ChatStreamDelta(text)).await;
                    }
                    Some(ChatEvent::ThinkingDelta(text)) => {
                        monitor.on_delta();
                        stall.on_stream_event();
                        let _ = tx.send(Action::ChatThinkingDelta(text)).await;
                    }
                    Some(ChatEvent::ToolCall { id, name, arguments }) => {
                        got_content = true;
                        monitor.on_delta();
                        stall.on_tool_completed();
                        let _ = tx.send(Action::ChatToolCall { id, name, arguments }).await;
                    }
                    Some(ChatEvent::Usage {
                        input_tokens,
                        output_tokens,
                        cache_creation_input_tokens,
                        cache_read_input_tokens,
                        truncated,
                    }) => {
                        let _ = tx.send(Action::ChatTokenUsage {
                            input_tokens,
                            output_tokens,
                            cache_creation_input_tokens,
                            cache_read_input_tokens,
                        }).await;
                        if truncated {
                            let _ = tx.send(Action::ChatStreamTruncated).await;
                        }
                    }
                    Some(ChatEvent::Done) | None => {
                        tracing::debug!(got_content, "drive_stream end");
                        return if got_content {
                            AttemptResult::Completed
                        } else {
                            AttemptResult::Failed(
                                "empty response: el provider no emitio contenido".to_string(),
                            )
                        };
                    }
                }
            }
            _ = tokio::time::sleep(interval) => {
                if monitor.tick().await {
                    // timeout: el monitor envia StreamTimeout al tx
                    return if got_content {
                        AttemptResult::Completed
                    } else {
                        AttemptResult::Failed(
                            "empty response: timeout sin contenido".to_string(),
                        )
                    };
                }
                if let StallAction::Nudge { nudge_number, .. } = stall.check() {
                    let _ = tx.send(Action::ChatPostToolStall { nudge_number }).await;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::chat::ChatEvent;

    fn stream_from(events: Vec<ChatEvent>) -> crate::services::chat::ChatStream {
        Box::pin(futures_util::stream::iter(events))
    }

    #[tokio::test]
    async fn drive_stream_empty_returns_failed() {
        let (tx, _rx) = tokio::sync::mpsc::channel::<Action>(16);
        let stream = stream_from(vec![ChatEvent::Done]);
        let result = drive_stream(stream, tx).await;
        match result {
            AttemptResult::Failed(msg) => assert!(msg.contains("empty response")),
            AttemptResult::Completed => panic!("expected Failed for empty stream"),
        }
    }

    #[tokio::test]
    async fn drive_stream_usage_only_returns_failed() {
        // Provider que emite solo Usage (sin Delta ni ToolCall) también cuenta
        // como respuesta vacía: el user no vio ningún contenido.
        let (tx, _rx) = tokio::sync::mpsc::channel::<Action>(16);
        let stream = stream_from(vec![
            ChatEvent::Usage {
                input_tokens: 10,
                output_tokens: 0,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 0,
                truncated: false,
            },
            ChatEvent::Done,
        ]);
        let result = drive_stream(stream, tx).await;
        assert!(matches!(result, AttemptResult::Failed(_)));
    }

    #[tokio::test]
    async fn drive_stream_with_delta_returns_completed() {
        let (tx, _rx) = tokio::sync::mpsc::channel::<Action>(16);
        let stream = stream_from(vec![ChatEvent::Delta("hola".to_string()), ChatEvent::Done]);
        let result = drive_stream(stream, tx).await;
        assert!(matches!(result, AttemptResult::Completed));
    }

    #[tokio::test]
    async fn drive_stream_with_tool_call_returns_completed() {
        let (tx, _rx) = tokio::sync::mpsc::channel::<Action>(16);
        let stream = stream_from(vec![
            ChatEvent::ToolCall { id: "1".into(), name: "Read".into(), arguments: "{}".into() },
            ChatEvent::Done,
        ]);
        let result = drive_stream(stream, tx).await;
        assert!(matches!(result, AttemptResult::Completed));
    }
}
