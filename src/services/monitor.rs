//! Process Monitor (E26).
//!
//! Spawnea procesos de shell en background y captura stdout/stderr linea
//! por linea. Se usa para builds largos (`cargo build`, `pnpm test`, etc.)
//! sin bloquear el chat principal. El resultado final se publica como
//! mensaje del asistente con el codigo de salida y las ultimas N lineas.
//!
//! Arquitectura (Action-Reducer):
//! - `MonitorRegistry` vive en `AppState.monitors` (fuente unica de verdad).
//! - El worker (`workers/process_monitor.rs`) emite `Action::MonitorOutput`
//!   por cada linea y `Action::MonitorFinished` al terminar.
//! - El handler en `app/monitor_handler.rs` mutea el registry y publica
//!   toasts / mensajes al chat.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

/// Tope de procesos simultaneos. 3 alcanza para builds paralelos
/// sin saturar el pool de tokio con I/O pipes.
pub const MAX_CONCURRENT_MONITORS: usize = 3;

/// Tope de lineas almacenadas por monitor — evita crecimiento ilimitado en
/// builds verbose. Las primeras lineas se descartan en orden FIFO.
pub const MAX_MONITOR_LINES: usize = 500;

/// Tope de monitores mantenidos en historia (activos + terminados) para
/// sesiones largas.
const MAX_MONITOR_HISTORY: usize = 15;

#[derive(Debug, Clone, PartialEq)]
pub enum MonitorStatus {
    /// Proceso corriendo.
    Running,
    /// Proceso termino con exit code 0.
    Done,
    /// Exit code != 0 o error al spawn.
    Failed,
    /// Usuario invoco `/monitor-kill`.
    Killed,
}

impl MonitorStatus {
    pub fn label(&self) -> &'static str {
        match self {
            MonitorStatus::Running => "running",
            MonitorStatus::Done => "done",
            MonitorStatus::Failed => "failed",
            MonitorStatus::Killed => "killed",
        }
    }

    pub fn is_terminal(&self) -> bool {
        !matches!(self, MonitorStatus::Running)
    }
}

/// Linea de output capturada por el monitor.
#[derive(Debug, Clone)]
pub struct MonitorLine {
    pub text: String,
    pub is_stderr: bool,
}

#[derive(Debug, Clone)]
pub struct MonitorInfo {
    pub id: String,
    pub command: String,
    pub status: MonitorStatus,
    pub started_at: SystemTime,
    pub completed_at: Option<SystemTime>,
    pub exit_code: Option<i32>,
    pub error: Option<String>,
    pub lines: Vec<MonitorLine>,
    /// Flag cooperativo — el worker lo chequea y manda SIGTERM al child.
    pub kill: Arc<AtomicBool>,
}

