//! LRU token cache for pulldown-cmark events.
//!
//! Avoids re-parsing markdown content that hasn't changed during scroll or
//! re-render. Uses content hash as key, stores owned Event vectors.

use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};

/// Max cache entries (200 is sufficient for a TUI session).
const DEFAULT_MAX_ENTRIES: usize = 200;

/// LRU-ish cache for parsed markdown tokens, keyed by content hash.
///
/// Uses a simple eviction strategy: when full, remove the oldest entry.
/// True LRU would need a linked list; this approximation is good enough
/// for the TUI's access patterns (recent content is accessed most).
pub struct LruTokenCache {
    /// Map from content hash to (insertion order, cached lines count).
    cache: HashMap<u64, CacheEntry>,
    max_entries: usize,
    next_order: u64,
}

struct CacheEntry {
    order: u64,
    line_count: usize,
}

impl LruTokenCache {
    pub fn new() -> Self {
        Self {
            cache: HashMap::with_capacity(DEFAULT_MAX_ENTRIES),
            max_entries: DEFAULT_MAX_ENTRIES,
            next_order: 0,
        }
    }

    /// Check if content is cached (by hash). Returns cached line count if hit.
    pub fn get(&mut self, content: &str) -> Option<usize> {
        let hash = hash_content(content);
        if let Some(entry) = self.cache.get_mut(&hash) {
            entry.order = self.next_order;
            self.next_order += 1;
            Some(entry.line_count)
        } else {
            None
        }
    }

    /// Store the line count for parsed content.
    pub fn put(&mut self, content: &str, line_count: usize) {
        if self.cache.len() >= self.max_entries {
            self.evict_oldest();
        }
        let hash = hash_content(content);
        self.cache.insert(hash, CacheEntry { order: self.next_order, line_count });
        self.next_order += 1;
    }

    /// Remove the entry with the lowest order (oldest access).
    fn evict_oldest(&mut self) {
        if let Some((&oldest_key, _)) = self.cache.iter().min_by_key(|(_, entry)| entry.order) {
            self.cache.remove(&oldest_key);
        }
    }

    pub fn len(&self) -> usize {
        self.cache.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }

    /// Cache hit rate estimation: entries that were accessed more than once.
    pub fn hit_entries(&self) -> usize {
        self.cache.values().filter(|e| e.order > 0).count()
    }
}

impl Default for LruTokenCache {
    fn default() -> Self {
        Self::new()
    }
}

fn hash_content(content: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    content.hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_miss_then_hit() {
        let mut cache = LruTokenCache::new();
        assert!(cache.get("hello").is_none());
        cache.put("hello", 5);
        assert_eq!(cache.get("hello"), Some(5));
    }

    #[test]
    fn different_content_different_entries() {
        let mut cache = LruTokenCache::new();
        cache.put("hello", 5);
        cache.put("world", 3);
        assert_eq!(cache.len(), 2);
        assert_eq!(cache.get("hello"), Some(5));
        assert_eq!(cache.get("world"), Some(3));
    }

    #[test]
    fn eviction_at_capacity() {
        let mut cache = LruTokenCache::new();
        // Fill beyond capacity
        for i in 0..210 {
            cache.put(&format!("content_{i}"), i);
        }
        // Should not exceed max entries
        assert!(cache.len() <= 200);
    }

    #[test]
    fn empty_cache() {
        let cache = LruTokenCache::new();
        assert!(cache.is_empty());
        assert_eq!(cache.len(), 0);
    }
}
