//! MockChatProvider (E21) — implementa `ChatProvider` con escenarios
//! deterministas para testing de CI y demos offline.
//!
//! No hace IO de red ni usa credenciales. Los escenarios estan hardcoded
//! y el orden de eventos es 100% determinista tick-a-tick.
//!
//! Escenarios:
//! - `SimpleResponse` — unico Delta + Done.
//! - `StreamingResponse` — varios Deltas para simular chunking.
//! - `ToolCall` — Delta breve + ToolCall + Done (provoca un tool loop).
//! - `StreamingError` — 1 Delta + error (anyhow en stream_chat).
//! - `MultiTurn` — Usage + multiples Deltas + Done.
//!
//! Uso:
//! ```ignore
//! let provider = MockChatProvider::with_scenario(MockScenario::SimpleResponse);
//! let stream = provider.stream_chat("mock", &messages, &[]).await?;
//! ```
//!
//! Integracion CLI:
//! - `--mock` en CLI fuerza este provider (ver `spawners_chat::resolve_provider`).
//! - El escenario default es `SimpleResponse`, override via env
//!   `INGENIERIA_MOCK_SCENARIO=tool_call|streaming|multi_turn|error|simple`.

use std::pin::Pin;

use futures_util::{stream, Stream};

use super::{ChatEvent, ChatProvider, ChatStream, ModelInfo, ToolDefinition};
use crate::state::ChatMessage;

/// Escenarios predefinidos cubriendo los caminos criticos del chat loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MockScenario {
    /// Una sola respuesta corta y cierre.
    SimpleResponse,
    /// Respuesta fragmentada en varios deltas (simula streaming real).
    StreamingResponse,
    /// Un tool call seguido de Done (el TUI debe lanzar ejecucion de tools).
    ToolCall,
    /// El provider falla al abrir el stream (anyhow::Error).
    StreamingError,
    /// Usage + deltas + done (ejercita cost tracking + OTPS metrics).
    MultiTurn,
}

impl MockScenario {
    /// Parsea desde env var o CLI token. Retorna `SimpleResponse` si no matchea.
    pub fn from_env_or_default() -> Self {
        match std::env::var("INGENIERIA_MOCK_SCENARIO").ok().as_deref() {
            Some("streaming") => Self::StreamingResponse,
            Some("tool_call") | Some("tool") => Self::ToolCall,
            Some("error") | Some("streaming_error") => Self::StreamingError,
            Some("multi_turn") | Some("multi") => Self::MultiTurn,
            _ => Self::SimpleResponse,
        }
    }

    #[allow(dead_code, reason = "consumido por tests + integracion tests/mock_provider.rs")]
    pub fn label(self) -> &'static str {
        match self {
            Self::SimpleResponse => "simple",
            Self::StreamingResponse => "streaming",
            Self::ToolCall => "tool_call",
            Self::StreamingError => "streaming_error",
            Self::MultiTurn => "multi_turn",
        }
    }
}

/// Genera la secuencia de eventos para un escenario dado. Para
/// `StreamingError` el caller debe devolver `Err` desde `stream_chat`;
/// aqui se retorna un stream vacio como placeholder.
fn events_for(scenario: MockScenario) -> Vec<ChatEvent> {
    match scenario {
        MockScenario::SimpleResponse => vec![
            ChatEvent::Delta("Hola, soy una respuesta mock.".into()),
            ChatEvent::Usage {
                input_tokens: 10,
                output_tokens: 6,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 0,
                truncated: false,
            },
            ChatEvent::Done,
        ],
        MockScenario::StreamingResponse => vec![
            ChatEvent::Delta("Primera ".into()),
            ChatEvent::Delta("parte ".into()),
            ChatEvent::Delta("de la ".into()),
            ChatEvent::Delta("respuesta ".into()),
            ChatEvent::Delta("streaming.".into()),
            ChatEvent::Usage {
                input_tokens: 20,
                output_tokens: 8,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 0,
                truncated: false,
            },
            ChatEvent::Done,
        ],
        MockScenario::ToolCall => vec![
            ChatEvent::Delta("Voy a leer un archivo:".into()),
            ChatEvent::ToolCall {
                id: "mock_tool_1".into(),
                name: "read_file".into(),
                arguments: r#"{"path":"src/main.rs"}"#.into(),
            },
            ChatEvent::Done,
        ],
        MockScenario::StreamingError => vec![],
        MockScenario::MultiTurn => vec![
            ChatEvent::Delta("Respuesta extensa ".into()),
            ChatEvent::Delta("con multiples chunks ".into()),
            ChatEvent::Delta("y usage detallado.".into()),
            ChatEvent::Usage {
                input_tokens: 100,
                output_tokens: 30,
                cache_creation_input_tokens: 15,
                cache_read_input_tokens: 50,
                truncated: false,
            },
            ChatEvent::Done,
        ],
    }
}

/// Provider mock determinista. No mantiene estado entre calls (cada
/// `stream_chat` re-emite la secuencia completa del escenario).
pub struct MockChatProvider {
    scenario: MockScenario,
}

impl MockChatProvider {
    /// Construye con un escenario explicito.
    #[allow(dead_code, reason = "API publica para tests de integracion + demos programaticas")]
    pub fn with_scenario(scenario: MockScenario) -> Self {
        Self { scenario }
    }

    /// Construye resolviendo el escenario desde env var.
    pub fn from_env() -> Self {
        Self { scenario: MockScenario::from_env_or_default() }
    }

    #[allow(dead_code, reason = "getter usado por tests + debugging")]
    pub fn scenario(&self) -> MockScenario {
        self.scenario
    }
}

