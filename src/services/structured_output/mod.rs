//! Structured Output (E19): parseo tipado de respuestas del assistant.
//!
//! El assistant puede emitir JSON en bloques fenced ```json o como texto crudo
//! cuando se le pide un plan, un resultado de compliance o una accion sobre
//! codigo. Este modulo define:
//!
//! - `StructuredOutput` con variantes tipadas (ComplianceResult / WorkflowPlan
//!   / CodeAction) cada una con discriminador `"kind"`.
//! - `detect_structured_output(text)` que extrae bloques candidatos del texto
//!   y devuelve la primera variante que deserializa correctamente.
//! - Fallback silencioso: si el parseo falla, retorna `None` y el texto del
//!   assistant sigue siendo el payload principal.
//!
//! No se fuerza `response_format: json_schema` en la API porque la Messages
//! API de Anthropic no lo expone nativamente y romperia streaming. En su
//! lugar, se confia en el prompting (system prompts de workflows/compliance)
//! + este parser defensivo.
//!
//! Consumidores esperados (fuera de scope de E19):
//! - UI de PlanReview: render de WorkflowPlan con steps checklist.
//! - TodoWrite (futuro E20): crear todos a partir de WorkflowPlan.steps.
//! - Compliance widget: render de violations con severity.

pub mod parser;

use serde::{Deserialize, Serialize};

pub use parser::detect_structured_output;

/// Output estructurado detectado en una respuesta del assistant.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum StructuredOutput {
    /// Resultado de una validacion de compliance (policies + ADRs).
    ComplianceResult(ComplianceResult),
    /// Plan de workflow con pasos ejecutables.
    WorkflowPlan(WorkflowPlan),
    /// Accion propuesta sobre un archivo.
    CodeAction(CodeAction),
}

impl StructuredOutput {
    /// Label corto para diagnosticos y audit log.
    pub fn kind_label(&self) -> &'static str {
        match self {
            Self::ComplianceResult(_) => "compliance_result",
            Self::WorkflowPlan(_) => "workflow_plan",
            Self::CodeAction(_) => "code_action",
        }
    }

    /// Resumen de una linea para notificaciones tipo toast.
    pub fn summary(&self) -> String {
        match self {
            Self::ComplianceResult(r) => {
                let status = if r.passed { "passed" } else { "FAILED" };
                format!("compliance {} ({} violations)", status, r.violations.len())
            }
            Self::WorkflowPlan(p) => {
                format!("workflow plan: {} ({} steps)", p.title, p.steps.len())
            }
            Self::CodeAction(a) => {
                format!("code action: {} {}", a.action.as_str(), a.target)
            }
        }
    }
}

/// Resultado de compliance validation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComplianceResult {
    pub factory: String,
    pub passed: bool,
    #[serde(default)]
    pub violations: Vec<ComplianceViolation>,
    #[serde(default)]
    pub summary: String,
}

/// Violacion individual detectada por el validador de compliance.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComplianceViolation {
    pub rule: String,
    pub severity: Severity,
    pub message: String,
    #[serde(default)]
    pub file: Option<String>,
}

/// Severidad de una violacion.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Info,
    Warn,
    Error,
}

impl Severity {
    #[allow(
        dead_code,
        reason = "consumido por UI futura (compliance widget) y por Debug del assistant stream"
    )]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Warn => "warn",
            Self::Error => "error",
        }
    }
}

/// Plan de workflow propuesto por el assistant.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkflowPlan {
    pub title: String,
    #[serde(default)]
    pub factory: Option<String>,
    #[serde(default)]
    pub steps: Vec<WorkflowStep>,
}

/// Paso individual de un WorkflowPlan.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkflowStep {
    pub order: u32,
    pub description: String,
    #[serde(default)]
    pub tool: Option<String>,
    #[serde(default)]
    pub done: bool,
}

/// Accion propuesta sobre un archivo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CodeAction {
    pub action: CodeActionKind,
    pub target: String,
    pub description: String,
    #[serde(default)]
    pub rationale: Option<String>,
}

/// Tipos de operacion sobre un archivo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodeActionKind {
    Create,
    Modify,
    Delete,
    Rename,
}

impl CodeActionKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Create => "create",
            Self::Modify => "modify",
            Self::Delete => "delete",
            Self::Rename => "rename",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_labels_are_stable() {
        let compliance = StructuredOutput::ComplianceResult(ComplianceResult {
            factory: "net".into(),
            passed: true,
            violations: vec![],
            summary: String::new(),
        });
        assert_eq!(compliance.kind_label(), "compliance_result");

        let plan = StructuredOutput::WorkflowPlan(WorkflowPlan {
            title: "t".into(),
            factory: None,
            steps: vec![],
        });
        assert_eq!(plan.kind_label(), "workflow_plan");

        let action = StructuredOutput::CodeAction(CodeAction {
            action: CodeActionKind::Create,
            target: "x.rs".into(),
            description: "d".into(),
            rationale: None,
        });
        assert_eq!(action.kind_label(), "code_action");
    }

    #[test]
    fn summary_reports_key_fields() {
        let compliance = StructuredOutput::ComplianceResult(ComplianceResult {
            factory: "ang".into(),
            passed: false,
            violations: vec![ComplianceViolation {
                rule: "R1".into(),
                severity: Severity::Error,
                message: "m".into(),
                file: None,
            }],
            summary: String::new(),
        });
        assert_eq!(compliance.summary(), "compliance FAILED (1 violations)");

        let plan = StructuredOutput::WorkflowPlan(WorkflowPlan {
            title: "Deploy".into(),
            factory: Some("all".into()),
            steps: vec![WorkflowStep {
                order: 1,
                description: "s".into(),
                tool: None,
                done: false,
            }],
        });
        assert_eq!(plan.summary(), "workflow plan: Deploy (1 steps)");

        let action = StructuredOutput::CodeAction(CodeAction {
            action: CodeActionKind::Modify,
            target: "src/lib.rs".into(),
            description: "d".into(),
            rationale: None,
        });
        assert_eq!(action.summary(), "code action: modify src/lib.rs");
    }

    #[test]
    fn serde_roundtrip_preserves_variant() {
        let original = StructuredOutput::CodeAction(CodeAction {
            action: CodeActionKind::Rename,
            target: "a.rs".into(),
            description: "rename".into(),
            rationale: Some("clarity".into()),
        });
        let json = serde_json::to_string(&original).expect("ser");
        let back: StructuredOutput = serde_json::from_str(&json).expect("de");
        assert_eq!(back, original);
    }

    #[test]
    fn severity_serializes_lowercase() {
        let json = serde_json::to_string(&Severity::Warn).unwrap();
        assert_eq!(json, "\"warn\"");
    }

    #[test]
    fn compliance_violation_file_is_optional() {
        let json = r#"{"rule":"R","severity":"info","message":"m"}"#;
        let v: ComplianceViolation = serde_json::from_str(json).unwrap();
        assert!(v.file.is_none());
    }
}
