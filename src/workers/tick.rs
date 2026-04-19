use crate::actions::Action;
use std::time::Duration;
use tokio::sync::mpsc::Sender;

const TICK_INTERVAL: Duration = Duration::from_millis(250);

pub async fn run(tx: Sender<Action>) {
    let mut interval = tokio::time::interval(TICK_INTERVAL);
    loop {
        interval.tick().await;
        if tx.send(Action::Tick).await.is_err() {
            break;
        }
    }
}
