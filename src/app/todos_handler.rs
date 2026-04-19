//! Planning & Tasks (E12) — handlers para slash commands y el ciclo de plan.

use crate::domain::todos::{parse_plan_tree, TodoStatus};
use crate::state::{ChatMessage, ChatRole};

use super::App;

impl App {
    /// `/todos` — renderiza la lista actual como mensaje de asistente.
    pub(crate) fn handle_todos_command(&mut self) {
        let body = render_todos_markdown(&self.state.chat.todos);
        self.push_assistant_markdown(body);
    }

    /// `/todo-add <titulo>` — crea un nuevo todo en estado pending.
    pub(crate) fn handle_todo_add_command(&mut self, arg: &str) {
        let title = arg.trim();
        if title.is_empty() {
            self.notify("Uso: /todo-add <titulo>".to_string());
            return;
        }
        if title.len() > 500 {
            self.notify("Titulo demasiado largo (max 500 chars)".to_string());
            return;
        }
        let id = self.state.chat.todos.add(title);
        self.notify(format!("✓ Todo #{id} agregado"));
    }

    /// `/todo-done <id>` — marca el todo como completado.
    /// Alias: `/todo-check`.
    pub(crate) fn handle_todo_done_command(&mut self, arg: &str) {
        let Some(id) = parse_todo_id(arg) else {
            self.notify("Uso: /todo-done <id>".to_string());
            return;
        };
        if !self.state.chat.todos.set_status(id, TodoStatus::Completed) {
            self.notify(format!("✗ Todo #{id} no encontrado"));
            return;
        }
        let summary = self.state.chat.todos.short_summary();
        self.notify(format!("✓ Todo #{id} → completado ({summary})"));
        self.maybe_emit_auto_verify_nudge();
    }

    /// `/todo-start <id>` — marca el todo como in_progress.
    pub(crate) fn handle_todo_start_command(&mut self, arg: &str) {
        let Some(id) = parse_todo_id(arg) else {
            self.notify("Uso: /todo-start <id>".to_string());
            return;
        };
        if !self.state.chat.todos.set_status(id, TodoStatus::InProgress) {
            self.notify(format!("✗ Todo #{id} no encontrado"));
            return;
        }
        self.notify(format!("◐ Todo #{id} → en curso"));
    }

    /// `/todo-remove <id>` — elimina un todo.
    pub(crate) fn handle_todo_remove_command(&mut self, arg: &str) {
        let Some(id) = parse_todo_id(arg) else {
            self.notify("Uso: /todo-remove <id>".to_string());
            return;
        };
        if self.state.chat.todos.remove(id) {
            self.notify(format!("✗ Todo #{id} eliminado"));
        } else {
            self.notify(format!("✗ Todo #{id} no encontrado"));
        }
    }

    /// `/todo-clear` — vacia la lista.
    pub(crate) fn handle_todo_clear_command(&mut self) {
        if self.state.chat.todos.is_empty() {
            self.notify("Lista de todos ya esta vacia".to_string());
            return;
        }
        self.state.chat.todos.clear();
        self.notify("✓ Todos eliminados".to_string());
    }

    /// Si la lista dispara el auto-verify nudge (E12: 3 completions consecutivas),
    /// inyecta un system message para que la AI revise el progreso.
    fn maybe_emit_auto_verify_nudge(&mut self) {
        if self.state.chat.todos.take_pending_nudge() {
            let nudge = format!(
                "AUTO-VERIFY: {}. Antes de seguir, revisa que los pasos completados \
                 sean consistentes con el plan y con las policies/ADRs de ingenierIA.",
                self.state.chat.todos.short_summary()
            );
            self.state.chat.messages.push(ChatMessage::new(ChatRole::System, nudge));
            self.notify("⚠ Auto-verify disparado (3 completions seguidas)".to_string());
        }
    }

    /// Intenta detectar un plan estructurado en el ultimo mensaje del asistente.
    /// Si lo encuentra, lo guarda en `pending_plan` para aprobacion.
    pub(crate) fn try_capture_pending_plan(&mut self) {
        let Some(last) = self.state.chat.messages.last() else {
            return;
        };
        if last.role != ChatRole::Assistant {
            return;
        }
        let plan = parse_plan_tree(&last.content);
        if plan.is_empty() {
            return;
        }
        self.state.chat.pending_plan = Some(plan);
    }

    /// Override de `handle_plan_approve` que deriva la TodoList desde el plan.
    /// El handler basico vive en `chat_tools.rs`; aqui solo amplia el efecto.
    pub(crate) fn derive_todos_from_pending_plan(&mut self) {
        let Some(plan) = self.state.chat.pending_plan.take() else {
            return;
        };
        if plan.is_empty() {
            return;
        }
        let titles: Vec<String> = plan.steps.iter().map(|s| s.title.clone()).collect();
        let n = titles.len();
        self.state.chat.todos.replace_from_steps(titles);
        self.notify(format!("✓ {n} todo(s) derivados del plan"));
    }

    fn push_assistant_markdown(&mut self, body: String) {
        let cached =
            crate::ui::widgets::markdown::render_markdown(&body, &self.state.active_theme.colors());
        let mut msg = ChatMessage::new(ChatRole::Assistant, body);
        msg.cached_lines = Some(std::sync::Arc::new(cached));
        self.state.chat.messages.push(msg);
        self.state.chat.scroll_offset = u16::MAX;
    }
}

fn parse_todo_id(arg: &str) -> Option<u32> {
    let trimmed = arg.trim().trim_start_matches('#');
    trimmed.parse::<u32>().ok()
}

/// Renderiza la lista como markdown legible dentro del chat.
fn render_todos_markdown(list: &crate::domain::todos::TodoList) -> String {
    if list.is_empty() {
        return "## Todos\n\nLista vacia. Usa `/todo-add <titulo>` o aprueba un plan con `/plan`.\n"
            .to_string();
    }
    let mut out = String::new();
    out.push_str("## Todos\n\n");
    let done = list.completed_count();
    let total = list.len();
    if let Some(progress) = list.progress() {
        let pct = (progress * 100.0).round() as u32;
        out.push_str(&format!("Progreso: **{done}/{total}** ({pct}%)\n\n"));
    }
    out.push_str("```\n");
    for item in &list.items {
        out.push_str(&format!(
            "{:>3}  {}  #{:<3}  {}\n",
            item.id,
            item.status.glyph(),
            item.status.label(),
            item.title
        ));
    }
    out.push_str("```\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_empty_returns_hint() {
        let list = crate::domain::todos::TodoList::new();
        let out = render_todos_markdown(&list);
        assert!(out.contains("Lista vacia"));
    }

    #[test]
    fn render_non_empty_includes_progress_and_items() {
        let mut list = crate::domain::todos::TodoList::new();
        list.add("hacer A");
        let id = list.add("hacer B");
        list.set_status(id, TodoStatus::Completed);
        let out = render_todos_markdown(&list);
        assert!(out.contains("1/2"));
        assert!(out.contains("hacer A"));
        assert!(out.contains("hacer B"));
    }

    #[test]
    fn parse_todo_id_accepts_hash_prefix() {
        assert_eq!(parse_todo_id("#3"), Some(3));
        assert_eq!(parse_todo_id("  7 "), Some(7));
        assert_eq!(parse_todo_id("abc"), None);
    }
}
