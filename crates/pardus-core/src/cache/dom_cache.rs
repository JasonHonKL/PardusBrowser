//! LRU cache for parsed DOMs

use crate::parser::lazy::LazyDom;
use bytes::Bytes;
use dashmap::DashMap;
use lru::LruCache;
use parking_lot::Mutex;
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{debug, trace};

/// Cache key - URL + content hash for freshness
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CacheKey {
    pub url: String,
    pub content_hash: u64,
}

impl CacheKey {
    pub fn new(url: impl Into<String>, content: &[u8]) -> Self {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let url = url.into();
        let mut hasher = DefaultHasher::new();
        content.hash(&mut hasher);
        let content_hash = hasher.finish();

        Self { url, content_hash }
    }
}

/// Cached DOM entry
#[derive(Debug, Clone)]
pub struct DomCacheEntry {
    pub key: CacheKey,
    pub dom: Arc<LazyDom>,
    pub size_bytes: usize,
    pub created_at: Instant,
    pub access_count: u64,
    pub last_accessed: Instant,
}

/// LRU DOM cache with size-based eviction
pub struct DomCache {
    /// URL -> entry mapping
    entries: DashMap<String, Arc<DomCacheEntry>>,
    /// LRU eviction list
    lru: Mutex<LruCache<String, ()>>,
    /// Current size in bytes
    current_size: std::sync::atomic::AtomicUsize,
    /// Maximum size in bytes
    max_size: usize,
    /// TTL for entries
    ttl: Duration,
}

impl DomCache {
    /// Create new cache with max size
    pub fn new(max_size_bytes: usize) -> Self {
        // Default to ~1000 entries if not limited by size
        let lru_size = NonZeroUsize::new(1000).unwrap();

        Self {
            entries: DashMap::new(),
            lru: Mutex::new(LruCache::new(lru_size)),
            current_size: std::sync::atomic::AtomicUsize::new(0),
            max_size: max_size_bytes,
            ttl: Duration::from_secs(3600),
        }
    }

    /// Get entry from cache
    pub fn get(&self, url: &str, content_hash: u64) -> Option<Arc<LazyDom>> {
        let key = format!("{}:{:x}", url, content_hash);

        if let Some(entry) = self.entries.get(&key) {
            // Check TTL
            if entry.created_at.elapsed() > self.ttl {
                trace!("cache entry expired: {}", url);
                drop(entry);
                self.remove(&key);
                return None;
            }

            // Update LRU
            self.lru.lock().put(key, ());

            // Update stats
            let entry_clone = entry.clone();
            drop(entry);

            trace!("cache hit: {}", url);
            Some(entry_clone.dom.clone())
        } else {
            trace!("cache miss: {}", url);
            None
        }
    }

    /// Insert entry into cache
    pub fn insert(&self, url: &str, content_hash: u64, dom: Arc<LazyDom>) {
        let key = format!("{}:{:x}", url, content_hash);
        let size_estimate = dom.memory_estimate();

        // Check if we need to evict
        self.ensure_space(size_estimate);

        let entry = Arc::new(DomCacheEntry {
            key: CacheKey::new(url, &[]),
            dom: dom.clone(),
            size_bytes: size_estimate,
            created_at: Instant::now(),
            access_count: 0,
            last_accessed: Instant::now(),
        });

        self.entries.insert(key.clone(), entry);
        self.lru.lock().put(key, ());

        self.current_size
            .fetch_add(size_estimate, std::sync::atomic::Ordering::SeqCst);

        debug!("cached DOM: {} ({} bytes)", url, size_estimate);
    }

    /// Insert with explicit size
    pub fn insert_with_size(&self, url: &str, content_hash: u64, dom: Arc<LazyDom>, size: usize) {
        let key = format!("{}:{:x}", url, content_hash);
        self.ensure_space(size);

        let entry = Arc::new(DomCacheEntry {
            key: CacheKey::new(url, &[]),
            dom,
            size_bytes: size,
            created_at: Instant::now(),
            access_count: 0,
            last_accessed: Instant::now(),
        });

        self.entries.insert(key.clone(), entry);
        self.lru.lock().put(key, ());

        self.current_size
            .fetch_add(size, std::sync::atomic::Ordering::SeqCst);
    }

    /// Remove entry from cache
    fn remove(&self, key: &str) {
        if let Some((_, entry)) = self.entries.remove(key) {
            self.current_size
                .fetch_sub(entry.size_bytes, std::sync::atomic::Ordering::SeqCst);
        }
        self.lru.lock().pop(key);
    }

    /// Ensure we have space for new entry
    fn ensure_space(&self, needed: usize) {
        if needed > self.max_size {
            // Entry too large for cache
            return;
        }

        let current = self.current_size.load(std::sync::atomic::Ordering::SeqCst);
        if current + needed <= self.max_size {
            return;
        }

        // Evict entries until we have space
        let mut lru = self.lru.lock();
        while self.current_size.load(std::sync::atomic::Ordering::SeqCst) + needed > self.max_size {
            if let Some((key, _)) = lru.pop_lru() {
                if let Some((_, entry)) = self.entries.remove(&key) {
                    self.current_size
                        .fetch_sub(entry.size_bytes, std::sync::atomic::Ordering::SeqCst);
                    debug!("evicted: {} ({} bytes)", entry.key.url, entry.size_bytes);
                }
            } else {
                break;
            }
        }
    }

    /// Clear all entries
    pub fn clear(&self) {
        self.entries.clear();
        self.lru.lock().clear();
        self.current_size
            .store(0, std::sync::atomic::Ordering::SeqCst);
        debug!("cache cleared");
    }

    /// Get cache statistics
    pub fn stats(&self) -> CacheStats {
        let entries = self.entries.len();
        let size = self.current_size.load(std::sync::atomic::Ordering::SeqCst);

        CacheStats {
            entries,
            size_bytes: size,
            max_size: self.max_size,
            utilization: size as f64 / self.max_size as f64,
        }
    }

    /// Set TTL
    pub fn set_ttl(&mut self, ttl: Duration) {
        self.ttl = ttl;
    }
}

/// Cache statistics
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub entries: usize,
    pub size_bytes: usize,
    pub max_size: usize,
    pub utilization: f64,
}

/// Multi-tier cache (memory + disk placeholder)
pub struct MultiTierCache {
    memory: DomCache,
}

impl MultiTierCache {
    pub fn new(max_memory: usize) -> Self {
        Self {
            memory: DomCache::new(max_memory),
        }
    }

    pub fn get(&self, url: &str, content_hash: u64) -> Option<Arc<LazyDom>> {
        // Try memory first
        self.memory.get(url, content_hash)
    }

    pub fn insert(&self, url: &str, content_hash: u64, dom: Arc<LazyDom>) {
        self.memory.insert(url, content_hash, dom);
    }
}
