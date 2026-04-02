//! Short-lived in-memory cache for HTTP/2 push simulation results.
//!
//! [`PushCache`] stores pre-fetched resource data keyed by URL with a short
//! TTL (default 30 seconds). It is designed for same-navigation deduplication:
//! resources discovered during early `<head>` scanning are fetched speculatively
//! and stored here so that later explicit fetches (e.g., via
//! `fetch_subresources`) can skip the HTTP round-trip.

use bytes::Bytes;
use dashmap::DashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};
use tracing::{debug, trace};

/// A single pre-fetched resource entry.
#[derive(Debug, Clone)]
pub struct PushEntry {
    pub url: String,
    pub status: u16,
    pub body: Bytes,
    pub content_type: Option<String>,
    pub duration_ms: u64,
    pub created_at: Instant,
    pub source: PushSource,
}

/// Where the entry came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PushSource {
    EarlyScan,
    H2PushPromise,
    Preload,
}

impl PushEntry {
    pub fn is_success(&self) -> bool {
        self.status >= 200 && self.status < 300
    }

    pub fn is_expired(&self, ttl: Duration) -> bool {
        self.created_at.elapsed() > ttl
    }
}

/// Short-lived cache for pre-fetched subresources.
///
/// Entries are evicted by TTL. The cache is bounded by `max_entries` — when the
/// limit is reached, expired entries are purged first, then the oldest entry is
/// removed.
pub struct PushCache {
    entries: DashMap<String, PushEntry>,
    max_entries: usize,
    ttl: Duration,
    hits: AtomicUsize,
    misses: AtomicUsize,
}

impl PushCache {
    pub fn new(max_entries: usize, ttl_secs: u64) -> Self {
        Self {
            entries: DashMap::new(),
            max_entries,
            ttl: Duration::from_secs(ttl_secs),
            hits: AtomicUsize::new(0),
            misses: AtomicUsize::new(0),
        }
    }

    /// Insert a pre-fetched resource into the cache.
    ///
    /// If the URL already exists, the entry is replaced. If the cache is at
    /// capacity, expired entries are evicted first, then the oldest entry.
    pub fn insert(&self, entry: PushEntry) {
        if self.entries.contains_key(&entry.url) {
            trace!("push cache: replacing existing entry for {}", entry.url);
        } else {
            self.ensure_space();
        }
        debug!(
            "push cache: inserted {} ({} bytes, {:?})",
            entry.url,
            entry.body.len(),
            entry.source,
        );
        self.entries.insert(entry.url.clone(), entry);
    }

    /// Insert a successful fetch result.
    pub fn insert_success(
        &self,
        url: String,
        status: u16,
        body: Bytes,
        content_type: Option<String>,
        duration_ms: u64,
        source: PushSource,
    ) {
        self.insert(PushEntry {
            url,
            status,
            body,
            content_type,
            duration_ms,
            created_at: Instant::now(),
            source,
        });
    }

    /// Get a cached entry if it exists and has not expired.
    pub fn get(&self, url: &str) -> Option<PushEntry> {
        match self.entries.get(url) {
            Some(entry) if !entry.is_expired(self.ttl) => {
                self.hits.fetch_add(1, Ordering::Relaxed);
                trace!("push cache hit: {}", url);
                Some(entry.clone())
            }
            Some(_) => {
                trace!("push cache: expired entry for {}", url);
                self.misses.fetch_add(1, Ordering::Relaxed);
                None
            }
            None => {
                self.misses.fetch_add(1, Ordering::Relaxed);
                None
            }
        }
    }

    /// Check whether the cache contains a valid (non-expired) entry.
    pub fn contains(&self, url: &str) -> bool {
        self.get(url).is_some()
    }

    /// Remove an entry.
    pub fn remove(&self, url: &str) {
        self.entries.remove(url);
    }

    /// Clear all entries and reset stats.
    pub fn clear(&self) {
        self.entries.clear();
        self.hits.store(0, Ordering::Relaxed);
        self.misses.store(0, Ordering::Relaxed);
    }

    /// Evict expired entries.
    pub fn evict_expired(&self) -> usize {
        let expired: Vec<String> = self
            .entries
            .iter()
            .filter(|e| e.is_expired(self.ttl))
            .map(|e| e.url.clone())
            .collect();
        let count = expired.len();
        for url in expired {
            self.entries.remove(&url);
        }
        count
    }

    /// Number of entries currently in the cache (including potentially expired).
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Collect all cached URLs (for debugging/logging).
    pub fn cached_urls(&self) -> Vec<String> {
        self.entries.iter().map(|e| e.key().clone()).collect()
    }

    /// Get cache statistics.
    pub fn stats(&self) -> PushCacheStats {
        PushCacheStats {
            entries: self.entries.len(),
            max_entries: self.max_entries,
            hits: self.hits.load(Ordering::Relaxed),
            misses: self.misses.load(Ordering::Relaxed),
        }
    }

    fn ensure_space(&self) {
        if self.entries.len() < self.max_entries {
            return;
        }
        let evicted = self.evict_expired();
        if evicted > 0 || self.entries.len() < self.max_entries {
            return;
        }
        // Remove the oldest entry (first one inserted, approximate)
        if let Some(oldest) = self
            .entries
            .iter()
            .min_by_key(|e| e.created_at)
            .map(|e| e.key().clone())
        {
            debug!("push cache: evicting oldest entry {}", oldest);
            self.entries.remove(&oldest);
        }
    }
}

