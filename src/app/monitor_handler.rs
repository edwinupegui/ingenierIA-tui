//! Slash commands para Process Monitor (E26).
//!
//! Comandos:
//!   /monitor <command>       — spawnea un proceso en background
//!   /monitor-list            — tabla de monitores activos/recientes
//!   /monitor-kill <id>       — solicita kill cooperativo
//!   /monitor-show <id>       — publica las ultimas lineas al chat
//!
//! El worker (`workers/process_monitor.rs`) emite `MonitorOutput` por linea
//! y `MonitorFinished` al terminar. Los handlers de estas Actions viven en
//! este modulo.

use crate::services::monitor::{
    MonitorInfo, MonitorLine, MonitorRegistry, MonitorStatus, MAX_CONCURRENT_MONITORS,
};
use crate::state::{ChatMessage, ChatRole, ToastLevel};
use crate::workers::process_monitor::spawn_monitor_task;

use super::App;

/// Limite de chars del comando — evita prompts pegados por accidente.
const MAX_COMMAND_LEN: usize = 1_000;
/// Lineas mostradas en el resumen final al completar un monitor.
const SUMMARY_TAIL_LINES: usize = 20;
/// Max monitores mostrados por `/monitor-list`.
const MONITOR_LIST_VIEW: usize = 10;

impl App {
    pub(crate) fn handle_monitor_start_command(&mut self, raw: &str) {
        let command = raw.trim();
        if command.is_empty() {
            self.notify("Uso: /monitor <command>".to_string());
            return;
        }
        if command.len() > MAX_COMMAND_LEN {
            self.notify(format!(
                "✗ Comando demasiado largo ({} chars). Limite: {MAX_COMMAND_LEN}",
                command.len()
            ));
            return;
        }
        if self.state.monitors.active_count() >= MAX_CONCURRENT_MONITORS {
            self.notify(format!(
                "✗ Pool de monitores lleno: {}/{MAX_CONCURRENT_MONITORS}. Usa /monitor-kill",
                self.state.monitors.active_count(),
            ));
            return;
        }

        let id = self.state.monitors.allocate_id();
        let info = MonitorInfo::new(id.clone(), command.to_string());
        let kill = info.kill.clone();
        self.state.monitors.insert(info);

        spawn_monitor_task(id.clone(), command.to_string(), kill, self.tx.clone());
        self.notify(format!("⇒ Monitor {id} lanzado: `{command}`"));
    }

    pub(crate) fn handle_monitor_list_command(&mut self) {
        let body = format_monitor_list(&self.state.monitors);
        self.push_monitor_markdown(body);
    }

    pub(crate) fn handle_monitor_kill_command(&mut self, arg: &str) {
        let id = arg.trim();
        if id.is_empty() {
            self.notify("Uso: /monitor-kill <id>".to_string());
            return;
        }
        if self.state.monitors.request_kill(id) {
            self.notify(format!("⊘ Kill solicitado para monitor {id}"));
        } else {
            self.notify(format!("✗ Monitor no encontrado o no activo: {id}"));
        }
    }

    pub(crate) fn handle_monitor_show_command(&mut self, arg: &str) {
        let id = arg.trim();
        if id.is_empty() {
            self.notify("Uso: /monitor-show <id>".to_string());
            return;
        }
        if self.state.monitors.get(id).is_none() {
            self.notify(format!("✗ Monitor no encontrado: {id}"));
            return;
        }
        // Open interactive panel overlay instead of dumping to chat.
        self.state.monitor_panel = Some(crate::state::MonitorPanelState::new(id.to_string()));
    }

    /// Handle key presses inside the monitor output panel overlay.
    pub(crate) fn on_char_monitor_panel(&mut self, c: char) {
        let Some(panel) = &mut self.state.monitor_panel else {
            return;
        };
        match c {
            'f' => {
                panel.follow = !panel.follow;
                if panel.follow {
                    panel.scroll_offset = 0;
                }
            }
            'k' => {
                let id = panel.monitor_id.clone();
                if self.state.monitors.request_kill(&id) {
                    self.notify(format!("⊘ Kill solicitado para monitor {id}"));
                }
            }
            'q' => {
                self.state.monitor_panel = None;
            }
            _ => {}
        }
    }

    pub(crate) fn handle_monitor_output(&mut self, id: String, line: String, is_stderr: bool) {
        // Sin notify — seria ruido. El usuario ve el output via /monitor-show
        // o cuando el proceso termina (resumen).
        self.state.monitors.push_line(&id, line, is_stderr);
    }

    pub(crate) fn handle_monitor_finished(
        &mut self,
        id: String,
        exit_code: Option<i32>,
        error: Option<String>,
        killed: bool,
    ) {
        let status = if killed {
            MonitorStatus::Killed
        } else if error.is_some() {
            MonitorStatus::Failed
        } else if exit_code == Some(0) {
            MonitorStatus::Done
        } else {
            MonitorStatus::Failed
        };
        self.state.monitors.finalize(&id, status.clone(), exit_code, error.clone());

        let Some(info) = self.state.monitors.get(&id).cloned() else {
            return;
        };
        let body = format_monitor_summary(&info);
        self.push_monitor_markdown(body);

        match status {
            MonitorStatus::Done => {
                self.notify(format!("✓ Monitor {id} completo"));
            }
            MonitorStatus::Failed => {
                let detail = info
                    .error
                    .clone()
                    .unwrap_or_else(|| format!("exit {}", exit_code.unwrap_or(-1)));
                self.notify_level(&format!("✗ Monitor {id} fallo: {detail}"), ToastLevel::Error);
            }
            MonitorStatus::Killed => {
                self.notify(format!("⊘ Monitor {id} terminado"));
            }
            MonitorStatus::Running => {}
        }
    }

