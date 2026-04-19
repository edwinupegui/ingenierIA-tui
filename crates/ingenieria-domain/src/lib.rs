//! Domain types for ingenierIA TUI.
//!
//! Pure data structures with no internal dependencies. This crate
//! is the foundation of the workspace — all other crates depend on it,
//! but it depends on nothing internal.

pub mod chat;
pub mod doctor;
pub mod document;
pub mod event;
pub mod factory;
pub mod failure;
pub mod health;
pub mod hook_event;
pub mod init_types;
pub mod permissions;
pub mod plugin;
pub mod recovery;
pub mod search;
pub mod time;
pub mod toast;
pub mod todos;
pub mod tool_event;
