//! Generic LRU cache with TTL eviction.
//!
//! Designed for small caches (max ~100 entries). Uses a `HashMap` for O(1) lookup
//! and a `Vec` for access-order tracking (MRU at the end).

use std::collections::HashMap;
use std::hash::Hash;
use std::time::{Duration, Instant};

struct Entry<V> {
    value: V,
    last_accessed: Instant,
}

/// A synchronous LRU cache with time-to-live expiration.
///
/// - `get()` checks TTL and bumps the key to most-recently-used.
/// - `peek()` checks TTL without mutating access order (for `&self` contexts).
/// - `insert()` evicts expired entries first, then the LRU entry if at capacity.
pub struct LruCache<K, V> {
    entries: HashMap<K, Entry<V>>,
    order: Vec<K>,
    max_entries: usize,
    ttl: Duration,
}

impl<K: Hash + Eq + Clone, V: Clone> LruCache<K, V> {
    pub fn new(max_entries: usize, ttl: Duration) -> Self {
        Self {
            entries: HashMap::with_capacity(max_entries),
            order: Vec::with_capacity(max_entries),
            max_entries,
            ttl,
        }
    }

    /// Look up a key, returning its value if present and not expired.
    /// Bumps the key to most-recently-used position.
    pub fn get(&mut self, key: &K) -> Option<&V> {
        let now = Instant::now();
        let entry = self.entries.get_mut(key)?;
        if now.duration_since(entry.last_accessed) > self.ttl {
            self.entries.remove(key);
            self.order.retain(|k| k != key);
            return None;
        }
        entry.last_accessed = now;
        // Bump to MRU: remove from current position, push to end
        if let Some(pos) = self.order.iter().position(|k| k == key) {
            self.order.remove(pos);
        }
        self.order.push(key.clone());
        Some(&self.entries[key].value)
    }

    /// Look up a key without mutating access order.
    /// Useful in `&self` contexts where `get()` cannot be called.
    pub fn peek(&self, key: &K) -> Option<&V> {
        let entry = self.entries.get(key)?;
        if Instant::now().duration_since(entry.last_accessed) > self.ttl {
            return None;
        }
        Some(&entry.value)
    }

    /// Insert a key-value pair. Evicts expired entries first,
    /// then removes the LRU entry if at capacity.
    pub fn insert(&mut self, key: K, value: V) {
        self.evict_expired();
        // Update existing entry in place
        if self.entries.contains_key(&key) {
            let entry = self.entries.get_mut(&key).expect("checked contains_key");
            entry.value = value;
            entry.last_accessed = Instant::now();
            if let Some(pos) = self.order.iter().position(|k| k == &key) {
                self.order.remove(pos);
            }
            self.order.push(key);
            return;
        }
        // Evict LRU if at capacity
        if self.entries.len() >= self.max_entries {
            if let Some(lru_key) = self.order.first().cloned() {
                self.entries.remove(&lru_key);
                self.order.remove(0);
            }
        }
        self.order.push(key.clone());
        self.entries.insert(key, Entry { value, last_accessed: Instant::now() });
    }

    /// Remove a specific key. Returns `true` if it was present.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn invalidate(&mut self, key: &K) -> bool {
        if self.entries.remove(key).is_some() {
            self.order.retain(|k| k != key);
            true
        } else {
            false
        }
    }

    /// Remove all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.order.clear();
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Remove all entries whose TTL has expired. Returns count removed.
    pub fn evict_expired(&mut self) -> usize {
        let now = Instant::now();
        let ttl = self.ttl;
        let before = self.entries.len();
        self.entries.retain(|_, e| now.duration_since(e.last_accessed) <= ttl);
        let removed = before - self.entries.len();
        if removed > 0 {
            self.order.retain(|k| self.entries.contains_key(k));
        }
        removed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_and_get() {
        let mut cache: LruCache<String, i32> = LruCache::new(5, Duration::from_secs(60));
        cache.insert("a".into(), 1);
        assert_eq!(cache.get(&"a".into()), Some(&1));
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn ttl_expiry() {
        let mut cache: LruCache<String, i32> = LruCache::new(5, Duration::from_millis(1));
        cache.insert("a".into(), 1);
        std::thread::sleep(Duration::from_millis(5));
        assert_eq!(cache.get(&"a".into()), None);
    }

    #[test]
    fn peek_does_not_bump() {
        let mut cache: LruCache<String, i32> = LruCache::new(5, Duration::from_secs(60));
        cache.insert("a".into(), 1);
        cache.insert("b".into(), 2);
        // peek "a" should not change order
        assert_eq!(cache.peek(&"a".into()), Some(&1));
        // order should still be [a, b]
        assert_eq!(cache.order, vec!["a".to_string(), "b".to_string()]);
    }

    #[test]
    fn lru_eviction() {
        let mut cache: LruCache<String, i32> = LruCache::new(2, Duration::from_secs(60));
        cache.insert("a".into(), 1);
        cache.insert("b".into(), 2);
        // Access "a" to make it MRU
        cache.get(&"a".into());
        // Insert "c" should evict "b" (LRU)
        cache.insert("c".into(), 3);
        assert_eq!(cache.get(&"b".into()), None);
        assert_eq!(cache.get(&"a".into()), Some(&1));
        assert_eq!(cache.get(&"c".into()), Some(&3));
    }

    #[test]
    fn invalidate() {
        let mut cache: LruCache<String, i32> = LruCache::new(5, Duration::from_secs(60));
        cache.insert("a".into(), 1);
        assert!(cache.invalidate(&"a".into()));
        assert!(!cache.invalidate(&"a".into()));
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn update_existing_key() {
        let mut cache: LruCache<String, i32> = LruCache::new(5, Duration::from_secs(60));
        cache.insert("a".into(), 1);
        cache.insert("a".into(), 2);
        assert_eq!(cache.get(&"a".into()), Some(&2));
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn evict_expired_bulk() {
        let mut cache: LruCache<String, i32> = LruCache::new(5, Duration::from_millis(1));
        cache.insert("a".into(), 1);
        cache.insert("b".into(), 2);
        std::thread::sleep(Duration::from_millis(5));
        let removed = cache.evict_expired();
        assert_eq!(removed, 2);
        assert_eq!(cache.len(), 0);
    }
}
