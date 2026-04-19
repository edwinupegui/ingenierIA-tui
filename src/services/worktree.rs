//! Worktree isolation para subagentes (E24).
//!
//! Cada subagente puede recibir su propio git worktree para aislar cambios y
//! evitar pisarse entre agentes paralelos. La creacion es opcional: si el
//! CWD no es un repo git, retorna `None` y el agente ejecuta sin worktree
//! (degradacion limpia).
//!
//! Ubicacion: `$XDG_DATA_HOME/ingenieria-tui/worktrees/<agent_id>/`.
//! Branch: `ingenieria/<agent_id>` para no colisionar con ramas del usuario.
//!
//! El manager guarda la lista de handles activos para que el shutdown de app
//! pueda limpiar via `cleanup_all()`. Cleanup por-agente se dispara al
//! finalize.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::SystemTime;

use anyhow::{anyhow, Result};

/// Prefijo de la branch creada por worktree — evita colisiones con branches
/// del usuario. Ejemplo: `ingenieria/a3`.
const BRANCH_PREFIX: &str = "ingenieria/";

/// Informacion de un worktree activo asociado a un subagente.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeHandle {
    pub agent_id: String,
    pub path: PathBuf,
    pub branch: String,
    pub created_at: SystemTime,
}

/// Manager de worktrees. Centraliza la lista de handles activos para que el
/// shutdown pueda limpiar todo de una vez.
#[derive(Debug, Default)]
pub struct WorktreeManager {
    active: Vec<WorktreeHandle>,
}

impl WorktreeManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Crea un worktree para `agent_id` en el directorio base. Retorna
    /// `Ok(None)` si estamos fuera de un repo git (caso legitimo, no error).
    ///
    /// El worktree se crea en un path unico por agente y con una branch
    /// `ingenieria/<agent_id>` creada desde HEAD.
    pub fn create(&mut self, agent_id: &str) -> Result<Option<WorktreeHandle>> {
        let cwd = std::env::current_dir()?;
        if !is_git_repo(&cwd) {
            return Ok(None);
        }
        let base =
            worktrees_base_dir().ok_or_else(|| anyhow!("no se pudo resolver XDG_DATA_HOME"))?;
        std::fs::create_dir_all(&base)?;
        let path = base.join(agent_id);
        // Si existe residuo de una corrida previa, lo limpiamos antes.
        if path.exists() {
            let _ = force_remove_worktree(&path); // best-effort: puede fallar si no es worktree git
            let _ = std::fs::remove_dir_all(&path); // best-effort: continúa aunque falle
        }
        let branch = format!("{BRANCH_PREFIX}{agent_id}");
        git_worktree_add(&cwd, &path, &branch)?;
        let handle = WorktreeHandle {
            agent_id: agent_id.to_string(),
            path: path.clone(),
            branch,
            created_at: SystemTime::now(),
        };
        self.active.push(handle.clone());
        Ok(Some(handle))
    }

    /// Limpia un worktree asociado al `agent_id`. Silencioso si no existe.
    pub fn cleanup(&mut self, agent_id: &str) -> Result<()> {
        let idx = match self.active.iter().position(|w| w.agent_id == agent_id) {
            Some(i) => i,
            None => return Ok(()),
        };
        let handle = self.active.remove(idx);
        if let Ok(cwd) = std::env::current_dir() {
            let _ = git_worktree_remove(&cwd, &handle.path); // best-effort: git puede ya haberlo removido
            let _ = git_branch_delete(&cwd, &handle.branch); // best-effort: rama puede no existir
        }
        if handle.path.exists() {
            let _ = std::fs::remove_dir_all(&handle.path); // best-effort: cleanup silencioso en shutdown
        }
        Ok(())
    }

    /// Limpia todos los worktrees activos. Idempotente. Usado en shutdown.
    pub fn cleanup_all(&mut self) {
        let ids: Vec<String> = self.active.iter().map(|w| w.agent_id.clone()).collect();
        for id in ids {
            let _ = self.cleanup(&id);
        }
    }

    #[allow(
        dead_code,
        reason = "expuesto para panel de worktrees del Sprint 12 + slash command /worktrees"
    )]
    pub fn list_active(&self) -> &[WorktreeHandle] {
        &self.active
    }

    pub fn active_count(&self) -> usize {
        self.active.len()
    }
}

/// Base directory para worktrees: `$XDG_DATA_HOME/ingenieria-tui/worktrees/` o
/// `$HOME/.local/share/ingenieria-tui/worktrees/` en su defecto.
pub fn worktrees_base_dir() -> Option<PathBuf> {
    let base = if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
        PathBuf::from(xdg)
    } else {
        let home = std::env::var("HOME").ok()?;
        PathBuf::from(home).join(".local").join("share")
    };
    Some(base.join("ingenieria-tui").join("worktrees"))
}

