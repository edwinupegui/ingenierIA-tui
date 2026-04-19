//! Logica de evaluacion de cron expressions (E23).
//!
//! Wrapping del crate `cron` para parsear expresiones extendidas (con
//! segundos) y determinar si un job debe disparar entre dos timestamps.

use std::str::FromStr;

use chrono::{DateTime, Utc};
use cron::Schedule;

use super::job::CronJob;

/// Parsea la expresion y devuelve el `Schedule` o un error.
pub fn parse_expression(expr: &str) -> anyhow::Result<Schedule> {
    Schedule::from_str(expr.trim()).map_err(anyhow::Error::from)
}

/// Resumen humano de las proximas N ejecuciones (max 3).
pub fn schedule_summary(expr: &str) -> String {
    match parse_expression(expr) {
        Err(e) => format!("expr invalida: {e}"),
        Ok(sched) => {
            let next: Vec<String> = sched
                .upcoming(Utc)
                .take(3)
                .map(|t| t.format("%Y-%m-%d %H:%M:%S UTC").to_string())
                .collect();
            if next.is_empty() {
                "sin proximas ejecuciones".to_string()
            } else {
                next.join(", ")
            }
        }
    }
}

/// Devuelve los IDs de los jobs que deben dispararse en `now`.
///
/// Un job se considera due si:
///   - esta `enabled`,
///   - su expresion es valida,
///   - existe alguna ocurrencia entre `last_seen` y `now` (exclusivo a la
///     izquierda, inclusivo a la derecha),
///   - y esa ocurrencia no coincide exactamente con `last_fired_at` para
///     evitar disparos duplicados.
///
/// `last_seen` permite que el worker no pierda disparos entre ticks: pasa
/// el `Utc::now()` del tick previo.
pub fn due_jobs(jobs: &[CronJob], last_seen: DateTime<Utc>, now: DateTime<Utc>) -> Vec<String> {
    let mut due = Vec::new();
    for job in jobs {
        if !job.enabled {
            continue;
        }
        let Ok(sched) = parse_expression(&job.expression) else {
            continue;
        };
        if let Some(last_fire_match) = next_after(&sched, last_seen, now) {
            if Some(last_fire_match) != job.last_fired_at {
                due.push(job.id.clone());
            }
        }
    }
    due
}

/// Devuelve la primera ocurrencia del schedule en `(after, until]`, si existe.
fn next_after(
    sched: &Schedule,
    after: DateTime<Utc>,
    until: DateTime<Utc>,
) -> Option<DateTime<Utc>> {
    sched.after(&after).take_while(|t| *t <= until).next()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::cron::job::CronAction;
    use chrono::TimeZone;

    fn at(year: i32, month: u32, day: u32, hour: u32, min: u32, sec: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(year, month, day, hour, min, sec).single().expect("valid date")
    }

    #[test]
    fn parse_valid_six_field_expression() {
        assert!(parse_expression("0 0 * * * *").is_ok());
    }

    #[test]
    fn parse_invalid_expression_returns_error() {
        assert!(parse_expression("garbage").is_err());
    }

    #[test]
    fn schedule_summary_invalid_returns_error_text() {
        assert!(schedule_summary("not a cron").contains("invalida"));
    }

    #[test]
    fn schedule_summary_lists_upcoming() {
        // Cada minuto exacto.
        let summary = schedule_summary("0 * * * * *");
        assert!(summary.contains("UTC"));
    }

    #[test]
    fn due_jobs_fires_when_window_contains_match() {
        // every minute at second :00
        let job = CronJob::new(
            "c1".into(),
            "0 * * * * *".into(),
            CronAction::Notify { message: "hi".into() },
        );
        let last_seen = at(2025, 1, 1, 12, 30, 30);
        let now = at(2025, 1, 1, 12, 31, 5);
        let due = due_jobs(&[job], last_seen, now);
        assert_eq!(due, vec!["c1".to_string()]);
    }

    #[test]
    fn due_jobs_skips_when_no_match_in_window() {
        let job = CronJob::new(
            "c1".into(),
            "0 0 12 * * *".into(),
            CronAction::Notify { message: "hi".into() },
        );
        let last_seen = at(2025, 1, 1, 12, 0, 5);
        let now = at(2025, 1, 1, 12, 0, 35);
        // ya disparo ese minuto, segundo :00 esta antes de last_seen
        assert!(due_jobs(&[job], last_seen, now).is_empty());
    }

    #[test]
    fn due_jobs_respects_disabled_flag() {
        let mut job = CronJob::new(
            "c1".into(),
            "0 * * * * *".into(),
            CronAction::Notify { message: "hi".into() },
        );
        job.enabled = false;
        let last_seen = at(2025, 1, 1, 12, 30, 30);
        let now = at(2025, 1, 1, 12, 31, 5);
        assert!(due_jobs(&[job], last_seen, now).is_empty());
    }

    #[test]
    fn due_jobs_skips_when_last_fired_matches_window_occurrence() {
        let mut job = CronJob::new(
            "c1".into(),
            "0 * * * * *".into(),
            CronAction::Notify { message: "hi".into() },
        );
        let last_seen = at(2025, 1, 1, 12, 30, 30);
        let now = at(2025, 1, 1, 12, 31, 5);
        // Marcamos como ya disparado en :31:00 — no debe re-disparar.
        job.last_fired_at = Some(at(2025, 1, 1, 12, 31, 0));
        assert!(due_jobs(&[job], last_seen, now).is_empty());
    }
}
