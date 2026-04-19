use crate::services::mcp::McpClient;

const CHECKS: &[&str] = &["security", "testing", "coding-standards", "adrs"];

/// Run all compliance checks via MCP validate_compliance tool.
/// Returns a markdown report with results.
pub async fn validate_all(base_url: &str, factory: &str) -> anyhow::Result<String> {
    let mcp = McpClient::connect(base_url).await?;

    let result = mcp
        .call_tool(
            "validate_compliance",
            serde_json::json!({ "factory": factory, "check_against": CHECKS }),
        )
        .await?;

    Ok(result)
}