/// Detecta si `cwd` o cualquier ancestro contiene `.git`. No ejecuta git
/// para evitar el costo del subprocess en el hot path.
pub fn is_git_repo(cwd: &Path) -> bool {
    let mut current = cwd;
    loop {
        if current.join(".git").exists() {
            return true;
        }
        match current.parent() {
            Some(parent) if parent != current => current = parent,
            _ => return false,
        }
    }
}

fn git_worktree_add(cwd: &Path, path: &Path, branch: &str) -> Result<()> {
    let output = Command::new("git")
        .current_dir(cwd)
        .args(["worktree", "add", "-b", branch, &path.to_string_lossy(), "HEAD"])
        .output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("git worktree add fallo: {}", stderr.trim()));
    }
    Ok(())
}

fn git_worktree_remove(cwd: &Path, path: &Path) -> Result<()> {
    let output = Command::new("git")
        .current_dir(cwd)
        .args(["worktree", "remove", "--force", &path.to_string_lossy()])
        .output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("git worktree remove fallo: {}", stderr.trim()));
    }
    Ok(())
}

fn force_remove_worktree(path: &Path) -> Result<()> {
    if let Ok(cwd) = std::env::current_dir() {
        let _ = Command::new("git")
            .current_dir(&cwd)
            .args(["worktree", "remove", "--force", &path.to_string_lossy()])
            .output();
    }
    Ok(())
}

fn git_branch_delete(cwd: &Path, branch: &str) -> Result<()> {
    let output = Command::new("git").current_dir(cwd).args(["branch", "-D", branch]).output()?;
    if !output.status.success() {
        // Best-effort: algunos worktrees ya borraron la branch implicitamente.
        tracing::debug!(
            branch,
            stderr = %String::from_utf8_lossy(&output.stderr),
            "git branch -D no exitoso, ignorado"
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Lock para tests que mutan env vars (XDG_DATA_HOME) — evita razas.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn init_repo(dir: &Path) {
        let _ = Command::new("git").current_dir(dir).args(["init", "-q"]).output();
        let _ = Command::new("git")
            .current_dir(dir)
            .args(["config", "user.email", "test@ingenieria"])
            .output();
        let _ = Command::new("git").current_dir(dir).args(["config", "user.name", "Test"]).output();
        std::fs::write(dir.join("README.md"), "test").unwrap();
        let _ = Command::new("git").current_dir(dir).args(["add", "-A"]).output();
        let _ = Command::new("git").current_dir(dir).args(["commit", "-m", "init", "-q"]).output();
    }

    #[test]
    fn is_git_repo_false_for_temp_dir() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(!is_git_repo(tmp.path()));
    }

    #[test]
    fn is_git_repo_true_after_init() {
        let tmp = tempfile::tempdir().unwrap();
        init_repo(tmp.path());
        assert!(is_git_repo(tmp.path()));
    }

    #[test]
    fn worktrees_base_dir_respects_xdg() {
        let _guard = ENV_LOCK.lock().unwrap();
        // SAFETY: tests en un unico hilo via ENV_LOCK.
        unsafe {
            std::env::set_var("XDG_DATA_HOME", "/tmp/fake-xdg");
        }
        let base = worktrees_base_dir().unwrap();
        assert_eq!(base, PathBuf::from("/tmp/fake-xdg/ingenieria-tui/worktrees"));
        unsafe {
            std::env::remove_var("XDG_DATA_HOME");
        }
    }

    #[test]
    fn create_returns_none_outside_repo() {
        let _guard = ENV_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let original = std::env::current_dir().unwrap();
        std::env::set_current_dir(&tmp).unwrap();
        let mut mgr = WorktreeManager::new();
        let result = mgr.create("a1").unwrap();
        std::env::set_current_dir(original).unwrap();
        assert!(result.is_none());
        assert_eq!(mgr.active_count(), 0);
    }

    #[test]
    fn create_and_cleanup_roundtrip_in_real_repo() {
        let _guard = ENV_LOCK.lock().unwrap();
        let tmp_repo = tempfile::tempdir().unwrap();
        let tmp_data = tempfile::tempdir().unwrap();
        init_repo(tmp_repo.path());

        let original = std::env::current_dir().unwrap();
        std::env::set_current_dir(tmp_repo.path()).unwrap();
        // SAFETY: serializado via ENV_LOCK.
        unsafe {
            std::env::set_var("XDG_DATA_HOME", tmp_data.path());
        }

        let mut mgr = WorktreeManager::new();
        let handle = mgr.create("t1").unwrap().expect("debio crear worktree");
        assert!(handle.path.exists());
        assert_eq!(handle.branch, "ingenieria/t1");
        assert_eq!(mgr.active_count(), 1);

        mgr.cleanup("t1").unwrap();
        assert_eq!(mgr.active_count(), 0);

        unsafe {
            std::env::remove_var("XDG_DATA_HOME");
        }
        std::env::set_current_dir(original).unwrap();
    }

    #[test]
    fn cleanup_is_idempotent_for_unknown_id() {
        let mut mgr = WorktreeManager::new();
        assert!(mgr.cleanup("unknown").is_ok());
    }
}
