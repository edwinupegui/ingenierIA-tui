//! In-memory cache layer with TTL-based eviction.
//!
//! Each cache uses `LruCache` for automatic size + TTL management.
//! The `CacheLayer` groups all application caches in one place.

use std::time::Duration;

use crate::domain::document::{DocumentDetail, DocumentSummary};
use crate::domain::search::SearchResultItem;
use crate::services::cache::LruCache;

pub struct CacheLayer {
    /// Document list cache. Key: `"all"`. TTL: 5 min.
    pub documents: LruCache<String, Vec<DocumentSummary>>,
    /// Individual document detail cache. Key: URI string. TTL: 5 min.
    pub doc_details: LruCache<String, DocumentDetail>,
    /// Server search results cache. Key: `"{query}\0{factory}"`. TTL: 1 min.
    pub search: LruCache<String, Vec<SearchResultItem>>,
    /// MCP tool schema cache. Key: tool name. TTL: 30 min.
    pub tool_schemas: LruCache<String, serde_json::Value>,
}

impl CacheLayer {
    pub fn new() -> Self {
        Self {
            documents: LruCache::new(1, Duration::from_secs(300)),
            doc_details: LruCache::new(20, Duration::from_secs(300)),
            search: LruCache::new(20, Duration::from_secs(60)),
            tool_schemas: LruCache::new(100, Duration::from_secs(1800)),
        }
    }

    /// Invalidate all document caches (on SSE sync/reload events).
    pub fn invalidate_documents(&mut self) {
        self.documents.clear();
        self.doc_details.clear();
    }

    /// Evict expired entries from all caches.
    pub fn evict_all_expired(&mut self) {
        self.documents.evict_expired();
        self.doc_details.evict_expired();
        self.search.evict_expired();
        self.tool_schemas.evict_expired();
    }
}
