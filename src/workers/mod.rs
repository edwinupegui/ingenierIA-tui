pub mod cron_worker;
pub mod file_watcher;
pub mod health;
pub mod hook_events;
pub mod keyboard;
#[expect(dead_code, reason = "E08 spec — consumed when workers adopt WorkerHandle lifecycle")]
pub mod lifecycle;
pub mod process_monitor;
pub mod sse;
pub mod tick;
pub mod tool_events;

/// Max backoff for SSE reconnection (shared by all event workers).
pub(crate) const MAX_BACKOFF_SECS: u64 = 30;
