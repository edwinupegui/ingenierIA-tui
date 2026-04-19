//! Stream monitor: heartbeat, warning, and timeout during AI streaming.
//!
//! Wraps the main chat stream loop and emits periodic Actions so the UI
//! can show elapsed time, warn about slow responses, and offer retry on timeout.

use std::time::{Duration, Instant};

use tokio::sync::mpsc::Sender;

use crate::actions::Action;

/// Thresholds for stream monitoring.
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(3);
const WARNING_THRESHOLD: Duration = Duration::from_secs(15);
const TIMEOUT_THRESHOLD: Duration = Duration::from_secs(60);

/// Tracks elapsed time since the last delta and emits heartbeat/warning/timeout Actions.
pub struct StreamMonitor {
    tx: Sender<Action>,
    last_delta: Instant,
    start: Instant,
    warned: bool,
}

impl StreamMonitor {
    pub fn new(tx: Sender<Action>) -> Self {
        let now = Instant::now();
        Self { tx, last_delta: now, start: now, warned: false }
    }

    /// Call when a delta (content or tool call) arrives.
    pub fn on_delta(&mut self) {
        self.last_delta = Instant::now();
        self.warned = false;
    }

    /// Returns the interval to use for the next select! timeout.
    pub fn next_interval(&self) -> Duration {
        HEARTBEAT_INTERVAL
    }

    /// Check elapsed time since last delta and emit appropriate Actions.
    /// Returns `true` if the stream should be aborted (timeout).
    pub async fn tick(&mut self) -> bool {
        let since_delta = self.last_delta.elapsed();
        let total = self.start.elapsed();

        if since_delta >= TIMEOUT_THRESHOLD {
            let _ = self.tx.send(Action::StreamTimeout).await;
            return true;
        }

        if since_delta >= WARNING_THRESHOLD && !self.warned {
            self.warned = true;
            let _ = self.tx.send(Action::StreamWarning(total.as_secs() as u16)).await;
        }

        let _ = self.tx.send(Action::StreamHeartbeat(total.as_secs() as u16)).await;

        false
    }
}
