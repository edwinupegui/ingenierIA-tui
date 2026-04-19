use crate::services::mcp::McpClient;

/// Call sync_project via MCP, parse the markdown response to extract updated URIs.
/// Returns (updated_uris, server_last_update_timestamp).
pub async fn sync_via_mcp(
    base_url: &str,
    factory: &str,
    last_sync_date: Option<&str>,
) -> anyhow::Result<(Vec<String>, String)> {
    let mcp = McpClient::connect(base_url).await?;

    let mut args = serde_json::json!({ "factory": factory });
    if let Some(date) = last_sync_date {
        args["last_sync_date"] = serde_json::Value::String(date.to_string());
    }

    let text = mcp.call_tool("sync_project", args).await?;

    // Parse the markdown response from sync_project.
    // Lines like "- `ingenieria://type/factory/name` ..." contain the URIs.
    let mut uris = Vec::new();
    let mut server_ts = String::new();

    for line in text.lines() {
        // Extract URIs: lines containing `ingenieria://...`
        if let Some(start) = line.find("ingenieria://") {
            let rest = &line[start..];
            let end = rest.find(['`', ' ', ')']).unwrap_or(rest.len());
            let uri = &rest[..end];
            if uri.len() > "ingenieria://".len() {
                uris.push(uri.to_string());
            }
        }
        // Extract server timestamp: "Last update: ..." or "**Last update:**"
        let lower = line.to_lowercase();
        if lower.contains("last update") || lower.contains("última actualización") {
            if let Some(ts) = extract_timestamp(line) {
                server_ts = ts;
            }
        }
    }

    // If no explicit timestamp found, use current time as the sync point
    if server_ts.is_empty() {
        server_ts = now_iso();
    }

    Ok((uris, server_ts))
}

fn extract_timestamp(line: &str) -> Option<String> {
    // Look for ISO-like timestamp in the line (e.g. 2026-03-28T10:30:00.000Z)
    let chars: Vec<char> = line.chars().collect();
    for i in 0..chars.len().saturating_sub(19) {
        if chars[i].is_ascii_digit()
            && chars.get(i + 4) == Some(&'-')
            && chars.get(i + 7) == Some(&'-')
            && chars.get(i + 10) == Some(&'T')
        {
            let end = chars[i..]
                .iter()
                .position(|&c| c == '`' || c == ' ' || c == ')' || c == '\n')
                .map(|p| i + p)
                .unwrap_or(chars.len());
            let ts: String = chars[i..end].iter().collect();
            if ts.len() >= 19 {
                return Some(ts);
            }
        }
    }
    None
}

pub fn now_iso() -> String {
    let now =
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
    let secs = now.as_secs();
    let days = secs / 86400;
    let time_of_day = secs % 86400;
    let hours = time_of_day / 3600;
    let minutes = (time_of_day % 3600) / 60;
    let seconds = time_of_day % 60;

    let (year, month, day) = days_to_date(days);
    format!("{year:04}-{month:02}-{day:02}T{hours:02}:{minutes:02}:{seconds:02}Z")
}

fn days_to_date(days: u64) -> (u64, u64, u64) {
    let mut y = 1970;
    let mut remaining = days;
    loop {
        let days_in_year = if is_leap(y) { 366 } else { 365 };
        if remaining < days_in_year {
            break;
        }
        remaining -= days_in_year;
        y += 1;
    }
    let months = if is_leap(y) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut m = 1u64;
    for days_in_month in months {
        if remaining < days_in_month {
            break;
        }
        remaining -= days_in_month;
        m += 1;
    }
    (y, m, remaining + 1)
}

fn is_leap(y: u64) -> bool {
    y.is_multiple_of(4) && (!y.is_multiple_of(100) || y.is_multiple_of(400))
}
