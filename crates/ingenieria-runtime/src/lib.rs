//! Runtime services for ingenierIA TUI — session, audit, memory, permissions,
//! config validation.
//!
//! Pure service logic with no dependency on application state or UI.
//! Depends only on `ingenieria-domain` and standard serialization crates.

#![allow(dead_code)]

pub mod audit;
pub mod config_validation;
pub mod memory;
pub mod permissions;
pub mod session;

/// Shared mutex for tests that mutate the filesystem (env vars, temp dirs).
/// Prevents race conditions when tests run in parallel.
#[cfg(test)]
pub(crate) static TEST_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