impl MonitorInfo {
    pub fn new(id: String, command: String) -> Self {
        Self {
            id,
            command,
            status: MonitorStatus::Running,
            started_at: SystemTime::now(),
            completed_at: None,
            exit_code: None,
            error: None,
            lines: Vec::new(),
            kill: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn duration(&self) -> Option<Duration> {
        let end = self.completed_at.unwrap_or_else(SystemTime::now);
        end.duration_since(self.started_at).ok()
    }

    /// Retorna las ultimas `n` lineas para publicar resumen.
    pub fn tail(&self, n: usize) -> &[MonitorLine] {
        let start = self.lines.len().saturating_sub(n);
        &self.lines[start..]
    }

    pub fn short_command(&self, width: usize) -> String {
        let trimmed = self.command.trim();
        if trimmed.chars().count() <= width {
            trimmed.to_string()
        } else {
            let truncated: String = trimmed.chars().take(width.saturating_sub(1)).collect();
            format!("{truncated}…")
        }
    }
}

#[derive(Debug, Default)]
pub struct MonitorRegistry {
    pub monitors: Vec<MonitorInfo>,
    next_id: usize,
}

impl MonitorRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn allocate_id(&mut self) -> String {
        self.next_id += 1;
        format!("m{}", self.next_id)
    }

    pub fn insert(&mut self, info: MonitorInfo) {
        self.monitors.push(info);
        if self.monitors.len() > MAX_MONITOR_HISTORY {
            if let Some(idx) = self.monitors.iter().position(|m| m.status.is_terminal()) {
                self.monitors.remove(idx);
            }
        }
    }

    pub fn get(&self, id: &str) -> Option<&MonitorInfo> {
        self.monitors.iter().find(|m| m.id == id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut MonitorInfo> {
        self.monitors.iter_mut().find(|m| m.id == id)
    }

    pub fn active_count(&self) -> usize {
        self.monitors.iter().filter(|m| m.status == MonitorStatus::Running).count()
    }

    pub fn recent(&self, n: usize) -> impl Iterator<Item = &MonitorInfo> {
        let len = self.monitors.len();
        let start = len.saturating_sub(n);
        self.monitors[start..].iter().rev()
    }

    /// Append a una linea al buffer, respetando `MAX_MONITOR_LINES` (FIFO drop).
    pub fn push_line(&mut self, id: &str, line: String, is_stderr: bool) -> bool {
        let Some(info) = self.get_mut(id) else { return false };
        info.lines.push(MonitorLine { text: line, is_stderr });
        if info.lines.len() > MAX_MONITOR_LINES {
            info.lines.remove(0);
        }
        true
    }

    pub fn finalize(
        &mut self,
        id: &str,
        status: MonitorStatus,
        exit_code: Option<i32>,
        error: Option<String>,
    ) {
        if let Some(info) = self.get_mut(id) {
            info.status = status;
            info.completed_at = Some(SystemTime::now());
            info.exit_code = exit_code;
            info.error = error;
        }
    }

    /// Marca el kill flag del monitor. Devuelve `true` si lo encontro y aun
    /// estaba running.
    pub fn request_kill(&mut self, id: &str) -> bool {
        let Some(info) = self.get_mut(id) else { return false };
        if info.status != MonitorStatus::Running {
            return false;
        }
        info.kill.store(true, Ordering::Relaxed);
        true
    }

    /// Usado en shutdown: manda kill a todos los activos.
    pub fn kill_all(&mut self) {
        for info in &mut self.monitors {
            if info.status == MonitorStatus::Running {
                info.kill.store(true, Ordering::Relaxed);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allocate_id_increments() {
        let mut r = MonitorRegistry::new();
        assert_eq!(r.allocate_id(), "m1");
        assert_eq!(r.allocate_id(), "m2");
    }

    #[test]
    fn insert_and_lookup() {
        let mut r = MonitorRegistry::new();
        let info = MonitorInfo::new("m1".into(), "echo hi".into());
        r.insert(info);
        assert!(r.get("m1").is_some());
        assert_eq!(r.active_count(), 1);
    }

    #[test]
    fn push_line_respects_cap() {
        let mut r = MonitorRegistry::new();
        r.insert(MonitorInfo::new("m1".into(), "echo".into()));
        for i in 0..(MAX_MONITOR_LINES + 10) {
            r.push_line("m1", format!("line {i}"), false);
        }
        assert_eq!(r.get("m1").unwrap().lines.len(), MAX_MONITOR_LINES);
        // The oldest line should have been dropped.
        assert_eq!(r.get("m1").unwrap().lines[0].text, format!("line {}", 10));
    }

    #[test]
    fn finalize_sets_status_and_exit() {
        let mut r = MonitorRegistry::new();
        r.insert(MonitorInfo::new("m1".into(), "echo".into()));
        r.finalize("m1", MonitorStatus::Done, Some(0), None);
        let info = r.get("m1").unwrap();
        assert_eq!(info.status, MonitorStatus::Done);
        assert_eq!(info.exit_code, Some(0));
        assert!(info.completed_at.is_some());
    }

    #[test]
    fn request_kill_flags_running_only() {
        let mut r = MonitorRegistry::new();
        r.insert(MonitorInfo::new("m1".into(), "echo".into()));
        assert!(r.request_kill("m1"));
        assert!(r.get("m1").unwrap().kill.load(Ordering::Relaxed));
        r.finalize("m1", MonitorStatus::Done, Some(0), None);
        assert!(!r.request_kill("m1"));
    }

    #[test]
    fn tail_returns_last_n() {
        let mut r = MonitorRegistry::new();
        r.insert(MonitorInfo::new("m1".into(), "echo".into()));
        for i in 0..5 {
            r.push_line("m1", format!("line {i}"), false);
        }
        let tail = r.get("m1").unwrap().tail(3);
        assert_eq!(tail.len(), 3);
        assert_eq!(tail[0].text, "line 2");
        assert_eq!(tail[2].text, "line 4");
    }

    #[test]
    fn active_count_ignores_terminal() {
        let mut r = MonitorRegistry::new();
        r.insert(MonitorInfo::new("m1".into(), "a".into()));
        r.insert(MonitorInfo::new("m2".into(), "b".into()));
        r.finalize("m1", MonitorStatus::Done, Some(0), None);
        assert_eq!(r.active_count(), 1);
    }

    #[test]
    fn kill_all_flags_only_running() {
        let mut r = MonitorRegistry::new();
        r.insert(MonitorInfo::new("m1".into(), "a".into()));
        r.insert(MonitorInfo::new("m2".into(), "b".into()));
        r.finalize("m2", MonitorStatus::Done, Some(0), None);
        r.kill_all();
        assert!(r.get("m1").unwrap().kill.load(Ordering::Relaxed));
        // m2 ya terminal — no tiene sentido setear, aunque el flag seteado
        // no causa dano; chequeamos solo m1 para no sobre-especificar.
    }

    #[test]
    fn short_command_truncates() {
        let info = MonitorInfo::new("m1".into(), "x".repeat(100));
        assert!(info.short_command(20).chars().count() <= 20);
    }

    #[test]
    fn status_is_terminal_classification() {
        assert!(!MonitorStatus::Running.is_terminal());
        assert!(MonitorStatus::Done.is_terminal());
        assert!(MonitorStatus::Failed.is_terminal());
        assert!(MonitorStatus::Killed.is_terminal());
    }
}
