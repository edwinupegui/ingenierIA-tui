use serde::Deserialize;

/// A hook/enforcement event from the MCP server's /api/hook-events SSE stream.
#[derive(Debug, Clone, Deserialize)]
pub struct HookEvent {
    #[serde(rename = "type")]
    pub event_type: String,
    pub hook: String,
    pub factory: Option<String>,
    pub rule: Option<String>,
    #[allow(dead_code, reason = "field populated by MCP /api/hook-events SSE deserialization")]
    pub detail: Option<String>,
    pub timestamp: String,
}

impl HookEvent {
    pub fn is_block(&self) -> bool {
        self.event_type == "hook:block"
    }

    pub fn is_pass(&self) -> bool {
        self.event_type == "hook:pass"
    }

    #[allow(dead_code, reason = "convenience method for enforcement dashboard hook filtering")]
    pub fn is_check(&self) -> bool {
        self.event_type == "hook:check"
    }
}
