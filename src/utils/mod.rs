//! Utilidades reusables independientes de la UI y el dominio.
//!
//! Los submodulos aqui deben ser funciones puras (sin estado global, sin I/O)
//! y sin dependencias en `state/`, `services/` o `ui/`.

#![cfg_attr(not(test), allow(dead_code, reason = "E37 toolkit — integracion pendiente"))]

pub mod text;
