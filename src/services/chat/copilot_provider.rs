use super::{stream_parser, ChatProvider, ChatStream, ModelInfo, ToolDefinition};
use crate::services::auth::CopilotAuth;
use crate::services::copilot;
use crate::state::{ChatMessage, ChatRole};

/// ChatProvider implementation backed by GitHub Copilot's API.
pub struct CopilotProvider {
    auth: CopilotAuth,
}

impl CopilotProvider {
    pub fn new(auth: CopilotAuth) -> Self {
        Self { auth }
    }
}

#[async_trait::async_trait]
impl ChatProvider for CopilotProvider {
    async fn stream_chat(
        &self,
        model: &str,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
    ) -> anyhow::Result<ChatStream> {
        let copilot_token =
            copilot::get_copilot_token(&self.auth.github_host, &self.auth.oauth_token).await?;

        let api_messages = build_api_messages(messages);
        let tool_values: Vec<&serde_json::Value> = tools.iter().map(|t| &t.json).collect();

        let body = serde_json::json!({
            "model": model,
            "messages": api_messages,
            "stream": true,
            "tools": tool_values,
        });

        let client = copilot::http_client();
        let resp = client
            .post("https://api.githubcopilot.com/chat/completions")
            .header("Authorization", format!("Bearer {copilot_token}"))
            .header("Content-Type", "application/json")
            .header("Accept", "text/event-stream")
            .header("Copilot-Integration-Id", "vscode-chat")
            .header("Editor-Version", copilot::EDITOR_VERSION)
            .header("Editor-Plugin-Version", copilot::EDITOR_PLUGIN_VERSION)
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            tracing::error!(%status, body = %text, "Copilot chat HTTP error");
            anyhow::bail!("Copilot chat failed: {status} {text}");
        }

        let stream = stream_parser::parse_sse_stream(resp.bytes_stream());
        Ok(Box::pin(stream))
    }

    async fn list_models(&self) -> anyhow::Result<Vec<ModelInfo>> {
        let models = copilot::fetch_models(&self.auth.github_host, &self.auth.oauth_token).await?;
        Ok(models
            .into_iter()
            .map(|m| ModelInfo { id: m.id, display_name: m.display_name })
            .collect())
    }

    fn provider_name(&self) -> &str {
        "GitHub Copilot"
    }
}

fn build_api_messages(messages: &[ChatMessage]) -> Vec<serde_json::Value> {
    messages
        .iter()
        .map(|m| {
            let mut msg = serde_json::json!({
                "role": match m.role {
                    ChatRole::System => "system",
                    ChatRole::User => "user",
                    ChatRole::Assistant => "assistant",
                    ChatRole::Tool => "tool",
                },
                "content": m.content,
            });
            if m.role == ChatRole::Assistant && !m.tool_calls.is_empty() {
                let tc: Vec<serde_json::Value> = m
                    .tool_calls
                    .iter()
                    .map(|tc| {
                        serde_json::json!({
                            "id": tc.id,
                            "type": "function",
                            "function": {
                                "name": tc.name,
                                "arguments": tc.arguments,
                            }
                        })
                    })
                    .collect();
                msg["tool_calls"] = serde_json::Value::Array(tc);
            }
            if let Some(ref id) = m.tool_call_id {
                msg["tool_call_id"] = serde_json::Value::String(id.clone());
            }
            msg
        })
        .collect()
}
