//! Funciones de texto grapheme-safe para TUI.
//!
//! Diseñadas para trabajar con terminal cell widths (no char counts), manejar
//! ANSI escapes, CJK (ancho 2), emoji (ancho 2) y combinaciones correctamente.

#![allow(unused_imports, reason = "re-exports E37 — toolkit pendiente de integrar")]
#![cfg_attr(not(test), allow(dead_code, reason = "E37 toolkit — integracion pendiente"))]

pub mod truncate;
pub mod visible_width;

pub use truncate::{truncate_path_middle, truncate_to_width};
pub use visible_width::{strip_ansi, visible_width};
