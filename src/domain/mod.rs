//! Domain types — thin re-export from `ingenieria-domain` workspace crate (E28).
//!
//! All domain types now live in `crates/ingenieria-domain/`. This module
//! re-exports them so existing `crate::domain::*` paths keep working.

pub use ingenieria_domain::*;
