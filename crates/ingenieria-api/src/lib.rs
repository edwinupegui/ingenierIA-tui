//! Chat API types, pricing, retry, metrics, and stream parsing for ingenierIA TUI.
//!
//! This crate provides the shared types and pure-logic modules used by chat
//! providers. The `ChatProvider` trait itself stays in the main binary because
//! it depends on `ChatMessage` (which carries UI-specific fields).

#![allow(dead_code)]

use std::pin::Pin;

use futures_util::Stream;

// ── Shared types ───────────────────────────────────────────────────────────

/// A single event emitted by a streaming chat response.
#[derive(Debug, Clone)]
pub enum ChatEvent {
    /// Text content delta from the assistant.
    Delta(String),
    /// Thinking/reasoning content delta (Extended Thinking feature).
    ThinkingDelta(String),
    /// A tool call has been fully received (id, name, arguments JSON).
    ToolCall { id: String, name: String, arguments: String },
    /// Token usage report (emitted before Done).
    Usage {
        input_tokens: u32,
        output_tokens: u32,
        cache_creation_input_tokens: u32,
        cache_read_input_tokens: u32,
        /// True when the API returned stop_reason=max_tokens (response was cut).
        truncated: bool,
    },
    /// The stream is complete.
    Done,
}

/// A tool definition in OpenAI-compatible format.
#[derive(Debug, Clone)]
pub struct ToolDefinition {
    pub json: serde_json::Value,
}

/// Information about an available model.
#[derive(Debug, Clone)]
pub struct ModelInfo {
    pub id: String,
    pub display_name: String,
}

/// Boxed async stream of ChatEvents.
pub type ChatStream = Pin<Box<dyn Stream<Item = ChatEvent> + Send>>;

// ── Pure-logic modules ─────────────────────────────────────────────────────

pub mod metrics;
pub mod model_fallback;
pub mod pricing;
pub mod prompt_cache;
pub mod retry;
pub mod stall_detector;
