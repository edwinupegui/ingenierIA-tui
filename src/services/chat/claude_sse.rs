//! SSE parser para Anthropic Messages API.
//!
//! Convierte eventos `event:` / `data:` en `ChatEvent`. Acumula:
//!   - tool_use blocks por content_block_start + input_json_delta
//!   - usage tokens (input/output/cache) de message_start y message_delta
//!
//! Emite los `ToolCall`s acumulados despues de `message_stop`, luego un
//! `Usage` con conteos finales, y finalmente `Done`.

use futures_util::{Stream, StreamExt};
use serde::Deserialize;

use super::ChatEvent;

#[derive(Debug, Deserialize)]
struct ContentBlockDelta {
    delta: Option<DeltaContent>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
// Los nombres vienen del spec de Anthropic API — todos terminan en Delta.
#[allow(clippy::enum_variant_names)]
enum DeltaContent {
    #[serde(rename = "text_delta")]
    TextDelta { text: String },
    #[serde(rename = "input_json_delta")]
    InputJsonDelta { partial_json: String },
    #[serde(rename = "thinking_delta")]
    ThinkingDelta { thinking: String },
    #[serde(rename = "signature_delta")]
    SignatureDelta {
        #[expect(dead_code, reason = "campo requerido por serde para deserializar el bloque de firma")]
        signature: String,
    },
}

#[derive(Debug, Deserialize)]
struct ContentBlockStart {
    index: usize,
    content_block: ContentBlock,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum ContentBlock {
    #[serde(rename = "text")]
    #[expect(dead_code, reason = "variant needed for serde deserialization")]
    Text { text: String },
    #[serde(rename = "tool_use")]
    ToolUse { id: String, name: String },
    #[serde(rename = "thinking")]
    #[expect(dead_code, reason = "variant needed for serde deserialization")]
    Thinking { thinking: String },
}

/// Tool accumulator: (index, id, name, arguments_json)
struct ToolAccumulator {
    #[expect(dead_code, reason = "tracks content_block index for potential future use")]
    index: usize,
    id: String,
    name: String,
    args: String,
}

/// Usage accumulator for tracking input/output/cache tokens across events.
///
/// Anthropic envia los conteos de cache en el `usage` del `message_start`:
///   - `cache_creation_input_tokens`: tokens enviados a crear cache (cobra +25%).
///   - `cache_read_input_tokens`: tokens leidos de cache (cuesta ~10% del input).
#[derive(Default)]
struct UsageAccumulator {
    input_tokens: u32,
    output_tokens: u32,
    cache_creation_input_tokens: u32,
    cache_read_input_tokens: u32,
    truncated: bool,
}

/// Lee un campo u64 opcional del json y lo trunca a u32 con saturacion.
fn read_u32(value: &serde_json::Value, key: &str) -> u32 {
    value.get(key).and_then(|v| v.as_u64()).unwrap_or(0).min(u32::MAX as u64) as u32
}

pub fn parse_claude_sse(
    byte_stream: impl Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Send + 'static,
) -> impl Stream<Item = ChatEvent> + Send + 'static {
    futures_util::stream::unfold(
        (
            byte_stream.boxed(),
            String::new(),
            Vec::<ToolAccumulator>::new(),
            UsageAccumulator::default(),
        ),
        |(mut stream, mut buffer, mut tools, mut usage)| async move {
            loop {
                if !tools.is_empty() {
                    let tool = tools.remove(0);
                    return Some((
                        ChatEvent::ToolCall { id: tool.id, name: tool.name, arguments: tool.args },
                        (stream, buffer, tools, usage),
                    ));
                }

                if let Some(end) = buffer.find("\n\n") {
                    let block = buffer[..end].to_string();
                    buffer.drain(..end + 2);

                    let (event_type, data) = parse_sse_block(&block);
                    if let Some(out) = handle_event(&event_type, &data, &mut tools, &mut usage) {
                        return Some((out, (stream, buffer, tools, usage)));
                    }
                    continue;
                }

                match stream.next().await {
                    Some(Ok(bytes)) => {
                        buffer.push_str(&String::from_utf8_lossy(&bytes));
                    }
                    _ => {
                        if !tools.is_empty() {
                            let tool = tools.remove(0);
                            return Some((
                                ChatEvent::ToolCall {
                                    id: tool.id,
                                    name: tool.name,
                                    arguments: tool.args,
                                },
                                (stream, buffer, tools, usage),
                            ));
                        }
                        return None;
                    }
                }
            }
        },
    )
}

fn parse_sse_block(block: &str) -> (String, String) {
    let mut event_type = String::new();
    let mut data = String::new();
    for line in block.lines() {
        if let Some(et) = line.strip_prefix("event: ") {
            event_type = et.trim().to_string();
        } else if let Some(d) = line.strip_prefix("data: ") {
            data = d.to_string();
        }
    }
    (event_type, data)
}

fn handle_event(
    event_type: &str,
    data: &str,
    tools: &mut Vec<ToolAccumulator>,
    usage: &mut UsageAccumulator,
) -> Option<ChatEvent> {
    match event_type {
        "content_block_start" => {
            if let Ok(cbs) = serde_json::from_str::<ContentBlockStart>(data) {
                if let ContentBlock::ToolUse { id, name } = cbs.content_block {
                    tools.push(ToolAccumulator { index: cbs.index, id, name, args: String::new() });
                }
            }
            None
        }
        "content_block_delta" => handle_content_block_delta(data, tools),
        "content_block_stop" => None,
        "message_stop" => Some(handle_message_stop(tools, usage)),
        "message_start" => {
            absorb_message_start_usage(data, usage);
            None
        }
        "message_delta" => {
            absorb_message_delta_usage(data, usage);
            None
        }
        "ping" => None,
        "error" => Some(ChatEvent::Delta(format!("Claude SSE error: {data}"))),
        _ => None,
    }
}

fn handle_content_block_delta(data: &str, tools: &mut [ToolAccumulator]) -> Option<ChatEvent> {
    let cbd = serde_json::from_str::<ContentBlockDelta>(data).ok()?;
    let delta = cbd.delta?;
    match delta {
        DeltaContent::TextDelta { text } => {
            if text.is_empty() { None } else { Some(ChatEvent::Delta(text)) }
        }
        DeltaContent::InputJsonDelta { partial_json } => {
            if let Some(tool) = tools.last_mut() {
                tool.args.push_str(&partial_json);
            }
            None
        }
        DeltaContent::ThinkingDelta { thinking } => {
            if thinking.is_empty() { None } else { Some(ChatEvent::ThinkingDelta(thinking)) }
        }
        DeltaContent::SignatureDelta { .. } => None,
    }
}

fn handle_message_stop(
    tools: &mut Vec<ToolAccumulator>,
    usage: &mut UsageAccumulator,
) -> ChatEvent {
    if !tools.is_empty() {
        let tool = tools.remove(0);
        return ChatEvent::ToolCall { id: tool.id, name: tool.name, arguments: tool.args };
    }
    let has_data = usage.input_tokens > 0
        || usage.output_tokens > 0
        || usage.cache_creation_input_tokens > 0
        || usage.cache_read_input_tokens > 0;
    if has_data {
        let evt = ChatEvent::Usage {
            input_tokens: usage.input_tokens,
            output_tokens: usage.output_tokens,
            cache_creation_input_tokens: usage.cache_creation_input_tokens,
            cache_read_input_tokens: usage.cache_read_input_tokens,
            truncated: usage.truncated,
        };
        *usage = UsageAccumulator::default();
        return evt;
    }
    ChatEvent::Done
}

fn absorb_message_start_usage(data: &str, usage: &mut UsageAccumulator) {
    let Ok(val) = serde_json::from_str::<serde_json::Value>(data) else {
        return;
    };
    let Some(u) = val.get("message").and_then(|m| m.get("usage")) else {
        return;
    };
    usage.input_tokens = read_u32(u, "input_tokens");
    usage.cache_creation_input_tokens = read_u32(u, "cache_creation_input_tokens");
    usage.cache_read_input_tokens = read_u32(u, "cache_read_input_tokens");
}

fn absorb_message_delta_usage(data: &str, usage: &mut UsageAccumulator) {
    let Ok(val) = serde_json::from_str::<serde_json::Value>(data) else {
        return;
    };
    if let Some(u) = val.get("usage") {
        usage.output_tokens = read_u32(u, "output_tokens");
    }
    if val.get("delta").and_then(|d| d.get("stop_reason")).and_then(|s| s.as_str())
        == Some("max_tokens")
    {
        usage.truncated = true;
    }
}