impl Default for PushCache {
    fn default() -> Self {
        Self::new(32, 30)
    }
}

/// Cache statistics snapshot.
#[derive(Debug, Clone)]
pub struct PushCacheStats {
    pub entries: usize,
    pub max_entries: usize,
    pub hits: usize,
    pub misses: usize,
}

impl PushCacheStats {
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            self.hits as f64 / total as f64
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_cache(max: usize, ttl: u64) -> PushCache {
        PushCache::new(max, ttl)
    }

    fn make_entry(url: &str) -> PushEntry {
        PushEntry {
            url: url.to_string(),
            status: 200,
            body: Bytes::from_static(b"hello"),
            content_type: Some("text/plain".to_string()),
            duration_ms: 10,
            created_at: Instant::now(),
            source: PushSource::EarlyScan,
        }
    }

    #[test]
    fn test_insert_and_get() {
        let cache = make_cache(10, 30);
        assert!(cache.get("https://example.com/a.css").is_none());
        cache.insert(make_entry("https://example.com/a.css"));
        let entry = cache.get("https://example.com/a.css").unwrap();
        assert_eq!(entry.status, 200);
        assert_eq!(entry.body.len(), 5);
    }

    #[test]
    fn test_cache_hit_miss_stats() {
        let cache = make_cache(10, 30);
        cache.insert(make_entry("https://example.com/a.css"));

        assert!(cache.get("https://example.com/a.css").is_some());
        assert!(cache.get("https://example.com/b.css").is_none());
        assert!(cache.get("https://example.com/b.css").is_none());

        let stats = cache.stats();
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.misses, 2);
    }

    #[test]
    fn test_max_entries_eviction() {
        let cache = make_cache(2, 30);
        cache.insert(make_entry("https://example.com/a.css"));
        cache.insert(make_entry("https://example.com/b.js"));
        cache.insert(make_entry("https://example.com/c.png"));

        // Should have evicted the oldest (a.css)
        assert!(cache.get("https://example.com/a.css").is_none());
        assert!(cache.get("https://example.com/b.js").is_some());
        assert!(cache.get("https://example.com/c.png").is_some());
    }

    #[test]
    fn test_clear() {
        let cache = make_cache(10, 30);
        cache.insert(make_entry("https://example.com/a.css"));
        cache.clear();
        assert!(cache.is_empty());
        assert_eq!(cache.stats().hits, 0);
    }

    #[test]
    fn test_replace_existing() {
        let cache = make_cache(10, 30);
        cache.insert(make_entry("https://example.com/a.css"));
        let mut entry = make_entry("https://example.com/a.css");
        entry.body = Bytes::from_static(b"updated");
        cache.insert(entry);
        let got = cache.get("https://example.com/a.css").unwrap();
        assert_eq!(got.body.len(), 7);
    }

    #[test]
    fn test_contains() {
        let cache = make_cache(10, 30);
        cache.insert(make_entry("https://example.com/a.css"));
        assert!(cache.contains("https://example.com/a.css"));
        assert!(!cache.contains("https://example.com/b.css"));
    }

    #[test]
    fn test_remove() {
        let cache = make_cache(10, 30);
        cache.insert(make_entry("https://example.com/a.css"));
        cache.remove("https://example.com/a.css");
        assert!(!cache.contains("https://example.com/a.css"));
    }

    #[test]
    fn test_evict_expired() {
        // Very short TTL for testing
        let cache = make_cache(10, 0);
        cache.insert(make_entry("https://example.com/a.css"));
        std::thread::sleep(Duration::from_millis(10));
        assert!(cache.get("https://example.com/a.css").is_none());
        let evicted = cache.evict_expired();
        assert_eq!(evicted, 1);
        assert!(cache.is_empty());
    }

    #[test]
    fn test_insert_success_convenience() {
        let cache = make_cache(10, 30);
        cache.insert_success(
            "https://example.com/style.css".to_string(),
            200,
            Bytes::from_static(b"body{}"),
            Some("text/css".to_string()),
            5,
            PushSource::Preload,
        );
        let entry = cache.get("https://example.com/style.css").unwrap();
        assert_eq!(entry.source, PushSource::Preload);
        assert_eq!(entry.content_type.as_deref(), Some("text/css"));
    }

    #[test]
    fn test_hit_rate() {
        let cache = make_cache(10, 30);
        let stats = cache.stats();
        assert_eq!(stats.hit_rate(), 0.0);
        cache.insert(make_entry("https://example.com/a.css"));
        cache.get("https://example.com/a.css"); // hit
        cache.get("https://example.com/x.css"); // miss
        let stats = cache.stats();
        assert!((stats.hit_rate() - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_cached_urls() {
        let cache = make_cache(10, 30);
        cache.insert(make_entry("https://example.com/a.css"));
        cache.insert(make_entry("https://example.com/b.js"));
        let urls = cache.cached_urls();
        assert_eq!(urls.len(), 2);
    }

    #[test]
    fn test_default() {
        let cache = PushCache::default();
        assert_eq!(cache.stats().max_entries, 32);
    }
}