    fn push_monitor_markdown(&mut self, body: String) {
        let cached =
            crate::ui::widgets::markdown::render_markdown(&body, &self.state.active_theme.colors());
        let mut msg = ChatMessage::new(ChatRole::Assistant, body);
        msg.cached_lines = Some(std::sync::Arc::new(cached));
        self.state.chat.messages.push(msg);
        self.state.chat.scroll_offset = u16::MAX;
    }
}

fn format_monitor_list(reg: &MonitorRegistry) -> String {
    let mut out = String::from("## Process Monitors\n\n");
    out.push_str(&format!("Activos: {}/{MAX_CONCURRENT_MONITORS}\n\n", reg.active_count()));
    if reg.monitors.is_empty() {
        out.push_str("No hay procesos monitoreados. Usa `/monitor <command>` para lanzar uno.\n");
        return out;
    }
    out.push_str("| ID | Estado | Exit | Duracion | Lineas | Comando |\n");
    out.push_str("|----|--------|------|----------|--------|---------|\n");
    for info in reg.recent(MONITOR_LIST_VIEW) {
        let dur =
            info.duration().map(|d| format!("{}s", d.as_secs())).unwrap_or_else(|| "-".to_string());
        let exit = info.exit_code.map(|c| c.to_string()).unwrap_or_else(|| "-".to_string());
        out.push_str(&format!(
            "| `{}` | {} | {} | {} | {} | `{}` |\n",
            info.id,
            info.status.label(),
            exit,
            dur,
            info.lines.len(),
            info.short_command(50),
        ));
    }
    out
}

fn format_monitor_summary(info: &MonitorInfo) -> String {
    let mut out = format!(
        "**[monitor {}]** · {} · exit {}\n\n",
        info.id,
        info.status.label(),
        info.exit_code.map(|c| c.to_string()).unwrap_or_else(|| "-".to_string())
    );
    out.push_str(&format!("> `{}`\n\n", info.command));
    if let Some(err) = &info.error {
        out.push_str(&format!("**Error**: {err}\n\n"));
    }
    append_tail_block(&mut out, info.tail(SUMMARY_TAIL_LINES), info.lines.len());
    out
}

fn append_tail_block(out: &mut String, lines: &[MonitorLine], total: usize) {
    if lines.is_empty() {
        out.push_str("_(sin output)_\n");
        return;
    }
    let shown = lines.len();
    if shown < total {
        out.push_str(&format!(
            "_{} lineas ocultas — mostrando las ultimas {shown}_\n\n",
            total - shown
        ));
    }
    out.push_str("```\n");
    for line in lines {
        if line.is_stderr {
            out.push_str("! ");
        }
        out.push_str(&line.text);
        out.push('\n');
    }
    out.push_str("```\n");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_list_empty_shows_hint() {
        let reg = MonitorRegistry::new();
        let out = format_monitor_list(&reg);
        assert!(out.contains("No hay procesos"));
    }

    #[test]
    fn format_list_renders_row() {
        let mut reg = MonitorRegistry::new();
        reg.insert(MonitorInfo::new("m1".into(), "cargo build".into()));
        let out = format_monitor_list(&reg);
        assert!(out.contains("| `m1` |"));
        assert!(out.contains("running"));
        assert!(out.contains("cargo build"));
    }

    #[test]
    fn format_summary_includes_exit_and_tail() {
        let mut info = MonitorInfo::new("m1".into(), "echo hi".into());
        info.lines.push(MonitorLine { text: "hi".into(), is_stderr: false });
        info.exit_code = Some(0);
        info.status = MonitorStatus::Done;
        let out = format_monitor_summary(&info);
        assert!(out.contains("monitor m1"));
        assert!(out.contains("exit 0"));
        assert!(out.contains("hi"));
    }

    #[test]
    fn format_summary_with_error_renders_error_block() {
        let mut info = MonitorInfo::new("m1".into(), "x".into());
        info.error = Some("spawn fallo: not found".into());
        info.status = MonitorStatus::Failed;
        let out = format_monitor_summary(&info);
        assert!(out.contains("**Error**"));
        assert!(out.contains("not found"));
    }

    #[test]
    fn tail_block_flags_stderr() {
        let mut out = String::new();
        let lines = vec![
            MonitorLine { text: "ok".into(), is_stderr: false },
            MonitorLine { text: "boom".into(), is_stderr: true },
        ];
        append_tail_block(&mut out, &lines, 2);
        assert!(out.contains("ok"));
        assert!(out.contains("! boom"));
    }

    #[test]
    fn tail_block_reports_hidden_lines() {
        let mut out = String::new();
        let lines = vec![MonitorLine { text: "last".into(), is_stderr: false }];
        append_tail_block(&mut out, &lines, 100);
        assert!(out.contains("99 lineas ocultas"));
    }
}
