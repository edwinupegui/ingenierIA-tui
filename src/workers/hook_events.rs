use crate::{actions::Action, domain::hook_event::HookEvent};
use futures_util::StreamExt;
use std::time::Duration;
use tokio::sync::mpsc::Sender;

use super::MAX_BACKOFF_SECS;

pub async fn run(base_url: String, tx: Sender<Action>) {
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .build()
        .unwrap_or_default();
    let url = format!("{}/api/hook-events", base_url.trim_end_matches('/'));
    let mut backoff_secs: u64 = 2;

    loop {
        tracing::debug!(url = %url, "Connecting to hook-events SSE");

        match connect(&client, &url, &tx).await {
            Ok(()) => {
                backoff_secs = 2;
            }
            Err(e) => {
                tracing::debug!(error = %e, "Hook events SSE error");
            }
        }

        let jitter_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos() % 1000)
            .unwrap_or(0) as u64;
        tokio::time::sleep(Duration::from_secs(backoff_secs) + Duration::from_millis(jitter_ms))
            .await;
        backoff_secs = (backoff_secs * 2).min(MAX_BACKOFF_SECS);
    }
}

async fn connect(client: &reqwest::Client, url: &str, tx: &Sender<Action>) -> anyhow::Result<()> {
    let resp =
        client.get(url).header("Accept", "text/event-stream").send().await?.error_for_status()?;

    let mut stream = resp.bytes_stream();
    let mut buffer = String::new();

    while let Some(chunk) = stream.next().await {
        let bytes = chunk?;
        buffer.push_str(&String::from_utf8_lossy(&bytes));

        while let Some(end) = buffer.find("\n\n") {
            let block = buffer[..end].to_string();
            buffer.drain(..end + 2);

            let data_line =
                block.lines().find_map(|line| line.strip_prefix("data: ").map(str::to_string));

            if let Some(data) = data_line {
                if let Ok(event) = serde_json::from_str::<HookEvent>(&data) {
                    if tx.send(Action::HookEventReceived(event)).await.is_err() {
                        return Ok(());
                    }
                }
            }
        }
    }

    Ok(())
}
