use std::collections::{HashMap, HashSet};

use crate::domain::document::{DocumentDetail, DocumentSummary};
use crate::domain::search::SearchResultItem;

// ── Search ───────────────────────────────────────────────────────────────────

pub struct SearchState {
    pub query: String,
    pub results: Vec<SearchResultItem>,
    pub cursor: usize,
    pub loading: bool,
    pub error: Option<String>,
}

impl SearchState {
    pub fn new() -> Self {
        Self { query: String::new(), results: Vec::new(), cursor: 0, loading: false, error: None }
    }

    pub fn move_up(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    pub fn move_down(&mut self) {
        if !self.results.is_empty() {
            self.cursor = (self.cursor + 1).min(self.results.len() - 1);
        }
    }

    pub fn reset(&mut self) {
        self.query.clear();
        self.results.clear();
        self.cursor = 0;
        self.loading = false;
        self.error = None;
    }
}

// ── Sidebar ──────────────────────────────────────────────────────────────────

pub struct SidebarSection {
    pub doc_type: &'static str,
    pub label: &'static str,
    pub expanded: bool,
    pub items: Vec<DocumentSummary>,
    /// Pre-computed section header string — evita format!() en el render loop.
    pub header: String,
}

impl SidebarSection {
    fn new(doc_type: &'static str, label: &'static str) -> Self {
        let mut s =
            Self { doc_type, label, expanded: true, items: Vec::new(), header: String::new() };
        s.rebuild_header();
        s
    }

    pub fn rebuild_header(&mut self) {
        let count = self.items.len();
        let icon = if self.expanded { "▼" } else { "▶" };
        self.header = format!(" {icon} {} ({count})", self.label);
    }
}

pub struct SidebarState {
    pub sections: Vec<SidebarSection>,
    pub cursor_pos: usize,
    pub all_docs: Vec<DocumentSummary>,
    pub loading: bool,
    pub error: Option<String>,
    /// True when docs were loaded from offline cache instead of server.
    pub is_cached: bool,
}

impl SidebarState {
    pub fn new() -> Self {
        Self {
            sections: vec![
                SidebarSection::new("skill", "Skills"),
                SidebarSection::new("command", "Commands"),
                SidebarSection::new("workflow", "Workflows"),
                SidebarSection::new("adr", "ADRs"),
                SidebarSection::new("policy", "Policies"),
                SidebarSection::new("agent", "Agents"),
            ],
            cursor_pos: 0,
            all_docs: Vec::new(),
            loading: false,
            error: None,
            is_cached: false,
        }
    }

    pub fn visible_count(&self) -> usize {
        self.sections.iter().map(|s| 1 + if s.expanded { s.items.len() } else { 0 }).sum()
    }

    pub fn move_down(&mut self) {
        let count = self.visible_count();
        if count > 0 {
            self.cursor_pos = (self.cursor_pos + 1).min(count - 1);
        }
    }

    pub fn move_up(&mut self) {
        self.cursor_pos = self.cursor_pos.saturating_sub(1);
    }

    pub fn toggle_current(&mut self) {
        let mut flat = 0usize;
        for si in 0..self.sections.len() {
            if flat == self.cursor_pos {
                self.sections[si].expanded = !self.sections[si].expanded;
                self.sections[si].rebuild_header();
                let count = self.visible_count();
                if count > 0 {
                    self.cursor_pos = self.cursor_pos.min(count - 1);
                }
                return;
            }
            flat += 1;
            if self.sections[si].expanded {
                let items_count = self.sections[si].items.len();
                if self.cursor_pos < flat + items_count {
                    self.sections[si].expanded = false;
                    self.sections[si].rebuild_header();
                    self.cursor_pos = flat - 1;
                    return;
                }
                flat += items_count;
            }
        }
    }

