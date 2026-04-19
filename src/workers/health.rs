use crate::{actions::Action, services::IngenieriaClient};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc::Sender;

const POLL_INTERVAL_SECS: u64 = 5;

pub async fn run(client: Arc<IngenieriaClient>, tx: Sender<Action>) {
    // Primera llamada inmediata para que el servidor aparezca Online lo antes posible
    poll_once(&client, &tx).await;

    let mut interval = tokio::time::interval(Duration::from_secs(POLL_INTERVAL_SECS));
    interval.tick().await; // consume el tick inicial inmediato del interval

    loop {
        interval.tick().await;
        if !poll_once(&client, &tx).await {
            break;
        }
    }
}

/// Hace un health check y envía la Action correspondiente.
/// Retorna false si el canal está cerrado (app terminando).
async fn poll_once(client: &IngenieriaClient, tx: &Sender<Action>) -> bool {
    let action = match client.health().await {
        Ok(health) => {
            tracing::debug!(docs = health.docs.total, "Health OK");
            Action::HealthUpdated(health)
        }
        Err(e) => {
            tracing::debug!(error = %e, "Health check failed");
            Action::HealthFetchFailed(e.to_string())
        }
    };
    tx.send(action).await.is_ok()
}
