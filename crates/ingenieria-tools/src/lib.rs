//! Tool validation, bash safety, hooks types, and MCP utilities for ingenierIA TUI.
//!
//! Contains the portable parts of the tools system. Modules that depend on
//! `Action` (hooks/runner, tools/config_tool, mcp/elicitation) stay in the
//! main binary.

#![allow(dead_code)]

pub mod bash;
pub mod hooks;
pub mod mcp;
pub mod retry;
