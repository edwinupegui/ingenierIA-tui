//! Typeahead de sugerencias para el input del chat (E30b).
//!
//! Construye una lista ordenada de candidatos para el input actual del
//! usuario, integrando varias fuentes: slash commands, mentions de agentes,
//! nombres de factories y los inputs recientes del historial. El widget de
//! slash autocomplete consume directamente esta lista; en el futuro un
//! footer dedicado puede mostrar el primer candidato como ghost-text.

use crate::services::agents::AgentRole;
use crate::state::chat_types::SLASH_COMMANDS;

/// Tipos de sugerencia con su prefijo asociado.
#[derive(Debug, Clone, PartialEq)]
pub enum SuggestionKind {
    /// `/cmd` — comando interno.
    SlashCommand,
    /// `@agent` — mention de un rol canonico de subagente.
    AgentMention,
    /// `#factory` — selector de factory.
    FactoryName,
    /// Input previo del historial.
    RecentHistory,
}

#[derive(Debug, Clone)]
pub struct Suggestion {
    pub kind: SuggestionKind,
    pub display: String,
    /// Texto que reemplaza el input cuando el usuario acepta (suele ser igual
    /// a `display`; lo dejamos separado para futuras sugerencias parciales).
    #[allow(
        dead_code,
        reason = "consumido por slash_autocomplete cuando integre Suggestion en Sprint 11"
    )]
    pub insertion: String,
}

/// Tope de sugerencias que se devuelven, sin importar cuantas matcheen.
pub const SUGGESTION_LIMIT: usize = 8;

/// Genera sugerencias para un input parcial. El caller decide donde mostrar
/// el resultado (footer ghost, slash popup, o un overlay propio).
pub fn suggest(input: &str, history: &[String]) -> Vec<Suggestion> {
    let trimmed = input.trim_start();
    if trimmed.is_empty() {
        return Vec::new();
    }
    let mut out: Vec<Suggestion> = Vec::new();
    if let Some(rest) = trimmed.strip_prefix('/') {
        out.extend(slash_matches(rest));
    } else if let Some(rest) = trimmed.strip_prefix('@') {
        out.extend(agent_matches(rest));
    } else if let Some(rest) = trimmed.strip_prefix('#') {
        out.extend(factory_matches(rest));
    } else {
        out.extend(history_matches(trimmed, history));
    }
    out.truncate(SUGGESTION_LIMIT);
    out
}

fn slash_matches(query: &str) -> impl Iterator<Item = Suggestion> + '_ {
    let q = query.to_ascii_lowercase();
    SLASH_COMMANDS.iter().filter_map(move |(cmd, _desc)| {
        let name = cmd.trim_start_matches('/');
        if q.is_empty() || name.starts_with(&q) {
            Some(Suggestion {
                kind: SuggestionKind::SlashCommand,
                display: (*cmd).to_string(),
                insertion: (*cmd).to_string(),
            })
        } else {
            None
        }
    })
}

fn agent_matches(query: &str) -> impl Iterator<Item = Suggestion> + '_ {
    let q = query.to_ascii_lowercase();
    AgentRole::canonical_names().iter().filter_map(move |name| {
        if q.is_empty() || name.starts_with(&q) {
            Some(Suggestion {
                kind: SuggestionKind::AgentMention,
                display: format!("@{name}"),
                insertion: format!("@{name}"),
            })
        } else {
            None
        }
    })
}

const FACTORY_NAMES: &[&str] = &["net", "ang", "nest", "all"];

fn factory_matches(query: &str) -> impl Iterator<Item = Suggestion> + '_ {
    let q = query.to_ascii_lowercase();
    FACTORY_NAMES.iter().filter_map(move |name| {
        if q.is_empty() || name.starts_with(&q) {
            Some(Suggestion {
                kind: SuggestionKind::FactoryName,
                display: format!("#{name}"),
                insertion: format!("#{name}"),
            })
        } else {
            None
        }
    })
}

fn history_matches<'a>(
    query: &'a str,
    history: &'a [String],
) -> impl Iterator<Item = Suggestion> + 'a {
    let q = query.to_ascii_lowercase();
    history.iter().rev().filter(move |entry| entry.to_ascii_lowercase().contains(&q)).map(|entry| {
        Suggestion {
            kind: SuggestionKind::RecentHistory,
            display: entry.clone(),
            insertion: entry.clone(),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_input_returns_no_suggestions() {
        assert!(suggest("", &[]).is_empty());
        assert!(suggest("   ", &[]).is_empty());
    }

    #[test]
    fn slash_prefix_lists_matching_commands() {
        let out = suggest("/cron", &[]);
        assert!(out.iter().all(|s| s.kind == SuggestionKind::SlashCommand));
        assert!(out.iter().any(|s| s.display.starts_with("/cron")));
    }

    #[test]
    fn at_prefix_lists_agent_roles() {
        let out = suggest("@disc", &[]);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].display, "@discovery");
        assert_eq!(out[0].kind, SuggestionKind::AgentMention);
    }

    #[test]
    fn hash_prefix_lists_factories() {
        let out = suggest("#an", &[]);
        assert_eq!(out.iter().filter(|s| s.display == "#ang").count(), 1);
    }

    #[test]
    fn plain_text_searches_history() {
        let history = vec!["explain the diff".to_string(), "show files".to_string()];
        let out = suggest("diff", &history);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].kind, SuggestionKind::RecentHistory);
        assert_eq!(out[0].display, "explain the diff");
    }

    #[test]
    fn truncates_to_suggestion_limit() {
        let out = suggest("/", &[]);
        assert!(out.len() <= SUGGESTION_LIMIT);
    }
}