    pub fn current_doc(&self) -> Option<&DocumentSummary> {
        let mut flat = 0usize;
        for section in &self.sections {
            flat += 1;
            if section.expanded {
                for item in &section.items {
                    if flat == self.cursor_pos {
                        return Some(item);
                    }
                    flat += 1;
                }
            }
        }
        None
    }

    /// Rebuild sidebar sections. If `factory_key` is Some, filter to that factory.
    /// If `priority_factory` is Some, sort matching items first within each section.
    pub fn rebuild_with_priority(
        &mut self,
        factory_key: Option<&str>,
        priority_factory: Option<&str>,
    ) {
        for section in &mut self.sections {
            section.items = self
                .all_docs
                .iter()
                .filter(|d| {
                    d.doc_type == section.doc_type && factory_key.is_none_or(|f| d.factory == f)
                })
                .cloned()
                .collect();
            // Sort: priority factory first, then alphabetical
            if let Some(pf) = priority_factory {
                section.items.sort_by(|a, b| {
                    let a_match = a.factory == pf;
                    let b_match = b.factory == pf;
                    b_match.cmp(&a_match).then_with(|| a.name.cmp(&b.name))
                });
            }
            section.rebuild_header();
        }
        let count = self.visible_count();
        if count > 0 {
            self.cursor_pos = self.cursor_pos.min(count - 1);
        }
    }
}

// ── Preview ──────────────────────────────────────────────────────────────────

pub struct PreviewState {
    pub doc: Option<DocumentDetail>,
    pub scroll: u16,
    pub loading: bool,
    /// Cached rendered markdown lines. Invalidated when doc changes.
    /// Wrapped in Arc to avoid deep cloning on every render frame.
    pub cached_lines: Option<std::sync::Arc<Vec<ratatui::text::Line<'static>>>>,
}

impl PreviewState {
    pub fn new() -> Self {
        Self { doc: None, scroll: 0, loading: false, cached_lines: None }
    }

    /// Set a new document and invalidate the cache.
    pub fn set_doc(&mut self, doc: DocumentDetail) {
        self.doc = Some(doc);
        self.cached_lines = None;
        self.scroll = 0;
    }
}

// ── Dashboard ────────────────────────────────────────────────────────────────

pub struct DashboardState {
    pub sidebar: SidebarState,
    pub preview: PreviewState,
    pub sync: SyncState,
}

impl DashboardState {
    pub fn new() -> Self {
        Self { sidebar: SidebarState::new(), preview: PreviewState::new(), sync: SyncState::new() }
    }
}

// ── Sync tracking ───────────────────────────────────────────────────────

pub struct SyncState {
    /// URIs of documents updated since last sync (e.g. "ingenieria://skill/net/add-feature")
    pub updated_uris: HashSet<String>,
    /// Count of updated docs per doc_type section
    pub badges: HashMap<String, usize>,
    /// Whether a sync check is in progress
    pub loading: bool,
}

impl SyncState {
    pub fn new() -> Self {
        Self { updated_uris: HashSet::new(), badges: HashMap::new(), loading: false }
    }

    /// Recompute badges by counting updated URIs per doc_type.
    /// URI format: ingenieria://type/factory/name
    pub fn recompute_badges(&mut self) {
        self.badges.clear();
        for uri in &self.updated_uris {
            if let Some(doc_type) = parse_doc_type_from_uri(uri) {
                *self.badges.entry(doc_type).or_insert(0) += 1;
            }
        }
    }

    pub fn badge_for(&self, doc_type: &str) -> Option<usize> {
        self.badges.get(doc_type).copied().filter(|&n| n > 0)
    }

    pub fn total_updated(&self) -> usize {
        self.updated_uris.len()
    }
}

/// Parse doc_type from a ingenieria URI: "ingenieria://type/factory/name" → "type"
fn parse_doc_type_from_uri(uri: &str) -> Option<String> {
    let path = uri.strip_prefix("ingenieria://")?;
    let doc_type = path.split('/').next()?;
    if doc_type.is_empty() {
        return None;
    }
    Some(doc_type.to_string())
}