#[async_trait::async_trait]
impl ChatProvider for MockChatProvider {
    async fn stream_chat(
        &self,
        _model: &str,
        _messages: &[ChatMessage],
        _tools: &[ToolDefinition],
    ) -> anyhow::Result<ChatStream> {
        if matches!(self.scenario, MockScenario::StreamingError) {
            anyhow::bail!("MockChatProvider: streaming error injected for testing");
        }
        let events = events_for(self.scenario);
        let s: Pin<Box<dyn Stream<Item = ChatEvent> + Send>> = Box::pin(stream::iter(events));
        Ok(s)
    }

    async fn list_models(&self) -> anyhow::Result<Vec<ModelInfo>> {
        Ok(vec![ModelInfo { id: "mock-default".into(), display_name: "Mock Model".into() }])
    }

    fn provider_name(&self) -> &str {
        "MockChatProvider"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::StreamExt;

    fn msg() -> Vec<ChatMessage> {
        vec![]
    }

    #[tokio::test]
    async fn simple_response_emits_delta_usage_done() {
        let p = MockChatProvider::with_scenario(MockScenario::SimpleResponse);
        let mut stream = p.stream_chat("mock", &msg(), &[]).await.unwrap();
        let mut events = Vec::new();
        while let Some(e) = stream.next().await {
            events.push(e);
        }
        assert!(events.iter().any(|e| matches!(e, ChatEvent::Delta(_))));
        assert!(events.iter().any(|e| matches!(e, ChatEvent::Usage { .. })));
        assert!(matches!(events.last(), Some(ChatEvent::Done)));
    }

    #[tokio::test]
    async fn streaming_response_emits_multiple_deltas() {
        let p = MockChatProvider::with_scenario(MockScenario::StreamingResponse);
        let mut stream = p.stream_chat("mock", &msg(), &[]).await.unwrap();
        let mut delta_count = 0;
        while let Some(e) = stream.next().await {
            if matches!(e, ChatEvent::Delta(_)) {
                delta_count += 1;
            }
        }
        assert!(delta_count >= 3, "streaming debe emitir multiples deltas, got {delta_count}");
    }

    #[tokio::test]
    async fn tool_call_scenario_emits_tool_call_event() {
        let p = MockChatProvider::with_scenario(MockScenario::ToolCall);
        let mut stream = p.stream_chat("mock", &msg(), &[]).await.unwrap();
        let mut saw_tool = false;
        while let Some(e) = stream.next().await {
            if let ChatEvent::ToolCall { name, .. } = &e {
                assert_eq!(name, "read_file");
                saw_tool = true;
            }
        }
        assert!(saw_tool);
    }

    #[tokio::test]
    async fn streaming_error_returns_err() {
        let p = MockChatProvider::with_scenario(MockScenario::StreamingError);
        let res = p.stream_chat("mock", &msg(), &[]).await;
        assert!(res.is_err());
    }

    #[tokio::test]
    async fn multi_turn_populates_cache_tokens() {
        let p = MockChatProvider::with_scenario(MockScenario::MultiTurn);
        let mut stream = p.stream_chat("mock", &msg(), &[]).await.unwrap();
        let mut found_cache = false;
        while let Some(e) = stream.next().await {
            if let ChatEvent::Usage {
                cache_read_input_tokens, cache_creation_input_tokens, ..
            } = e
            {
                assert!(cache_read_input_tokens > 0);
                assert!(cache_creation_input_tokens > 0);
                found_cache = true;
            }
        }
        assert!(found_cache, "MultiTurn debe reportar tokens de cache");
    }

    #[tokio::test]
    async fn list_models_returns_mock_default() {
        let p = MockChatProvider::with_scenario(MockScenario::SimpleResponse);
        let models = p.list_models().await.unwrap();
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].id, "mock-default");
    }

    #[test]
    fn provider_name_identifies_mock() {
        let p = MockChatProvider::with_scenario(MockScenario::SimpleResponse);
        assert_eq!(p.provider_name(), "MockChatProvider");
    }

    #[test]
    fn scenario_label_stable_for_ci() {
        assert_eq!(MockScenario::SimpleResponse.label(), "simple");
        assert_eq!(MockScenario::ToolCall.label(), "tool_call");
        assert_eq!(MockScenario::MultiTurn.label(), "multi_turn");
    }

    #[test]
    fn scenario_from_env_var_parses_known_aliases() {
        // Set + unset alrededor de cada check para no interferir con otros tests
        // del mismo proceso (cargo-test corre en paralelo).
        std::env::set_var("INGENIERIA_MOCK_SCENARIO", "tool");
        assert_eq!(MockScenario::from_env_or_default(), MockScenario::ToolCall);
        std::env::set_var("INGENIERIA_MOCK_SCENARIO", "streaming");
        assert_eq!(MockScenario::from_env_or_default(), MockScenario::StreamingResponse);
        std::env::set_var("INGENIERIA_MOCK_SCENARIO", "error");
        assert_eq!(MockScenario::from_env_or_default(), MockScenario::StreamingError);
        std::env::remove_var("INGENIERIA_MOCK_SCENARIO");
        assert_eq!(MockScenario::from_env_or_default(), MockScenario::SimpleResponse);
    }

    #[test]
    fn scenario_from_env_unknown_falls_back_to_simple() {
        std::env::set_var("INGENIERIA_MOCK_SCENARIO", "nonexistent");
        let s = MockScenario::from_env_or_default();
        std::env::remove_var("INGENIERIA_MOCK_SCENARIO");
        assert_eq!(s, MockScenario::SimpleResponse);
    }
}
