//! Offline document cache.
//!
//! Caches document summaries to `~/.config/ingenieria-tui/doc_cache.json`
//! so the dashboard can display them when the MCP server is unreachable.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::domain::document::DocumentSummary;

#[derive(Debug, Serialize, Deserialize)]
struct CacheFile {
    cached_at: String,
    documents: Vec<DocumentSummary>,
}

/// Save document summaries to the local cache.
pub fn save(docs: &[DocumentSummary]) {
    let Some(path) = cache_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let file = CacheFile { cached_at: crate::services::sync::now_iso(), documents: docs.to_vec() };
    if let Ok(json) = serde_json::to_string(&file) {
        let _ = std::fs::write(path, json);
    }
}

/// Load document summaries from local cache. Returns None if no cache exists.
pub fn load() -> Option<(Vec<DocumentSummary>, String)> {
    let path = cache_path()?;
    let data = std::fs::read_to_string(path).ok()?;
    let file: CacheFile = serde_json::from_str(&data).ok()?;
    Some((file.documents, file.cached_at))
}

fn cache_path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("ingenieria-tui").join("doc_cache.json"))
}
