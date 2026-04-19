//! Cron scheduler (E23).
//!
//! Sistema de tareas recurrentes configuradas por el usuario via slash
//! commands (`/cron-add`, `/cron-list`, `/cron-remove`).
//!
//! Diseno:
//! - `CronJob` describe el trabajo (id, expresion cron 6+1 fields, accion).
//! - `CronStore` persiste a `~/.config/ingenieria-tui/crons.json`.
//! - `CronRegistry` mantiene en memoria la lista cargada y trackea el ultimo
//!   `last_fired_at` por job para evitar re-disparos en el mismo segundo.
//! - `workers/cron_worker.rs` evalua cada `CRON_TICK_SECS` segundos.
//!
//! La expresion cron usa el formato extendido del crate `cron` (segundos
//! incluidos): `"sec min hour day month weekday [year]"` — 6 o 7 campos.

pub mod job;
pub mod registry;
pub mod scheduler;
pub mod store;

pub use job::{CronAction, CronJob};
pub use registry::CronRegistry;
pub use scheduler::{due_jobs, parse_expression, schedule_summary};
pub use store::{crons_config_path, load_jobs, save_jobs};

/// Frecuencia con la que el worker evalua los crons. 30s evita disparos
/// duplicados gracias a `last_fired_at` y deja margen para schedules cada
/// minuto sin desbordar el canal de actions.
pub const CRON_TICK_SECS: u64 = 30;
