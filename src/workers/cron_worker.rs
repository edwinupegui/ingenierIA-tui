//! Cron tick worker (E23).
//!
//! Tarea long-running que evalua el `CronRegistry` cada
//! [`CRON_TICK_SECS`](crate::services::cron::CRON_TICK_SECS) segundos y
//! emite `Action::CronJobFired { id }` por cada job due. El reducer en App
//! aplica el side-effect (notify/spawn) y persiste el estado actualizado.

use std::time::Duration;

use chrono::Utc;
use tokio::sync::mpsc::Sender;

use crate::actions::Action;
use crate::services::cron::{due_jobs, CronRegistry, CRON_TICK_SECS};

/// Lanza el cron worker. Devuelve inmediatamente; el task corre indefinido
/// hasta que `tx` se cierra.
pub fn spawn(registry: CronRegistry, tx: Sender<Action>) {
    tokio::spawn(async move {
        let mut last_seen = Utc::now();
        let mut interval = tokio::time::interval(Duration::from_secs(CRON_TICK_SECS));
        // Skip first tick (interval fires inmediato por defecto).
        interval.tick().await;
        loop {
            interval.tick().await;
            let now = Utc::now();
            let snap = registry.snapshot();
            for id in due_jobs(&snap, last_seen, now) {
                if tx.send(Action::CronJobFired { id }).await.is_err() {
                    return; // canal cerrado, app terminando
                }
            }
            last_seen = now;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::cron::job::{CronAction, CronJob};

    /// Smoke test: el worker no panic-ea cuando el registry esta vacio.
    /// No verifica timing — cubrir eso requiere mockear tokio::time, lo
    /// dejamos para tests de integracion futuros.
    #[tokio::test]
    async fn worker_spawns_with_empty_registry() {
        let reg = CronRegistry::new();
        let (tx, _rx) = tokio::sync::mpsc::channel(8);
        spawn(reg, tx);
        // Inmediatamente despues no debe haber panics.
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    #[test]
    fn registry_fires_due_job_via_due_jobs() {
        let reg = CronRegistry::new();
        let id = reg.allocate_id();
        reg.add(CronJob::new(
            id.clone(),
            "0 * * * * *".into(),
            CronAction::Notify { message: "hi".into() },
        ));
        let snap = reg.snapshot();
        let last_seen = Utc::now() - chrono::Duration::seconds(120);
        let now = Utc::now();
        let due = due_jobs(&snap, last_seen, now);
        // Al menos un disparo en los ultimos 2 minutos (cada minuto :00).
        assert!(due.contains(&id));
    }
}
