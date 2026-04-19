//! Reducer helpers para eventos del sistema de hooks configurable (E16).

use crate::services::hooks::HookOutcome;

use super::App;

impl App {
    /// Convierte `HookOutcome` en una notificacion UI. Exitos silenciosos
    /// (exit=0 sin stderr) no producen toast para evitar ruido; fallos y
    /// timeouts se muestran con nivel apropiado.
    pub(crate) fn handle_hook_executed(&mut self, outcome: HookOutcome) {
        let trigger = outcome.trigger.label();
        if outcome.is_success() {
            if !outcome.stderr_tail.is_empty() {
                self.notify_level(
                    &format!("◇ hook {}/{} OK ({}ms)", trigger, outcome.name, outcome.duration_ms),
                    crate::state::ToastLevel::Info,
                );
            }
            return;
        }
        let level = if outcome.exit_code == -2 {
            crate::state::ToastLevel::Warning
        } else {
            crate::state::ToastLevel::Error
        };
        let detail = if outcome.stderr_tail.is_empty() {
            format!("exit={}", outcome.exit_code)
        } else {
            let first_line: String = outcome.stderr_tail.lines().take(1).collect();
            let preview = if first_line.chars().count() > 60 {
                let truncated: String = first_line.chars().take(59).collect();
                format!("{truncated}…")
            } else {
                first_line
            };
            format!("exit={} {}", outcome.exit_code, preview)
        };
        self.notify_level(&format!("⚠ hook {}/{}: {detail}", trigger, outcome.name), level);
    }
}
