//! Selector<T> — versioned view into AppState slices.
//!
//! Selectors track a version counter to know if their backing data changed,
//! enabling the render loop to skip re-computation when state is stale-free.
//! Inspired by Redux selectors / claw-code AppStateStore.

use std::sync::atomic::{AtomicU64, Ordering};

/// Global monotonic counter for selector versioning.
static GLOBAL_VERSION: AtomicU64 = AtomicU64::new(0);

/// Bump the global version. Call whenever AppState mutates.
pub fn bump_version() -> u64 {
    GLOBAL_VERSION.fetch_add(1, Ordering::Relaxed) + 1
}

/// Read current global version without bumping.
pub fn current_version() -> u64 {
    GLOBAL_VERSION.load(Ordering::Relaxed)
}

/// A cached derived value that recomputes only when the source version changes.
pub struct Selector<T> {
    value: Option<T>,
    computed_at: u64,
}

impl<T> Selector<T> {
    pub fn new() -> Self {
        Self { value: None, computed_at: 0 }
    }

    /// Get the cached value, recomputing via `compute` if version changed.
    pub fn get_or_compute(&mut self, version: u64, compute: impl FnOnce() -> T) -> &T {
        if self.computed_at != version || self.value.is_none() {
            self.value = Some(compute());
            self.computed_at = version;
        }
        self.value.as_ref().expect("just computed")
    }

    /// Invalidate the cached value, forcing recompute on next access.
    pub fn invalidate(&mut self) {
        self.value = None;
        self.computed_at = 0;
    }

    /// Whether the selector has a cached value.
    pub fn is_cached(&self) -> bool {
        self.value.is_some()
    }
}

impl<T> Default for Selector<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selector_computes_on_first_access() {
        let mut sel = Selector::<i32>::new();
        let val = sel.get_or_compute(1, || 42);
        assert_eq!(*val, 42);
        assert!(sel.is_cached());
    }

    #[test]
    fn selector_reuses_cached_on_same_version() {
        let mut sel = Selector::<i32>::new();
        let mut call_count = 0;
        sel.get_or_compute(1, || {
            call_count += 1;
            42
        });
        sel.get_or_compute(1, || {
            call_count += 1;
            99
        });
        // Second call should reuse cached — compute called only once
        assert_eq!(call_count, 1);
        assert_eq!(*sel.get_or_compute(1, || 0), 42);
    }

    #[test]
    fn selector_recomputes_on_version_change() {
        let mut sel = Selector::<i32>::new();
        sel.get_or_compute(1, || 42);
        let val = sel.get_or_compute(2, || 99);
        assert_eq!(*val, 99);
    }

    #[test]
    fn invalidate_forces_recompute() {
        let mut sel = Selector::<i32>::new();
        sel.get_or_compute(1, || 42);
        sel.invalidate();
        assert!(!sel.is_cached());
        let val = sel.get_or_compute(1, || 99);
        assert_eq!(*val, 99);
    }

    #[test]
    fn bump_version_increments() {
        let v1 = bump_version();
        let v2 = bump_version();
        assert!(v2 > v1);
    }

    #[test]
    fn current_version_reads_without_bump() {
        let v1 = current_version();
        let v2 = current_version();
        assert_eq!(v1, v2);
    }
}
