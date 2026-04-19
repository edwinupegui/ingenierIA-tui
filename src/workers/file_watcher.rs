//! File watcher (E42) — vigila archivos de configuracion y emite Actions.
//!
//! Archivos monitoreados (solo los que existen en disco al arrancar):
//! - `$XDG_CONFIG_HOME/ingenieria-tui/config.json` → `Action::ConfigChanged`
//! - `$XDG_CONFIG_HOME/ingenieria-tui/keybindings.json` → `Action::KeybindingsChanged`
//! - `$CWD/CLAUDE.md` → `Action::ClaudeMdChanged`
//! - `$CWD/.env` → `Action::EnvChanged`
//!
//! **Debounce** de `DEBOUNCE_MS=500` evita multiples reloads cuando los
//! editores hacen escrituras fragmentadas (tipico en Vim/VSCode).
//!
//! El worker usa `notify::recommended_watcher` que elige inotify/kqueue/
//! fsevents segun OS. Si no se puede inicializar el watcher (p.ej. sin
//! permisos), el worker emite `tracing::warn!` y termina — NO paniquea.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use notify::{Event, EventKind, RecursiveMode, Watcher};
use tokio::sync::mpsc::{Sender, UnboundedReceiver};

use crate::actions::Action;

/// Debounce: eventos mas cercanos que esto se colapsan.
const DEBOUNCE_MS: u64 = 500;

/// Clasifica un path concreto a la Action correspondiente. Retorna `None`
/// si el path no corresponde a ningun archivo monitoreado.
fn classify(path: &std::path::Path) -> Option<Action> {
    let name = path.file_name()?.to_str()?;
    match name {
        "config.json" => Some(Action::ConfigChanged),
        "keybindings.json" => Some(Action::KeybindingsChanged),
        "CLAUDE.md" => Some(Action::ClaudeMdChanged),
        ".env" => Some(Action::EnvChanged),
        _ => None,
    }
}

/// Resuelve los paths que tiene sentido vigilar (solo los que existen).
fn discover_watch_paths() -> Vec<PathBuf> {
    let mut paths = Vec::with_capacity(4);
    if let Some(config_dir) = dirs::config_dir().map(|d| d.join("ingenieria-tui")) {
        let cfg = config_dir.join("config.json");
        if cfg.exists() {
            paths.push(cfg);
        }
        let kb = config_dir.join("keybindings.json");
        if kb.exists() {
            paths.push(kb);
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        let claude = cwd.join("CLAUDE.md");
        if claude.exists() {
            paths.push(claude);
        }
        let env = cwd.join(".env");
        if env.exists() {
            paths.push(env);
        }
    }
    paths
}

/// Entry point del worker. Si el watcher no puede inicializarse, loguea y sale.
/// Caller: `tokio::spawn(workers::file_watcher::run(tx.clone()))`.
pub async fn run(tx: Sender<Action>) {
    let (notify_tx, notify_rx) = tokio::sync::mpsc::unbounded_channel::<Event>();
    let watcher = match build_watcher(notify_tx) {
        Some(w) => w,
        None => return,
    };
    let (mut watcher, paths) = (watcher, discover_watch_paths());
    if paths.is_empty() {
        tracing::info!("file_watcher: no hay archivos para vigilar; saliendo");
        drop(watcher);
        return;
    }
    for path in &paths {
        if let Err(e) = watcher.watch(path, RecursiveMode::NonRecursive) {
            tracing::warn!(path = %path.display(), error = %e, "file_watcher: watch failed");
        }
    }
    tracing::info!(count = paths.len(), "file_watcher: activo");

    // Retenemos el watcher vivo hasta que el loop termine.
    debounced_dispatch_loop(notify_rx, &tx).await;
    drop(watcher);
}

/// Construye el `recommended_watcher` que envia eventos al tokio channel.
/// Retorna `None` si falla la creacion.
fn build_watcher(
    notify_tx: tokio::sync::mpsc::UnboundedSender<Event>,
) -> Option<notify::RecommendedWatcher> {
    let result = notify::recommended_watcher(move |res: Result<Event, notify::Error>| match res {
        Ok(event) => {
            if notify_tx.send(event).is_err() {
                // Receiver cerrado — el worker esta terminando.
            }
        }
        Err(e) => tracing::warn!(error = %e, "file_watcher: error del notifier"),
    });
    match result {
        Ok(w) => Some(w),
        Err(e) => {
            tracing::warn!(error = %e, "file_watcher: no se pudo inicializar notify");
            None
        }
    }
}

/// Loop principal: recibe eventos del watcher, aplica debounce y dispatcha
/// Actions. Solo considera `EventKind::Modify | Create`.
async fn debounced_dispatch_loop(mut notify_rx: UnboundedReceiver<Event>, tx: &Sender<Action>) {
    let debounce = Duration::from_millis(DEBOUNCE_MS);
    let mut last_event: Option<Instant> = None;
    while let Some(event) = notify_rx.recv().await {
        if !is_mutation(&event.kind) {
            continue;
        }
        let now = Instant::now();
        if let Some(prev) = last_event {
            if now.duration_since(prev) < debounce {
                continue;
            }
        }
        last_event = Some(now);
        dispatch_event_paths(&event.paths, tx).await;
    }
}

/// True si el evento representa una mutacion (no rename/remove/access).
fn is_mutation(kind: &EventKind) -> bool {
    matches!(kind, EventKind::Modify(_) | EventKind::Create(_))
}

/// Clasifica cada path del evento y envia la Action. Best-effort: si el
/// channel esta cerrado, corta el loop.
async fn dispatch_event_paths(paths: &[PathBuf], tx: &Sender<Action>) {
    for path in paths {
        if let Some(action) = classify(path) {
            if tx.send(action).await.is_err() {
                // Main app channel cerrado, nada que hacer.
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_matches_known_filenames() {
        assert!(matches!(
            classify(std::path::Path::new("/foo/config.json")),
            Some(Action::ConfigChanged)
        ));
        assert!(matches!(
            classify(std::path::Path::new("/foo/keybindings.json")),
            Some(Action::KeybindingsChanged)
        ));
        assert!(matches!(
            classify(std::path::Path::new("/foo/CLAUDE.md")),
            Some(Action::ClaudeMdChanged)
        ));
        assert!(matches!(classify(std::path::Path::new("/foo/.env")), Some(Action::EnvChanged)));
    }

    #[test]
    fn classify_unknown_returns_none() {
        assert!(classify(std::path::Path::new("/foo/other.json")).is_none());
        assert!(classify(std::path::Path::new("/foo/")).is_none());
    }

    #[test]
    fn is_mutation_filters_non_modifications() {
        use notify::event::{AccessKind, ModifyKind};
        assert!(is_mutation(&EventKind::Modify(ModifyKind::Any)));
        assert!(is_mutation(&EventKind::Create(notify::event::CreateKind::Any)));
        assert!(!is_mutation(&EventKind::Access(AccessKind::Any)));
        assert!(!is_mutation(&EventKind::Remove(notify::event::RemoveKind::Any)));
    }
}
