//! Persistencia de cron jobs (E23).
//!
//! Formato JSON en `~/.config/ingenieria-tui/crons.json` con shape:
//!
//! ```json
//! {
//!   "jobs": [
//!     { "id": "c1", "expression": "0 0 9 * * Mon-Fri", "action": { "kind": "notify", "message": "Standup" }, "enabled": true, "fire_count": 0, "last_fired_at": null }
//!   ]
//! }
//! ```

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::job::CronJob;

#[derive(Debug, Serialize, Deserialize, Default)]
struct CronFile {
    #[serde(default)]
    jobs: Vec<CronJob>,
}

/// Path al archivo `crons.json` en el config dir XDG. Devuelve `None` si el
/// dir no esta disponible (ej. cuentas headless sin HOME).
pub fn crons_config_path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("ingenieria-tui").join("crons.json"))
}

/// Carga los cron jobs desde disco. Devuelve `Ok(vec![])` si el archivo no
/// existe; `Err` solo si esta corrupto.
pub fn load_jobs() -> anyhow::Result<Vec<CronJob>> {
    let Some(path) = crons_config_path() else {
        return Ok(Vec::new());
    };
    load_jobs_from(&path)
}

/// Variante que lee desde un path explicito — usada por tests para evitar
/// race conditions con el config dir global del sistema.
pub fn load_jobs_from(path: &std::path::Path) -> anyhow::Result<Vec<CronJob>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let raw = std::fs::read_to_string(path).map_err(anyhow::Error::from)?;
    let parsed: CronFile = serde_json::from_str(&raw).map_err(anyhow::Error::from)?;
    Ok(parsed.jobs)
}

/// Guarda atomicamente — escribe a `<path>.tmp` y rename. Crea el dir padre
/// si no existe. No falla si el config dir es inaccesible (silent skip,
/// devuelve Ok).
pub fn save_jobs(jobs: &[CronJob]) -> anyhow::Result<()> {
    let Some(path) = crons_config_path() else {
        return Ok(());
    };
    save_jobs_to(&path, jobs)
}

/// Variante que escribe a un path explicito.
pub fn save_jobs_to(path: &std::path::Path, jobs: &[CronJob]) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(anyhow::Error::from)?;
    }
    let file = CronFile { jobs: jobs.to_vec() };
    let serialized = serde_json::to_string_pretty(&file).map_err(anyhow::Error::from)?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serialized).map_err(anyhow::Error::from)?;
    std::fs::rename(&tmp, path).map_err(anyhow::Error::from)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::cron::job::CronAction;

    /// Tests usan paths explicitos en tempdirs propios — el config dir
    /// global no es respetado en macOS (dirs ignora XDG_CONFIG_HOME), asi
    /// que evitamos ese codepath para no depender de estado global.

    #[test]
    fn path_uses_config_dir() {
        // Smoke: solo verifica que la funcion devuelve algo plausible
        // cuando el sistema tiene HOME. En environments sin HOME (CI raro)
        // devuelve None, lo cual tambien es valido.
        if let Some(p) = crons_config_path() {
            assert!(p.ends_with("crons.json"));
        }
    }

    #[test]
    fn load_returns_empty_when_missing() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("crons.json");
        assert!(load_jobs_from(&path).expect("load ok").is_empty());
    }

    #[test]
    fn save_then_load_round_trip() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("nested").join("crons.json");
        let job = CronJob::new(
            "c1".into(),
            "0 0 12 * * *".into(),
            CronAction::Notify { message: "noon".into() },
        );
        save_jobs_to(&path, std::slice::from_ref(&job)).expect("save");
        let restored = load_jobs_from(&path).expect("load");
        assert_eq!(restored.len(), 1);
        assert_eq!(restored[0].id, job.id);
        assert_eq!(restored[0].expression, job.expression);
    }

    #[test]
    fn corrupt_file_returns_err() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("crons.json");
        std::fs::write(&path, "{not json").unwrap();
        assert!(load_jobs_from(&path).is_err());
    }
}
