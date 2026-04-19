//! Worker lifecycle state machine — observable health for each background worker.
//!
//! Each spawned worker gets a `WorkerHandle` that tracks its state via a
//! `tokio::sync::watch` channel. The UI can poll all handles to show a
//! status bar indicator for worker health.

use std::fmt;

use tokio::sync::watch;

/// Lifecycle states for a worker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkerState {
    /// Worker is starting up.
    Starting,
    /// Worker is running normally.
    Running,
    /// Worker encountered an error, backing off before retry.
    Backoff { attempt: u32 },
    /// Worker has stopped (channel closed or explicit shutdown).
    Stopped,
}

impl fmt::Display for WorkerState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Starting => write!(f, "starting"),
            Self::Running => write!(f, "running"),
            Self::Backoff { attempt } => write!(f, "backoff #{attempt}"),
            Self::Stopped => write!(f, "stopped"),
        }
    }
}

/// Handle to observe and control a worker's lifecycle.
pub struct WorkerHandle {
    pub name: &'static str,
    state_tx: watch::Sender<WorkerState>,
    state_rx: watch::Receiver<WorkerState>,
}

impl WorkerHandle {
    /// Create a new handle with initial `Starting` state.
    pub fn new(name: &'static str) -> Self {
        let (state_tx, state_rx) = watch::channel(WorkerState::Starting);
        Self { name, state_tx, state_rx }
    }

    /// Get the sender for the worker to report state changes.
    pub fn state_sender(&self) -> watch::Sender<WorkerState> {
        self.state_tx.clone()
    }

    /// Read current state.
    pub fn state(&self) -> WorkerState {
        *self.state_rx.borrow()
    }

    /// Whether the worker is healthy (Starting or Running).
    pub fn is_healthy(&self) -> bool {
        matches!(self.state(), WorkerState::Starting | WorkerState::Running)
    }
}

/// Registry of all worker handles, for status bar display.
pub struct WorkerRegistry {
    handles: Vec<WorkerHandle>,
}

impl WorkerRegistry {
    pub fn new() -> Self {
        Self { handles: Vec::with_capacity(8) }
    }

    /// Register a new worker and return the watch sender for it.
    pub fn register(&mut self, name: &'static str) -> watch::Sender<WorkerState> {
        let handle = WorkerHandle::new(name);
        let tx = handle.state_sender();
        self.handles.push(handle);
        tx
    }

    /// Snapshot of all worker states, for rendering.
    pub fn snapshot(&self) -> Vec<(&'static str, WorkerState)> {
        self.handles.iter().map(|h| (h.name, h.state())).collect()
    }

    /// Count of workers in each state.
    pub fn health_summary(&self) -> (usize, usize, usize) {
        let mut running = 0;
        let mut backoff = 0;
        let mut stopped = 0;
        for h in &self.handles {
            match h.state() {
                WorkerState::Starting | WorkerState::Running => running += 1,
                WorkerState::Backoff { .. } => backoff += 1,
                WorkerState::Stopped => stopped += 1,
            }
        }
        (running, backoff, stopped)
    }

    pub fn len(&self) -> usize {
        self.handles.len()
    }

    pub fn is_empty(&self) -> bool {
        self.handles.is_empty()
    }
}

impl Default for WorkerRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handle_starts_in_starting_state() {
        let handle = WorkerHandle::new("test");
        assert_eq!(handle.state(), WorkerState::Starting);
        assert!(handle.is_healthy());
    }

    #[test]
    fn state_sender_updates_receiver() {
        let handle = WorkerHandle::new("test");
        let tx = handle.state_sender();
        let _ = tx.send(WorkerState::Running);
        assert_eq!(handle.state(), WorkerState::Running);
    }

    #[test]
    fn registry_tracks_multiple_workers() {
        let mut reg = WorkerRegistry::new();
        let tx1 = reg.register("health");
        let tx2 = reg.register("sse");
        assert_eq!(reg.len(), 2);

        let _ = tx1.send(WorkerState::Running);
        let _ = tx2.send(WorkerState::Backoff { attempt: 1 });

        let (running, backoff, stopped) = reg.health_summary();
        assert_eq!(running, 1);
        assert_eq!(backoff, 1);
        assert_eq!(stopped, 0);
    }

    #[test]
    fn worker_state_display() {
        assert_eq!(format!("{}", WorkerState::Running), "running");
        assert_eq!(format!("{}", WorkerState::Backoff { attempt: 3 }), "backoff #3");
    }

    #[test]
    fn snapshot_returns_all_states() {
        let mut reg = WorkerRegistry::new();
        let tx = reg.register("tick");
        let _ = tx.send(WorkerState::Running);
        let snap = reg.snapshot();
        assert_eq!(snap.len(), 1);
        assert_eq!(snap[0], ("tick", WorkerState::Running));
    }
}
