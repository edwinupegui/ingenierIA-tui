/// MinDisplayTime — Prevents flickering of states that change too quickly.
///
/// Reference: claude-code useMinDisplayTime.ts.
/// Guarantees that a state is displayed for at least `min_duration`.
use std::time::{Duration, Instant};

const DEFAULT_MIN_DURATION: Duration = Duration::from_millis(500);

pub struct MinDisplayTime {
    current: String,
    shown_since: Instant,
    min_duration: Duration,
    pending: Option<String>,
}

impl MinDisplayTime {
    pub fn new(initial: String) -> Self {
        Self {
            current: initial,
            shown_since: Instant::now(),
            min_duration: DEFAULT_MIN_DURATION,
            pending: None,
        }
    }

    /// Update to new text. If min duration hasn't elapsed, queue it.
    pub fn update(&mut self, new_text: String) {
        if self.shown_since.elapsed() >= self.min_duration {
            self.current = new_text;
            self.shown_since = Instant::now();
            self.pending = None;
        } else {
            self.pending = Some(new_text);
        }
    }

    /// Call each tick to flush pending text once min duration elapses.
    pub fn tick(&mut self) -> &str {
        if let Some(pending) = self.pending.take() {
            if self.shown_since.elapsed() >= self.min_duration {
                self.current = pending;
                self.shown_since = Instant::now();
            } else {
                self.pending = Some(pending);
            }
        }
        &self.current
    }

    pub fn current(&self) -> &str {
        &self.current
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_text_available() {
        let mdt = MinDisplayTime::new("loading".into());
        assert_eq!(mdt.current(), "loading");
    }

    #[test]
    fn rapid_update_queues_pending() {
        let mut mdt = MinDisplayTime::new("a".into());
        // Immediately update — should queue since <500ms
        mdt.update("b".into());
        // pending is set but current is still "a" unless tick flushes
        assert_eq!(mdt.current(), "a");
    }
}
