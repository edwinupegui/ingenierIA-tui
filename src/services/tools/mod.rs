mod bash;
pub mod config_tool;
mod edit;
mod fs;
mod fs_write;
mod glob;
pub mod todowrite;

use crate::services::chat::ToolDefinition;

// Re-export domain type for backward compatibility.
pub use ingenieria_domain::permissions::ToolPermission;

// ── Tool trait ──────────────────────────────────────────────────────────────

#[async_trait::async_trait]
pub trait Tool: Send + Sync {
    /// Unique tool name (matches the function name in the API).
    fn name(&self) -> &str;

    /// Permission level for this tool.
    fn permission(&self) -> ToolPermission;

    /// OpenAI-compatible tool definition for the chat API.
    fn definition(&self) -> ToolDefinition;

    /// Execute the tool with JSON arguments, return result as string.
    async fn execute(&self, arguments: &str) -> String;
}

// ── Registry ────────────────────────────────────────────────────────────────

pub struct ToolRegistry {
    tools: Vec<Box<dyn Tool>>,
}

impl ToolRegistry {
    /// Creates a registry pre-loaded with all built-in tools.
    pub fn new() -> Self {
        Self {
            tools: vec![
                Box::new(fs::ReadFileTool),
                Box::new(fs::ListDirectoryTool),
                Box::new(fs::SearchFilesTool),
                Box::new(fs_write::GrepFilesTool),
                Box::new(fs_write::WriteFileTool),
                Box::new(edit::EditFileTool),
                Box::new(bash::BashTool),
                Box::new(glob::GlobTool),
            ],
        }
    }

    /// Returns tool definitions for all registered tools.
    pub fn definitions(&self) -> Vec<ToolDefinition> {
        self.tools.iter().map(|t| t.definition()).collect()
    }

    /// Get the permission level for a tool by name.
    pub fn permission_for(&self, name: &str) -> Option<ToolPermission> {
        self.tools.iter().find(|t| t.name() == name).map(|t| t.permission())
    }

    /// Execute a tool by name. Returns None if the tool is not found.
    pub async fn execute(&self, name: &str, arguments: &str) -> Option<String> {
        for tool in &self.tools {
            if tool.name() == name {
                return Some(tool.execute(arguments).await);
            }
        }
        None
    }
}
