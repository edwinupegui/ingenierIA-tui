//! Audit Log (E13): registro estructurado de eventos significativos.
//!
//! Cada entry JSONL contiene: timestamp ISO, kind (ToolCall / ToolResult /
//! Failure / Permission / Fork), session_id, y payload redactado.
//!
//! Diseno:
//! - Append-only JSONL rotado por dia.
//! - Redactor de secretos aplicado automaticamente a strings.
//! - Fire-and-forget: errores de escritura van a `tracing::warn!` pero no
//!   bloquean el flujo principal.

pub mod redactor;
pub mod storage;

/// Re-export del lock compartido (ver `services::TEST_ENV_LOCK`).
#[cfg(test)]
#[cfg(test)]
pub(super) use crate::TEST_ENV_LOCK;

use serde::{Deserialize, Serialize};

pub use redactor::redact_secrets;
pub use storage::{export_to, list_log_files};

use ingenieria_domain::failure::StructuredFailure;

/// Tipo de evento auditado.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AuditKind {
    ToolCall {
        tool: String,
        arguments: String,
        /// True si el usuario aprobo explicitamente, None si fue auto-approved.
        approved: Option<bool>,
    },
    ToolResult {
        tool: String,
        tool_call_id: String,
        success: bool,
        bytes: usize,
    },
    Failure {
        failure: StructuredFailure,
    },
    PermissionDecision {
        tool: String,
        decision: String,
    },
    SessionFork {
        parent_id: String,
        child_id: String,
        label: String,
    },
    /// ConfigTool (E20): el AI cambio un campo runtime/persistente.
    ConfigUpdated {
        field: String,
        old_value: String,
        new_value: String,
    },
}

/// Entry serializado al JSONL.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub timestamp: String,
    pub session_id: String,
    #[serde(flatten)]
    pub kind: AuditKind,
}

impl AuditEntry {
    pub fn new(session_id: String, kind: AuditKind) -> Self {
        Self { timestamp: ingenieria_domain::time::now_iso(), session_id, kind }
    }
}

/// Loguea un entry al audit log. Fire-and-forget: si falla, logea warning.
/// Aplica redactor automaticamente a los campos de strings.
pub fn log_entry(entry: AuditEntry) {
    let redacted = redact_entry(entry);
    let line = match serde_json::to_string(&redacted) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(error = %e, "failed to serialize audit entry");
            return;
        }
    };
    if let Err(e) = storage::append_line(&line) {
        tracing::warn!(error = %e, "failed to append audit entry");
    }
}

fn redact_entry(mut entry: AuditEntry) -> AuditEntry {
    entry.kind = match entry.kind {
        AuditKind::ToolCall { tool, arguments, approved } => {
            AuditKind::ToolCall { tool, arguments: redact_secrets(&arguments), approved }
        }
        AuditKind::ToolResult { tool, tool_call_id, success, bytes } => {
            AuditKind::ToolResult { tool, tool_call_id, success, bytes }
        }
        AuditKind::Failure { mut failure } => {
            failure.message = redact_secrets(&failure.message);
            AuditKind::Failure { failure }
        }
        other => other,
    };
    entry
}

#[cfg(test)]
mod tests {
    use super::*;
    use ingenieria_domain::failure::{FailureCategory, StructuredFailure};

    #[test]
    fn tool_call_entry_serializes_with_kind_tag() {
        let entry = AuditEntry::new(
            "sess1".into(),
            AuditKind::ToolCall {
                tool: "bash".into(),
                arguments: "ls -la".into(),
                approved: Some(true),
            },
        );
        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("\"kind\":\"tool_call\""));
        assert!(json.contains("\"session_id\":\"sess1\""));
    }

    #[test]
    fn redact_entry_strips_api_key_from_arguments() {
        let entry = AuditEntry::new(
            "s".into(),
            AuditKind::ToolCall {
                tool: "curl".into(),
                arguments: "-H 'Authorization: Bearer sk-ant-abc123def456ghi789jkl012mno'".into(),
                approved: None,
            },
        );
        let redacted = redact_entry(entry);
        let json = serde_json::to_string(&redacted).unwrap();
        assert!(json.contains("[REDACTED]"));
        assert!(!json.contains("sk-ant-abc123"));
    }

    #[test]
    fn failure_entry_redacts_message() {
        let failure = StructuredFailure::new(
            FailureCategory::ApiKeyInvalid,
            "Bearer ghp_abc123def456ghi789jkl012 expired",
        );
        let entry = AuditEntry::new("s".into(), AuditKind::Failure { failure });
        let redacted = redact_entry(entry);
        let json = serde_json::to_string(&redacted).unwrap();
        assert!(json.contains("[REDACTED]"));
    }
}
