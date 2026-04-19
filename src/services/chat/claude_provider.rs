//! ChatProvider implementation for Anthropic Claude API.
//!
//! Uses the Messages API with streaming (SSE).
//! API docs: https://docs.anthropic.com/en/api/messages

use super::claude_sse::parse_claude_sse;
use super::prompt_cache::{cache_system_prompt, cache_tool_definitions};
use super::{ChatProvider, ChatStream, ModelInfo, ToolDefinition};
use crate::state::{ChatMessage, ChatRole};

const ANTHROPIC_API_URL: &str = "https://api.anthropic.com/v1/messages";
const ANTHROPIC_VERSION: &str = "2023-06-01";
const DEFAULT_MAX_TOKENS: u32 = 4096;
const CLAUDE_HTTP_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);

/// ChatProvider backed by Anthropic's Claude Messages API.
pub struct ClaudeProvider {
    api_key: String,
    client: reqwest::Client,
}

impl ClaudeProvider {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            client: reqwest::Client::builder()
                .timeout(CLAUDE_HTTP_TIMEOUT)
                .build()
                .unwrap_or_default(),
        }
    }
}

#[async_trait::async_trait]
impl ChatProvider for ClaudeProvider {
    async fn stream_chat(
        &self,
        model: &str,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
    ) -> anyhow::Result<ChatStream> {
        let (system_prompt, api_messages) = build_claude_messages(messages);

        let mut body = serde_json::json!({
            "model": model,
            "max_tokens": DEFAULT_MAX_TOKENS,
            "messages": api_messages,
            "stream": true,
        });

        if let Some(system) = system_prompt {
            // Aplicar prompt cache si el system es lo bastante largo (≥1024 tk).
            // Si es corto, enviamos el string plano (Anthropic acepta ambos formatos).
            body["system"] = match cache_system_prompt(&system) {
                Some(blocks) => blocks,
                None => serde_json::Value::String(system),
            };
        }

        if !tools.is_empty() {
            let mut tool_defs: Vec<serde_json::Value> =
                tools.iter().filter_map(convert_tool_definition).collect();
            if !tool_defs.is_empty() {
                // Marcar la ultima tool con cache_control para cachear todas las
                // definitions (Anthropic cachea desde el inicio del array hasta
                // el primer breakpoint encontrado).
                cache_tool_definitions(&mut tool_defs);
                body["tools"] = serde_json::Value::Array(tool_defs);
            }
        }

        let resp = self
            .client
            .post(ANTHROPIC_API_URL)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("content-type", "application/json")
            .header("accept", "text/event-stream")
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            // Captura retry-after antes de consumir el body (los headers se pierden después).
            let retry_after = resp
                .headers()
                .get("retry-after-ms")
                .and_then(|v| v.to_str().ok())
                .map(|s| format!(" [retry-after-ms={s}]"))
                .or_else(|| {
                    resp.headers()
                        .get("retry-after")
                        .and_then(|v| v.to_str().ok())
                        .map(|s| format!(" [retry-after={s}]"))
                })
                .unwrap_or_default();
            let text = resp.text().await.unwrap_or_default();
            anyhow::bail!("Claude API error: {status}{retry_after} {text}");
        }

        let stream = parse_claude_sse(resp.bytes_stream());
        Ok(Box::pin(stream))
    }

    async fn list_models(&self) -> anyhow::Result<Vec<ModelInfo>> {
        Ok(vec![
            ModelInfo {
                id: "claude-sonnet-4-20250514".into(),
                display_name: "Claude Sonnet 4".into(),
            },
            ModelInfo {
                id: "claude-haiku-4-20250514".into(),
                display_name: "Claude Haiku 4".into(),
            },
            ModelInfo { id: "claude-opus-4-20250514".into(), display_name: "Claude Opus 4".into() },
        ])
    }

    fn provider_name(&self) -> &str {
        "Anthropic Claude"
    }
}

// ── Message conversion ──────────────────────────────────────────────────

/// Convert internal messages to Anthropic format.
/// System messages are extracted separately (Anthropic uses a top-level `system` field).
fn build_claude_messages(messages: &[ChatMessage]) -> (Option<String>, Vec<serde_json::Value>) {
    let mut system_parts: Vec<String> = Vec::new();
    let mut api_msgs: Vec<serde_json::Value> = Vec::new();

    for msg in messages {
        match msg.role {
            ChatRole::System => {
                system_parts.push(msg.content.clone());
            }
            ChatRole::User => {
                api_msgs.push(serde_json::json!({
                    "role": "user",
                    "content": msg.content,
                }));
            }
            ChatRole::Assistant => {
                let mut content_blocks: Vec<serde_json::Value> = Vec::new();

                if !msg.content.is_empty() {
                    content_blocks.push(serde_json::json!({
                        "type": "text",
                        "text": msg.content,
                    }));
                }

                for tc in &msg.tool_calls {
                    let args: serde_json::Value =
                        serde_json::from_str(&tc.arguments).unwrap_or(serde_json::json!({}));
                    content_blocks.push(serde_json::json!({
                        "type": "tool_use",
                        "id": tc.id,
                        "name": tc.name,
                        "input": args,
                    }));
                }

                api_msgs.push(serde_json::json!({
                    "role": "assistant",
                    "content": content_blocks,
                }));
            }
            ChatRole::Tool => {
                if let Some(ref id) = msg.tool_call_id {
                    api_msgs.push(serde_json::json!({
                        "role": "user",
                        "content": [{
                            "type": "tool_result",
                            "tool_use_id": id,
                            "content": msg.content,
                        }],
                    }));
                }
            }
        }
    }

    let system = if system_parts.is_empty() { None } else { Some(system_parts.join("\n\n")) };
    (system, api_msgs)
}

/// Convert OpenAI-format tool definition to Anthropic format.
fn convert_tool_definition(tool: &ToolDefinition) -> Option<serde_json::Value> {
    let func = tool.json.get("function")?;
    Some(serde_json::json!({
        "name": func.get("name")?,
        "description": func.get("description").unwrap_or(&serde_json::Value::Null),
        "input_schema": func.get("parameters").unwrap_or(&serde_json::json!({"type": "object"})),
    }))
}
