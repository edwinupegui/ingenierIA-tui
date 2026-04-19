//! Streaming Markdown Engine — incremental rendering with safe-boundary splitting.
//!
//! Splits streaming content into stable (already-rendered) and volatile (may change)
//! portions. Only the volatile portion gets re-parsed, reducing CPU during streaming.

pub mod fence_normalizer;
pub mod stream_state;
pub mod thinking_collapse;
pub mod token_cache;
