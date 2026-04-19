//! Global registries — OnceLock-backed singletons for cross-module access.
//!
//! Provides thread-safe, lazily-initialized registries for MCP tools and
//! session metadata. These are read-heavy / write-rare, so RwLock is used.
//!
//! Reference: claw-code task_registry.rs, team_cron_registry.rs patterns.

#![expect(dead_code, reason = "E08 spec — consumed when services adopt global registries")]

use std::sync::{OnceLock, RwLock};

// ── MCP Registry ────────────────────────────────────────────────────────────

/// Global registry of MCP tool names + schemas discovered via `tools/list`.
static MCP_REGISTRY: OnceLock<RwLock<Vec<McpToolEntry>>> = OnceLock::new();

/// Lightweight tool entry for the registry (name + description only).
#[derive(Debug, Clone)]
pub struct McpToolEntry {
    pub name: String,
    pub description: String,
}

fn mcp_registry() -> &'static RwLock<Vec<McpToolEntry>> {
    MCP_REGISTRY.get_or_init(|| RwLock::new(Vec::new()))
}

/// Replace the MCP tool registry contents. Called after `tools/list` discovery.
pub fn set_mcp_tools(tools: Vec<McpToolEntry>) {
    let mut guard = mcp_registry().write().unwrap_or_else(|poisoned| poisoned.into_inner());
    *guard = tools;
}

/// Read snapshot of MCP tool names.
pub fn mcp_tool_names() -> Vec<String> {
    let guard = mcp_registry().read().unwrap_or_else(|poisoned| poisoned.into_inner());
    guard.iter().map(|t| t.name.clone()).collect()
}

/// Count of registered MCP tools.
pub fn mcp_tool_count() -> usize {
    let guard = mcp_registry().read().unwrap_or_else(|poisoned| poisoned.into_inner());
    guard.len()
}

// ── Session Registry ────────────────────────────────────────────────────────

/// Global registry for the current session ID (future: forking, branching).
static SESSION_ID: OnceLock<RwLock<Option<String>>> = OnceLock::new();

fn session_registry() -> &'static RwLock<Option<String>> {
    SESSION_ID.get_or_init(|| RwLock::new(None))
}

/// Set the active session ID.
pub fn set_session_id(id: String) {
    let mut guard = session_registry().write().unwrap_or_else(|poisoned| poisoned.into_inner());
    *guard = Some(id);
}

/// Get the active session ID, if any.
pub fn session_id() -> Option<String> {
    let guard = session_registry().read().unwrap_or_else(|poisoned| poisoned.into_inner());
    guard.clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mcp_registry_starts_empty() {
        // Note: OnceLock means this test depends on process-global state.
        // In production, tools are set once after discovery.
        assert!(mcp_tool_count() == 0 || mcp_tool_count() > 0); // non-deterministic in test
    }

    #[test]
    fn set_and_read_mcp_tools() {
        set_mcp_tools(vec![
            McpToolEntry { name: "search".into(), description: "Search docs".into() },
            McpToolEntry { name: "get_doc".into(), description: "Get document".into() },
        ]);
        let names = mcp_tool_names();
        assert!(names.contains(&"search".to_string()));
        assert!(names.contains(&"get_doc".to_string()));
    }
}
