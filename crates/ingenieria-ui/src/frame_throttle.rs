//! Adaptive frame throttle — caps rendering to a configurable max FPS.
//!
//! Prevents flooding the terminal with redraws during high-frequency actions
//! (e.g. streaming deltas, rapid keypresses). Frames are skipped when the
//! interval since last draw is below the minimum frame time.

use std::time::{Duration, Instant};

const DEFAULT_MAX_FPS: u32 = 30;

pub struct FrameThrottle {
    min_frame_time: Duration,
    last_draw: Instant,
    frames_drawn: u64,
    frames_skipped: u64,
}

impl FrameThrottle {
    pub fn new(max_fps: u32) -> Self {
        let fps = max_fps.max(1);
        Self {
            min_frame_time: Duration::from_micros(1_000_000 / u64::from(fps)),
            last_draw: Instant::now() - Duration::from_secs(1), // allow first frame immediately
            frames_drawn: 0,
            frames_skipped: 0,
        }
    }

    /// Returns `true` if enough time has elapsed to draw a new frame.
    pub fn should_draw(&self) -> bool {
        self.last_draw.elapsed() >= self.min_frame_time
    }

    /// Mark a frame as drawn. Call after `terminal.draw()`.
    pub fn mark_drawn(&mut self) {
        self.last_draw = Instant::now();
        self.frames_drawn += 1;
    }

    /// Mark a frame as skipped (too soon after last draw).
    pub fn mark_skipped(&mut self) {
        self.frames_skipped += 1;
    }

    pub fn frames_drawn(&self) -> u64 {
        self.frames_drawn
    }

    pub fn frames_skipped(&self) -> u64 {
        self.frames_skipped
    }
}

impl Default for FrameThrottle {
    fn default() -> Self {
        Self::new(DEFAULT_MAX_FPS)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_frame_always_allowed() {
        let throttle = FrameThrottle::default();
        assert!(throttle.should_draw());
    }

    #[test]
    fn immediate_second_frame_blocked() {
        let mut throttle = FrameThrottle::default();
        throttle.mark_drawn();
        // Immediately after drawing, next frame should be blocked
        assert!(!throttle.should_draw());
    }

    #[test]
    fn counters_track_correctly() {
        let mut throttle = FrameThrottle::default();
        throttle.mark_drawn();
        throttle.mark_skipped();
        throttle.mark_skipped();
        assert_eq!(throttle.frames_drawn(), 1);
        assert_eq!(throttle.frames_skipped(), 2);
    }

    #[test]
    fn custom_fps() {
        let throttle = FrameThrottle::new(60);
        assert!(throttle.should_draw());
    }
}
