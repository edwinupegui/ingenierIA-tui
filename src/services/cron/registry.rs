//! Registry en memoria de cron jobs (E23).
//!
//! Vive en `AppState.crons` y es la fuente de verdad durante la ejecucion.
//! `crate::workers::cron_worker` lee snapshots periodicamente; los slash
//! commands mutan la lista y persisten via `store::save_jobs`.

use std::sync::{Arc, RwLock};

use super::job::CronJob;

#[derive(Debug, Default, Clone)]
pub struct CronRegistry {
    inner: Arc<RwLock<Vec<CronJob>>>,
    next_id: Arc<std::sync::atomic::AtomicUsize>,
}

impl CronRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Construye el registry desde una lista pre-cargada (typicamente desde
    /// disco al inicializar la app). Asegura que `next_id` quede coherente.
    pub fn with_jobs(jobs: Vec<CronJob>) -> Self {
        let max_seen = jobs
            .iter()
            .filter_map(|j| j.id.strip_prefix('c'))
            .filter_map(|n| n.parse::<usize>().ok())
            .max()
            .unwrap_or(0);
        Self {
            inner: Arc::new(RwLock::new(jobs)),
            next_id: Arc::new(std::sync::atomic::AtomicUsize::new(max_seen)),
        }
    }

    pub fn allocate_id(&self) -> String {
        let n = self.next_id.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
        format!("c{n}")
    }

    /// Snapshot de la lista actual — usar para iterar sin mantener el lock.
    pub fn snapshot(&self) -> Vec<CronJob> {
        self.inner.read().unwrap_or_else(|p| p.into_inner()).clone()
    }

    pub fn add(&self, job: CronJob) {
        let mut guard = self.inner.write().unwrap_or_else(|p| p.into_inner());
        guard.push(job);
    }

    pub fn remove(&self, id: &str) -> bool {
        let mut guard = self.inner.write().unwrap_or_else(|p| p.into_inner());
        let len_before = guard.len();
        guard.retain(|j| j.id != id);
        guard.len() != len_before
    }

    pub fn get_clone(&self, id: &str) -> Option<CronJob> {
        let guard = self.inner.read().unwrap_or_else(|p| p.into_inner());
        guard.iter().find(|j| j.id == id).cloned()
    }

    pub fn record_fired(&self, id: &str, now: chrono::DateTime<chrono::Utc>) {
        let mut guard = self.inner.write().unwrap_or_else(|p| p.into_inner());
        if let Some(job) = guard.iter_mut().find(|j| j.id == id) {
            job.record_fired(now);
        }
    }

    #[allow(dead_code, reason = "API publica del registry; consumido por tests + futuros widgets")]
    pub fn len(&self) -> usize {
        self.inner.read().map(|g| g.len()).unwrap_or(0)
    }

    #[allow(dead_code, reason = "API publica del registry; consumido por tests + futuros widgets")]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::cron::job::CronAction;

    fn sample(reg: &CronRegistry) -> CronJob {
        let id = reg.allocate_id();
        CronJob::new(id, "0 * * * * *".into(), CronAction::Notify { message: "ping".into() })
    }

    #[test]
    fn allocate_id_starts_at_c1() {
        let reg = CronRegistry::new();
        assert_eq!(reg.allocate_id(), "c1");
        assert_eq!(reg.allocate_id(), "c2");
    }

    #[test]
    fn with_jobs_continues_max_seen_id() {
        let jobs = vec![
            CronJob::new(
                "c5".into(),
                "0 * * * * *".into(),
                CronAction::Notify { message: "x".into() },
            ),
            CronJob::new(
                "c2".into(),
                "0 * * * * *".into(),
                CronAction::Notify { message: "y".into() },
            ),
        ];
        let reg = CronRegistry::with_jobs(jobs);
        assert_eq!(reg.allocate_id(), "c6");
    }

    #[test]
    fn add_and_snapshot() {
        let reg = CronRegistry::new();
        let job = sample(&reg);
        let id = job.id.clone();
        reg.add(job);
        let snap = reg.snapshot();
        assert_eq!(snap.len(), 1);
        assert_eq!(snap[0].id, id);
    }

    #[test]
    fn remove_returns_true_when_found() {
        let reg = CronRegistry::new();
        let job = sample(&reg);
        let id = job.id.clone();
        reg.add(job);
        assert!(reg.remove(&id));
        assert!(reg.is_empty());
    }

    #[test]
    fn remove_returns_false_when_missing() {
        let reg = CronRegistry::new();
        assert!(!reg.remove("nope"));
    }

    #[test]
    fn record_fired_updates_in_place() {
        let reg = CronRegistry::new();
        let job = sample(&reg);
        let id = job.id.clone();
        reg.add(job);
        reg.record_fired(&id, chrono::Utc::now());
        assert_eq!(reg.get_clone(&id).unwrap().fire_count, 1);
    }
}
