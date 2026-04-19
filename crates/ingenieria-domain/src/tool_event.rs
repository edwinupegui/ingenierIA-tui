use serde::Deserialize;

/// A tool invocation event from the MCP server's /api/tool-events SSE stream.
#[derive(Debug, Clone, Deserialize)]
pub struct ToolEvent {
    #[serde(rename = "type")]
    pub event_type: String,
    pub tool: String,
    pub factory: Option<String>,
    pub duration_ms: Option<u64>,
    #[allow(dead_code, reason = "field populated by MCP /api/tool-events SSE deserialization")]
    pub error: Option<String>,
    pub timestamp: String,
}

impl ToolEvent {
    #[allow(dead_code)]
    pub fn is_invoke(&self) -> bool {
        self.event_type == "tool:invoke"
    }

    pub fn is_complete(&self) -> bool {
        self.event_type == "tool:complete"
    }

    pub fn is_error(&self) -> bool {
        self.event_type == "tool:error"
    }

    #[allow(dead_code, reason = "convenience method for tool monitor UI status display")]
    pub fn status_label(&self) -> &str {
        match self.event_type.as_str() {
            "tool:invoke" => "pending",
            "tool:complete" => "ok",
            "tool:error" => "error",
            _ => "unknown",
        }
    }
}
