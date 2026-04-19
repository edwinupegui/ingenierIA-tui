//! Estado del modal de `@` mentions (F2).
//!
//! Se dispara con `@` en el input del chat y permite insertar referencias a
//! documentos (skills, agents, workflows, adrs, policies, commands) que la
//! AI puede consumir como contexto. Reutiliza el matching fuzzy de
//! `nucleo_matcher` — mismo criterio que `history_search`.
//!
//! Inspirado en el `@` mention picker de opencode-dev (slash-popover.tsx).

use nucleo_matcher::{
    pattern::{CaseMatching, Normalization, Pattern},
    Matcher,
};

use ingenieria_domain::document::DocumentSummary;

/// Límite de items mostrados en el popover.
pub const MENTION_PICKER_LIMIT: usize = 10;

/// Tipo de mention; determina cómo se inserta en el input y el color del
/// badge en el widget.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MentionKind {
    Skill,
    Agent,
    Workflow,
    Adr,
    Policy,
    Command,
    Other,
}

impl MentionKind {
    /// Infiere el kind desde el `doc_type` del server.
    pub fn from_doc_type(s: &str) -> Self {
        match s {
            "skill" => Self::Skill,
            "agent" => Self::Agent,
            "workflow" => Self::Workflow,
            "adr" => Self::Adr,
            "policy" => Self::Policy,
            "command" => Self::Command,
            _ => Self::Other,
        }
    }

    /// Etiqueta corta mostrada en el badge.
    pub fn label(self) -> &'static str {
        match self {
            Self::Skill => "skill",
            Self::Agent => "agent",
            Self::Workflow => "workflow",
            Self::Adr => "adr",
            Self::Policy => "policy",
            Self::Command => "command",
            Self::Other => "doc",
        }
    }
}

/// Item candidato del picker.
#[derive(Debug, Clone)]
pub struct MentionItem {
    pub kind: MentionKind,
    pub name: String,
    pub description: String,
}

impl MentionItem {
    /// Texto que se pega en el input al seleccionar el item.
    pub fn insert_text(&self) -> String {
        format!("@{}:{}", self.kind.label(), self.name)
    }
}

#[derive(Debug, Default)]
pub struct MentionPicker {
    pub visible: bool,
    pub query: String,
    pub items: Vec<MentionItem>,
    pub matches: Vec<(usize, u32)>,
    pub cursor: usize,
}

impl MentionPicker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Abre el picker poblando el pool desde los documentos del sidebar.
    pub fn open(&mut self, docs: &[DocumentSummary]) {
        self.items = docs
            .iter()
            .map(|d| MentionItem {
                kind: MentionKind::from_doc_type(&d.doc_type),
                name: d.name.clone(),
                description: d.description.clone(),
            })
            .collect();
        self.visible = true;
        self.query.clear();
        self.cursor = 0;
        self.recompute();
    }

    pub fn close(&mut self) {
        self.visible = false;
        self.query.clear();
        self.items.clear();
        self.matches.clear();
        self.cursor = 0;
    }

    /// Actualiza `matches` usando fuzzy matching sobre `name + description`.
    pub fn recompute(&mut self) {
        self.matches.clear();
        if self.items.is_empty() {
            return;
        }
        if self.query.is_empty() {
            self.matches = self
                .items
                .iter()
                .enumerate()
                .take(MENTION_PICKER_LIMIT)
                .map(|(i, _)| (i, 0u32))
                .collect();
            self.clamp_cursor();
            return;
        }
        let mut matcher = Matcher::new(nucleo_matcher::Config::DEFAULT);
        let pattern = Pattern::parse(&self.query, CaseMatching::Ignore, Normalization::Smart);
        let mut buf = Vec::new();
        let mut scored: Vec<(usize, u32)> = self
            .items
            .iter()
            .enumerate()
            .filter_map(|(i, item)| {
                let haystack = format!("{} {}", item.name, item.description);
                let needle = nucleo_matcher::Utf32Str::new(&haystack, &mut buf);
                pattern.score(needle, &mut matcher).map(|score| (i, score))
            })
            .collect();
        scored.sort_by(|a, b| b.1.cmp(&a.1));
        scored.truncate(MENTION_PICKER_LIMIT);
        self.matches = scored;
        self.clamp_cursor();
    }

    fn clamp_cursor(&mut self) {
        if self.cursor >= self.matches.len() {
            self.cursor = self.matches.len().saturating_sub(1);
        }
    }

    pub fn move_up(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
    }

    pub fn move_down(&mut self) {
        if self.cursor + 1 < self.matches.len() {
            self.cursor += 1;
        }
    }

    /// Devuelve el item seleccionado según el cursor.
    pub fn selected(&self) -> Option<&MentionItem> {
        self.matches.get(self.cursor).and_then(|(i, _)| self.items.get(*i))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_doc(doc_type: &str, name: &str) -> DocumentSummary {
        DocumentSummary {
            uri: format!("ingenieria://{doc_type}/{name}"),
            doc_type: doc_type.to_string(),
            factory: "all".to_string(),
            name: name.to_string(),
            description: format!("desc for {name}"),
            last_modified: "2026-04-17T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn open_populates_items_from_docs() {
        let docs = vec![sample_doc("skill", "auth-flow"), sample_doc("adr", "adr-001")];
        let mut p = MentionPicker::new();
        p.open(&docs);
        assert!(p.visible);
        assert_eq!(p.items.len(), 2);
        assert_eq!(p.matches.len(), 2);
    }

    #[test]
    fn close_clears_state() {
        let mut p = MentionPicker::new();
        p.open(&[sample_doc("skill", "foo")]);
        p.close();
        assert!(!p.visible);
        assert!(p.items.is_empty());
        assert!(p.matches.is_empty());
    }

    #[test]
    fn query_filters_fuzzy() {
        let docs = vec![
            sample_doc("skill", "auth-flow"),
            sample_doc("skill", "billing-api"),
            sample_doc("adr", "architecture-v1"),
        ];
        let mut p = MentionPicker::new();
        p.open(&docs);
        p.query = "auth".into();
        p.recompute();
        assert!(!p.matches.is_empty());
        assert_eq!(p.selected().map(|i| i.name.as_str()), Some("auth-flow"));
    }

    #[test]
    fn insert_text_formats_with_kind_prefix() {
        let item = MentionItem {
            kind: MentionKind::Skill,
            name: "auth-flow".into(),
            description: String::new(),
        };
        assert_eq!(item.insert_text(), "@skill:auth-flow");
    }

    #[test]
    fn mention_kind_from_doc_type_maps_known_types() {
        assert_eq!(MentionKind::from_doc_type("skill"), MentionKind::Skill);
        assert_eq!(MentionKind::from_doc_type("adr"), MentionKind::Adr);
        assert_eq!(MentionKind::from_doc_type("unknown"), MentionKind::Other);
    }

    #[test]
    fn cursor_navigation_stays_in_bounds() {
        let docs = vec![sample_doc("skill", "a"), sample_doc("skill", "b")];
        let mut p = MentionPicker::new();
        p.open(&docs);
        p.move_up();
        assert_eq!(p.cursor, 0);
        p.move_down();
        assert_eq!(p.cursor, 1);
        p.move_down();
        assert_eq!(p.cursor, 1);
    }
}
