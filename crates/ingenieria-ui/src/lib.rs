//! UI primitives, theme, design system, and accessibility for ingenierIA TUI.
//!
//! This crate provides reusable visual components with no dependency on
//! application state or services. It depends only on `ingenieria-domain`
//! (for `UiFactory`) and `ratatui`.

// Many items are defined ahead of their consumers; suppress dead_code
// at the crate level since the public API is consumed by the main binary.
#![allow(dead_code)]

pub mod a11y;
pub mod buffer_diff;
pub mod design_system;
pub mod frame_throttle;
pub mod primitives;
pub mod style_pool;
pub mod theme;
