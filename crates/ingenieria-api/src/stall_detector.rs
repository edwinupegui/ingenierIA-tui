//! Detector de stall post-tool execution.
//!
//! Cuando el AI llama a un tool, ejecutamos la tool y enviamos el resultado.
//! Si despues de enviar el tool_result el API no emite eventos por N segundos,
//! la conexion probablemente esta colgada (caso reportado en claw-code con
//! `POST_TOOL_STALL_TIMEOUT=10s`). El detector genera un "nudge" — un
//! reintento automatico silencioso — y si el nudge tambien falla, escala el
//! error.

use std::time::{Duration, Instant};

/// Timeout por defecto despues de ejecutar un tool antes de considerar stall.
pub const DEFAULT_STALL_TIMEOUT: Duration = Duration::from_secs(10);
/// Numero maximo de nudges automaticos antes de rendirse.
pub const DEFAULT_MAX_NUDGES: u32 = 1;

/// Estado del detector.
#[derive(Debug, Clone, PartialEq)]
pub enum StallState {
    /// No hay tool en curso; el detector esta dormido.
    Idle,
    /// Acabamos de terminar un tool. Contando desde `since`.
    Watching { since: Instant },
}

/// Decision tomada cuando se comprueba el estado.
#[derive(Debug, Clone, PartialEq)]
pub enum StallAction {
    /// Todo bien, seguir esperando o el stream esta activo.
    Ok,
    /// Han pasado >= `timeout` sin eventos. Hacer un nudge (retry silencioso).
    Nudge { nudge_number: u32, elapsed: Duration },
    /// Ya agotamos los nudges. Reportar el fallo.
    GiveUp { elapsed: Duration },
}

/// Detector con estado.
pub struct PostToolStallDetector {
    state: StallState,
    timeout: Duration,
    max_nudges: u32,
    nudges_used: u32,
}

impl PostToolStallDetector {
    pub fn new() -> Self {
        Self::with_config(DEFAULT_STALL_TIMEOUT, DEFAULT_MAX_NUDGES)
    }

    pub fn with_config(timeout: Duration, max_nudges: u32) -> Self {
        Self { state: StallState::Idle, timeout, max_nudges, nudges_used: 0 }
    }

    /// Marca que acabamos de ejecutar un tool y empezamos a vigilar.
    pub fn on_tool_completed(&mut self) {
        self.state = StallState::Watching { since: Instant::now() };
    }

    /// Marca que llego un evento del stream; resetea el detector a Idle.
    pub fn on_stream_event(&mut self) {
        self.state = StallState::Idle;
        // Nota: nudges_used no se resetea aqui — se resetea al iniciar nueva request.
    }

    /// Resetea por completo (nueva request del usuario).
    #[cfg_attr(not(test), allow(dead_code, reason = "API publica para reutilizar el detector"))]
    pub fn reset(&mut self) {
        self.state = StallState::Idle;
        self.nudges_used = 0;
    }

    /// Consulta el estado actual. Llamar periodicamente desde el loop del stream.
    pub fn check(&mut self) -> StallAction {
        let StallState::Watching { since } = self.state else {
            return StallAction::Ok;
        };
        let elapsed = since.elapsed();
        if elapsed < self.timeout {
            return StallAction::Ok;
        }
        if self.nudges_used >= self.max_nudges {
            self.state = StallState::Idle;
            return StallAction::GiveUp { elapsed };
        }
        self.nudges_used += 1;
        self.state = StallState::Idle;
        StallAction::Nudge { nudge_number: self.nudges_used, elapsed }
    }

    /// `true` si el detector esta vigilando actualmente.
    #[allow(dead_code)]
    pub fn is_watching(&self) -> bool {
        matches!(self.state, StallState::Watching { .. })
    }
}

impl Default for PostToolStallDetector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;

    #[test]
    fn idle_returns_ok() {
        let mut d = PostToolStallDetector::new();
        assert_eq!(d.check(), StallAction::Ok);
    }

    #[test]
    fn watching_under_timeout_is_ok() {
        let mut d = PostToolStallDetector::with_config(Duration::from_secs(10), 1);
        d.on_tool_completed();
        assert_eq!(d.check(), StallAction::Ok);
        assert!(d.is_watching());
    }

    #[test]
    fn timeout_triggers_nudge() {
        let mut d = PostToolStallDetector::with_config(Duration::from_millis(30), 1);
        d.on_tool_completed();
        sleep(Duration::from_millis(50));
        match d.check() {
            StallAction::Nudge { nudge_number, .. } => assert_eq!(nudge_number, 1),
            other => panic!("expected Nudge, got {other:?}"),
        }
    }

    #[test]
    fn second_timeout_gives_up() {
        let mut d = PostToolStallDetector::with_config(Duration::from_millis(20), 1);
        d.on_tool_completed();
        sleep(Duration::from_millis(30));
        assert!(matches!(d.check(), StallAction::Nudge { .. }));
        // Simular que tras el nudge seguimos sin respuesta.
        d.on_tool_completed();
        sleep(Duration::from_millis(30));
        assert!(matches!(d.check(), StallAction::GiveUp { .. }));
    }

    #[test]
    fn stream_event_resets_to_idle() {
        let mut d = PostToolStallDetector::new();
        d.on_tool_completed();
        assert!(d.is_watching());
        d.on_stream_event();
        assert!(!d.is_watching());
    }

    #[test]
    fn reset_clears_nudges_used() {
        let mut d = PostToolStallDetector::with_config(Duration::from_millis(20), 1);
        d.on_tool_completed();
        sleep(Duration::from_millis(30));
        let _ = d.check(); // consume el nudge
        d.reset();
        d.on_tool_completed();
        sleep(Duration::from_millis(30));
        assert!(matches!(d.check(), StallAction::Nudge { .. }));
    }
}
