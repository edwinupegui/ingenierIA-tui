//! TodoWriteTool — permite al AI sincronizar la lista de todos del chat.
//!
//! Diseño: mismo patrón que `config_tool` — NO vive en `ToolRegistry` porque
//! necesita una `Sender<Action>` para aplicar cambios al `ChatState.todos`.
//! La definición se expone via `todo_write_definition()` y el dispatch
//! especial está en `chat_tools.rs` antes del fallback a registry/MCP.
//!
//! Semántica (Claude Code-style):
//! - El AI manda la lista COMPLETA de todos en cada call. No hay "add" o
//!   "remove" incremental — el handler reemplaza el estado con la lista
//!   recibida, preservando ids monotónicos cuando coincide el título.
//! - Exactamente un item puede estar `in_progress` a la vez.

use tokio::sync::mpsc::Sender;

use crate::actions::Action;
use crate::services::chat::ToolDefinition;
use ingenieria_domain::todos::TodoStatus;

pub const TODO_WRITE_NAME: &str = "todo_write";

/// Entrada plana que el AI envía (sin id: el reducer asigna/preserva).
#[derive(Debug, Clone, serde::Deserialize)]
pub struct TodoInput {
    /// Título imperativo (ej: "Lee Cargo.toml y añade dep serde").
    #[serde(alias = "title")]
    pub content: String,
    /// Estado: "pending" | "in_progress" | "completed". Default pending.
    #[serde(default)]
    pub status: TodoInputStatus,
    /// Forma present-continuous para el spinner. No persistida; solo UX hint
    /// que el AI envia; el TUI actual no la muestra (reservado para E12 v2).
    #[allow(dead_code)]
    #[serde(default, rename = "activeForm", alias = "active_form")]
    pub active_form: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TodoInputStatus {
    #[default]
    Pending,
    InProgress,
    Completed,
}

impl From<TodoInputStatus> for TodoStatus {
    fn from(s: TodoInputStatus) -> Self {
        match s {
            TodoInputStatus::Pending => TodoStatus::Pending,
            TodoInputStatus::InProgress => TodoStatus::InProgress,
            TodoInputStatus::Completed => TodoStatus::Completed,
        }
    }
}

pub fn todo_write_definition() -> ToolDefinition {
    ToolDefinition {
        json: serde_json::json!({
            "type": "function",
            "function": {
                "name": TODO_WRITE_NAME,
                "description": "Sincroniza la lista de todos del chat. Envía SIEMPRE la lista completa (no es incremental). Usa para trackear progreso en tareas de múltiples pasos. Exactamente un item puede estar en 'in_progress' a la vez.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "todos": {
                            "type": "array",
                            "description": "Lista completa de tasks actual",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "content": { "type": "string", "description": "Descripción imperativa del task" },
                                    "status": { "type": "string", "enum": ["pending", "in_progress", "completed"] },
                                    "activeForm": { "type": "string", "description": "Present-continuous para spinner" }
                                },
                                "required": ["content", "status"]
                            }
                        }
                    },
                    "required": ["todos"]
                }
            }
        }),
    }
}

pub async fn handle_request(arguments: &str, tx: &Sender<Action>) -> String {
    let items = match parse_request(arguments) {
        Ok(v) => v,
        Err(e) => return format!("Error: {e}"),
    };
    let summary = format_summary(&items);
    if tx.send(Action::ApplyTodoWrite { items }).await.is_err() {
        return "Error: canal de actions cerrado".into();
    }
    summary
}

fn parse_request(arguments: &str) -> anyhow::Result<Vec<TodoInput>> {
    #[derive(serde::Deserialize)]
    struct Payload {
        todos: Vec<TodoInput>,
    }
    let p: Payload = serde_json::from_str(arguments)
        .map_err(|e| anyhow::anyhow!("arguments JSON inválido: {e}"))?;
    validate_single_in_progress(&p.todos)?;
    Ok(p.todos)
}

fn validate_single_in_progress(items: &[TodoInput]) -> anyhow::Result<()> {
    let in_prog = items.iter().filter(|t| t.status == TodoInputStatus::InProgress).count();
    if in_prog > 1 {
        anyhow::bail!("{in_prog} items en in_progress — solo puede haber 1 activo a la vez");
    }
    Ok(())
}

fn format_summary(items: &[TodoInput]) -> String {
    let pending = items.iter().filter(|t| t.status == TodoInputStatus::Pending).count();
    let in_progress = items.iter().filter(|t| t.status == TodoInputStatus::InProgress).count();
    let completed = items.iter().filter(|t| t.status == TodoInputStatus::Completed).count();
    let total = items.len();
    format!(
        "TodoList sincronizada ({total} items): {completed} completed, {in_progress} in_progress, {pending} pending"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_minimal_list() {
        let args = r#"{"todos":[{"content":"paso 1","status":"pending"}]}"#;
        let out = parse_request(args).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].content, "paso 1");
        assert_eq!(out[0].status, TodoInputStatus::Pending);
    }

    #[test]
    fn rejects_two_in_progress() {
        let args = r#"{"todos":[
            {"content":"a","status":"in_progress"},
            {"content":"b","status":"in_progress"}
        ]}"#;
        let err = parse_request(args).unwrap_err();
        assert!(err.to_string().contains("solo puede haber 1"));
    }

    #[test]
    fn accepts_active_form_and_title_aliases() {
        let args = r#"{"todos":[{"title":"foo","status":"in_progress","activeForm":"fooing"}]}"#;
        let out = parse_request(args).unwrap();
        assert_eq!(out[0].content, "foo");
        assert_eq!(out[0].active_form.as_deref(), Some("fooing"));
    }

    #[test]
    fn invalid_json_errors() {
        let err = parse_request("{bad").unwrap_err();
        assert!(err.to_string().contains("JSON"));
    }

    #[test]
    fn format_summary_counts() {
        let items = vec![
            TodoInput {
                content: "a".into(),
                status: TodoInputStatus::Completed,
                active_form: None,
            },
            TodoInput {
                content: "b".into(),
                status: TodoInputStatus::InProgress,
                active_form: None,
            },
            TodoInput { content: "c".into(), status: TodoInputStatus::Pending, active_form: None },
        ];
        let out = format_summary(&items);
        assert!(out.contains("3 items"));
        assert!(out.contains("1 completed"));
        assert!(out.contains("1 in_progress"));
        assert!(out.contains("1 pending"));
    }
}
