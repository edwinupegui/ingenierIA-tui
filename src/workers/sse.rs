use crate::{actions::Action, domain::event::IngenieriaEvent};
use futures_util::StreamExt;
use std::time::Duration;
use tokio::sync::mpsc::Sender;

use super::MAX_BACKOFF_SECS;

pub async fn run(events_url: String, developer: String, model: String, tx: Sender<Action>) {
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .build()
        .unwrap_or_default();
    let mut backoff_secs: u64 = 1;

    loop {
        tracing::debug!(url = %events_url, "Connecting to SSE");

        let url = format!("{events_url}?dev={developer}&model={model}");
        match connect_sse(&client, &url, &tx).await {
            Ok(()) => tracing::debug!("SSE stream ended cleanly"),
            Err(e) => tracing::debug!(error = %e, backoff = backoff_secs, "SSE error"),
        }

        if tx.send(Action::SseDisconnected).await.is_err() {
            break;
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

async fn connect_sse(
    client: &reqwest::Client,
    url: &str,
    tx: &Sender<Action>,
) -> anyhow::Result<()> {
    let resp =
        client.get(url).header("Accept", "text/event-stream").send().await?.error_for_status()?;

    let mut stream = resp.bytes_stream();
    let mut buffer = String::new();

    while let Some(chunk) = stream.next().await {
        let bytes = chunk?;
        buffer.push_str(&String::from_utf8_lossy(&bytes));

        // El protocolo SSE separa mensajes con "\n\n"
        while let Some(end) = buffer.find("\n\n") {
            let block = buffer[..end].to_string();
            buffer.drain(..end + 2);

            // Extraer la línea "data: ..."
            let data_line =
                block.lines().find_map(|line| line.strip_prefix("data: ").map(str::to_string));

            if let Some(data) = data_line {
                match serde_json::from_str::<IngenieriaEvent>(&data) {
                    Ok(event) => {
                        if tx.send(Action::ServerEvent(event)).await.is_err() {
                            return Ok(());
                        }
                    }
                    Err(e) => {
                        tracing::warn!(data = %data, error = %e, "Failed to parse SSE event");
                    }
                }
            }
        }
    }

    Ok(())
}
