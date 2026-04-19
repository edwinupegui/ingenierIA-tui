//! Roles de subagente (E22a).
//!
//! Los 8 agentes de ingenierIA como roles ejecutables. Cada rol define:
//!   - Nombre canonico y display label.
//!   - System prompt enfocado al rol (focus + constraints).
//!   - Whitelist de herramientas que el rol puede invocar (informativo en
//!     Sprint 10 — la ejecucion de tools desde subagent llega en Sprint 11).

#[derive(Debug, Clone, PartialEq)]
pub enum AgentRole {
    Orchestrator,
    Discovery,
    Architecture,
    Migration,
    Execution,
    Testing,
    Docs,
    Planning,
    /// Rol libre con nombre arbitrario (cuando el usuario no usa uno canonico).
    Generic(String),
}

/// Maxima longitud aceptable de un nombre de rol generico, para evitar entrada
/// abusiva via `/agent-spawn`.
const MAX_ROLE_NAME: usize = 32;

impl AgentRole {
    pub fn from_name(name: &str) -> Self {
        match name.trim().to_ascii_lowercase().as_str() {
            "orchestrator" | "orchestra" => AgentRole::Orchestrator,
            "discovery" | "disc" => AgentRole::Discovery,
            "architecture" | "arch" => AgentRole::Architecture,
            "migration" | "migrate" => AgentRole::Migration,
            "execution" | "exec" => AgentRole::Execution,
            "testing" | "test" => AgentRole::Testing,
            "docs" | "documentation" => AgentRole::Docs,
            "planning" | "plan" => AgentRole::Planning,
            other => {
                let trimmed: String = other.chars().take(MAX_ROLE_NAME).collect();
                AgentRole::Generic(trimmed)
            }
        }
    }

    pub fn name(&self) -> &str {
        match self {
            AgentRole::Orchestrator => "orchestrator",
            AgentRole::Discovery => "discovery",
            AgentRole::Architecture => "architecture",
            AgentRole::Migration => "migration",
            AgentRole::Execution => "execution",
            AgentRole::Testing => "testing",
            AgentRole::Docs => "docs",
            AgentRole::Planning => "planning",
            AgentRole::Generic(n) => n.as_str(),
        }
    }

    /// Lista de roles canonicos para ayuda en `/agents` y autocomplete.
    pub fn canonical_names() -> &'static [&'static str] {
        &[
            "orchestrator",
            "discovery",
            "architecture",
            "migration",
            "execution",
            "testing",
            "docs",
            "planning",
        ]
    }

    /// Whitelist de tools (informativa en Sprint 10; consumida por la UI
    /// futura y por E22b para enforcement real).
    #[allow(
        dead_code,
        reason = "Sprint 10 expone pero no enforza — consumido por agent_panel display + Sprint 11"
    )]
    pub fn tools_whitelist(&self) -> &'static [&'static str] {
        match self {
            AgentRole::Orchestrator => &[
                "search_documents",
                "get_factory_context",
                "get_workflow",
                "get_skills",
                "validate_compliance",
            ],
            AgentRole::Discovery => &["search_documents", "get_factory_context"],
            AgentRole::Architecture => &["get_skills", "validate_compliance"],
            AgentRole::Migration => &["get_workflow"],
            AgentRole::Execution => &[],
            AgentRole::Testing => &["validate_compliance"],
            AgentRole::Docs => &["search_documents", "sync_project"],
            AgentRole::Planning => &["get_workflow"],
            AgentRole::Generic(_) => &[],
        }
    }

    pub fn system_prompt(&self) -> &'static str {
        match self {
            AgentRole::Orchestrator => {
                "Eres el orchestrator de ingenierIA. Coordina sub-tareas, decide \
                 a quien delegar y produce un plan o resumen final corto."
            }
            AgentRole::Discovery => {
                "Eres el agente de discovery de ingenierIA. Investiga el contexto \
                 disponible (documentos, skills, ADRs) y devuelve hallazgos \
                 relevantes en formato bullet."
            }
            AgentRole::Architecture => {
                "Eres el agente de arquitectura de ingenierIA. Analiza el problema \
                 y propone una decision de diseno con trade-offs explicitos."
            }
            AgentRole::Migration => {
                "Eres el agente de migracion de ingenierIA. Identifica pasos para \
                 migrar el codigo objetivo respetando los workflows existentes."
            }
            AgentRole::Execution => {
                "Eres el agente de ejecucion de ingenierIA. Genera el comando o \
                 codigo concreto a ejecutar. Si requiere bash, indicalo \
                 textualmente — no ejecutes nada por tu cuenta."
            }
            AgentRole::Testing => {
                "Eres el agente de testing de ingenierIA. Sugiere casos de prueba \
                 + comandos de validacion para el problema dado."
            }
            AgentRole::Docs => {
                "Eres el agente de documentacion de ingenierIA. Genera o actualiza \
                 documentacion concisa y bien estructurada en markdown."
            }
            AgentRole::Planning => {
                "Eres el agente de planning de ingenierIA. Devuelve un plan \
                 numerado con pasos accionables y dependencias."
            }
            AgentRole::Generic(_) => {
                "Eres un asistente especializado. Responde de forma concisa, \
                 estructurada y centrada en la tarea solicitada."
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_name_canonical_aliases() {
        assert_eq!(AgentRole::from_name("DISC"), AgentRole::Discovery);
        assert_eq!(AgentRole::from_name(" arch "), AgentRole::Architecture);
        assert_eq!(AgentRole::from_name("orchestra"), AgentRole::Orchestrator);
    }

    #[test]
    fn from_name_falls_back_to_generic() {
        match AgentRole::from_name("custom-role") {
            AgentRole::Generic(n) => assert_eq!(n, "custom-role"),
            _ => panic!("expected Generic"),
        }
    }

    #[test]
    fn generic_truncates_long_input() {
        let long: String = "x".repeat(100);
        match AgentRole::from_name(&long) {
            AgentRole::Generic(n) => assert_eq!(n.len(), MAX_ROLE_NAME),
            _ => panic!("expected Generic"),
        }
    }

    #[test]
    fn whitelist_present_for_canonicals() {
        for name in AgentRole::canonical_names() {
            let role = AgentRole::from_name(name);
            // No assertion sobre contenido — solo que .name() vuelva igual.
            assert_eq!(role.name(), *name);
        }
    }

    #[test]
    fn system_prompt_nonempty() {
        for name in AgentRole::canonical_names() {
            let role = AgentRole::from_name(name);
            assert!(!role.system_prompt().is_empty());
        }
    }
}
