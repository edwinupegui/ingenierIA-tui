use std::collections::VecDeque;

use futures_util::{Stream, StreamExt};
use serde::Deserialize;

use super::ChatEvent;

// ── OpenAI-compatible SSE types ─────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct ToolCallDelta {
    index: Option<usize>,
    id: Option<String>,
    function: Option<ToolCallFnDelta>,
}

#[derive(Debug, Deserialize)]
struct ToolCallFnDelta {
    name: Option<String>,
    arguments: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ChatDelta {
    content: Option<String>,
    tool_calls: Option<Vec<ToolCallDelta>>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    delta: ChatDelta,
    #[expect(dead_code, reason = "field populated by OpenAI-compatible SSE deserialization")]
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ChatChunk {
    choices: Vec<ChatChoice>,
}

// ── Parser ──────────────────────────────────────────────────────────────────

/// Parses an OpenAI-compatible SSE byte stream into a stream of ChatEvents.
///
/// Works with both Copilot and any OpenAI-compatible API.
/// Handles `data: [DONE]`, content deltas, and streaming tool calls.
pub fn parse_sse_stream(
    byte_stream: impl Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Send + 'static,
) -> impl Stream<Item = ChatEvent> + Send + 'static {
    // State del unfold:
    // - stream: byte stream boxed.
    // - buffer: acumulador SSE entre frames.
    // - pending_tools: tool calls parcialmente ensamblados.
    // - pending_events: cola de eventos listos por emitir (>=1 cuando un [DONE]
    //   o un EOF drena multiples tool calls y debemos emitirlos en iteraciones
    //   sucesivas junto con el Done/None final).
    futures_util::stream::unfold(
        (
            byte_stream.boxed(),
            String::new(),
            Vec::<(String, String, String)>::new(),
            VecDeque::<ChatEvent>::new(),
            false, // stream_ended: tras EOF seguimos drenando pending_events y luego None.
        ),
        |(mut stream, mut buffer, mut pending_tools, mut pending_events, mut stream_ended)| async move {
            loop {
                // Primero drenamos eventos pendientes (tool calls acumulados + Done).
                if let Some(event) = pending_events.pop_front() {
                    return Some((
                        event,
                        (stream, buffer, pending_tools, pending_events, stream_ended),
                    ));
                }
                if stream_ended {
                    return None;
                }

                // Try to parse complete SSE blocks from buffer
                if let Some(end) = buffer.find("\n\n") {
                    let block = buffer[..end].to_string();
                    buffer.drain(..end + 2);

                    for line in block.lines() {
                        let Some(data) = line.strip_prefix("data: ") else {
                            continue;
                        };

                        if data.trim() == "[DONE]" {
                            for ev in drain_tool_calls(&mut pending_tools) {
                                pending_events.push_back(ev);
                            }
                            pending_events.push_back(ChatEvent::Done);
                            let event = pending_events
                                .pop_front()
                                .expect("pending_events has at least Done");
                            return Some((
                                event,
                                (stream, buffer, pending_tools, pending_events, stream_ended),
                            ));
                        }

                        if let Ok(chunk) = serde_json::from_str::<ChatChunk>(data) {
                            for choice in &chunk.choices {
                                if let Some(content) = &choice.delta.content {
                                    if !content.is_empty() {
                                        let event = ChatEvent::Delta(content.clone());
                                        return Some((
                                            event,
                                            (
                                                stream,
                                                buffer,
                                                pending_tools,
                                                pending_events,
                                                stream_ended,
                                            ),
                                        ));
                                    }
                                }
                                if let Some(tool_calls) = &choice.delta.tool_calls {
                                    accumulate_tool_calls(&mut pending_tools, tool_calls);
                                }
                            }
                        }
                    }
                    continue;
                }

                // Need more data from the stream
                match stream.next().await {
                    Some(Ok(bytes)) => {
                        buffer.push_str(&String::from_utf8_lossy(&bytes));
                    }
                    other => {
                        // Stream ended — emit remaining tool calls.
                        // Log si terminó sin [DONE] (abort del server, 200 con body
                        // vacío, etc.) para poder diagnosticar silencios.
                        let had_err = matches!(other, Some(Err(_)));
                        tracing::warn!(
                            pending_tools = pending_tools.len(),
                            buffer_len = buffer.len(),
                            had_err,
                            "SSE stream ended without [DONE]"
                        );
                        stream_ended = true;
                        for ev in drain_tool_calls(&mut pending_tools) {
                            pending_events.push_back(ev);
                        }
                        // No emitimos Done sintetico aqui: drive_stream interpreta
                        // None como "stream murio sin contenido" si no hubo eventos.
                        continue;
                    }
                }
            }
        },
    )
}

fn accumulate_tool_calls(pending: &mut Vec<(String, String, String)>, deltas: &[ToolCallDelta]) {
    for tc_delta in deltas {
        let idx = tc_delta.index.unwrap_or(0);
        while pending.len() <= idx {
            pending.push((String::new(), String::new(), String::new()));
        }
        if let Some(ref id) = tc_delta.id {
            pending[idx].0 = id.clone();
        }
        if let Some(ref f) = tc_delta.function {
            if let Some(ref name) = f.name {
                pending[idx].1 = name.clone();
            }
            if let Some(ref args) = f.arguments {
                pending[idx].2.push_str(args);
            }
        }
    }
}

fn drain_tool_calls(pending: &mut Vec<(String, String, String)>) -> Vec<ChatEvent> {
    pending
        .drain(..)
        .filter(|(id, _, _)| !id.is_empty())
        .map(|(id, name, args)| ChatEvent::ToolCall { id, name, arguments: args })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
    use futures_util::stream;

    // reqwest::Error no es construible manualmente; usamos infallible y
    // mapeamos a Result<Bytes, reqwest::Error> via .map(Ok) cuando nunca falla.
    fn bytes_stream(
        frames: Vec<&'static str>,
    ) -> impl Stream<Item = Result<Bytes, reqwest::Error>> {
        stream::iter(frames.into_iter().map(|s| Ok(Bytes::from_static(s.as_bytes()))))
    }

    async fn collect_events(
        byte_stream: impl Stream<Item = Result<Bytes, reqwest::Error>> + Send + 'static,
    ) -> Vec<ChatEvent> {
        parse_sse_stream(byte_stream).collect::<Vec<_>>().await
    }

    #[tokio::test]
    async fn multiple_tool_calls_drained_at_done() {
        // SSE con 3 tool calls acumulados (indices 0,1,2) + [DONE].
        let frames = vec![
            concat!(
                "data: {\"choices\":[{\"delta\":{\"tool_calls\":[",
                "{\"index\":0,\"id\":\"t0\",\"function\":{\"name\":\"read\",\"arguments\":\"{}\"}},",
                "{\"index\":1,\"id\":\"t1\",\"function\":{\"name\":\"write\",\"arguments\":\"{}\"}},",
                "{\"index\":2,\"id\":\"t2\",\"function\":{\"name\":\"bash\",\"arguments\":\"{}\"}}",
                "]}}]}\n\n",
            ),
            "data: [DONE]\n\n",
        ];
        let events = collect_events(bytes_stream(frames)).await;
        let tool_calls: Vec<_> = events
            .iter()
            .filter_map(|e| match e {
                ChatEvent::ToolCall { id, .. } => Some(id.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(tool_calls, vec!["t0", "t1", "t2"], "deben emitirse los 3 tool calls");
        assert_eq!(
            events.iter().filter(|e| matches!(e, ChatEvent::Done)).count(),
            1,
            "Done debe emitirse una sola vez"
        );
        // Done debe venir al final.
        assert!(matches!(events.last(), Some(ChatEvent::Done)));
    }

    #[tokio::test]
    async fn multiple_tool_calls_drained_at_eof_without_done() {
        // Stream termina sin [DONE] pero con 2 tool calls pendientes.
        let frames = vec![concat!(
            "data: {\"choices\":[{\"delta\":{\"tool_calls\":[",
            "{\"index\":0,\"id\":\"a0\",\"function\":{\"name\":\"glob\",\"arguments\":\"{}\"}},",
            "{\"index\":1,\"id\":\"a1\",\"function\":{\"name\":\"grep\",\"arguments\":\"{}\"}}",
            "]}}]}\n\n",
        )];
        let events = collect_events(bytes_stream(frames)).await;
        let ids: Vec<_> = events
            .iter()
            .filter_map(|e| match e {
                ChatEvent::ToolCall { id, .. } => Some(id.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(ids, vec!["a0", "a1"], "ambos tool calls deben emitirse al EOF");
        // Sin [DONE] no debe sintetizarse Done (drive_stream decide con got_content).
        assert!(!events.iter().any(|e| matches!(e, ChatEvent::Done)));
    }

    #[tokio::test]
    async fn single_delta_then_done_still_works() {
        // Regresion: path basico no debe romperse con el cambio.
        let frames = vec![
            "data: {\"choices\":[{\"delta\":{\"content\":\"hola\"}}]}\n\n",
            "data: [DONE]\n\n",
        ];
        let events = collect_events(bytes_stream(frames)).await;
        assert!(matches!(&events[0], ChatEvent::Delta(s) if s == "hola"));
        assert!(matches!(events.last(), Some(ChatEvent::Done)));
    }
}
