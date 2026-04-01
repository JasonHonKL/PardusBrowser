//! Cache for HTTP resources

use bytes::Bytes;
use dashmap::DashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::debug;

/// Cached resource entry
#[derive(Debug, Clone)]
pub struct CachedResource {
    pub url: String,
    pub content: Bytes,
    pub content_type: Option<String>,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub created_at: Instant,
}

/// Resource cache with TTL
pub struct ResourceCache {
    entries: DashMap<String, Arc<CachedResource>>,
    max_size: usize,
    current_size: std::sync::atomic::AtomicUsize,
    default_ttl: Duration,
}

impl ResourceCache {
    pub fn new(max_size_bytes: usize) -> Self {
        Self {
            entries: DashMap::new(),
            max_size: max_size_bytes,
            current_size: std::sync::atomic::AtomicUsize::new(0),
            default_ttl: Duration::from_secs(300), // 5 min default
        }
    }

    /// Get cached resource
    pub fn get(&self,
        url: &str,
    ) -> Option<Arc<CachedResource>> {
        self.entries.get(url).map(|e| e.clone())
    }

    /// Insert resource
    pub fn insert(&self,
        url: &str,
        content: Bytes,
        content_type: Option<String>,
    ) {
        let size = content.len();
        
        // Check if entry already exists
        if let Some(existing) = self.entries.get(url) {
            self.current_size.fetch_sub(existing.content.len(), std::sync::atomic::Ordering::SeqCst);
        }
        
        // Ensure space
        self.ensure_space(size);
        
        let resource = Arc::new(CachedResource {
            url: url.to_string(),
            content,
            content_type,
            etag: None,
            last_modified: None,
            created_at: Instant::now(),
        });
        
        self.current_size.fetch_add(size, std::sync::atomic::Ordering::SeqCst);
        self.entries.insert(url.to_string(), resource);
        
        debug!("cached resource: {} ({} bytes)", url, size);
    }

    /// Insert with validation headers
    pub fn insert_with_validation(
        &self,
        url: &str,
        content: Bytes,
        content_type: Option<String>,
        etag: Option<String>,
        last_modified: Option<String>,
    ) {
        let size = content.len();
        self.ensure_space(size);
        
        let resource = Arc::new(CachedResource {
            url: url.to_string(),
            content,
            content_type,
            etag,
            last_modified,
            created_at: Instant::now(),
        });
        
        if let Some(existing) = self.entries.insert(url.to_string(), resource) {
            self.current_size.fetch_sub(existing.content.len(), std::sync::atomic::Ordering::SeqCst);
        }
        self.current_size.fetch_add(size, std::sync::atomic::Ordering::SeqCst);
    }

    /// Check if we need to revalidate
    pub fn needs_revalidation(&self,
        url: &str,
    ) -> bool {
        if let Some(entry) = self.entries.get(url) {
            entry.created_at.elapsed() > self.default_ttl
        } else {
            true
        }
    }

    /// Get cache key for conditional request
    pub fn get_validation_info(&self,
        url: &str,
    ) -> Option<( Option<&str>, Option<&str>)> {
        self.entries.get(url).map(|e| {
            (e.etag.as_deref(), e.last_modified.as_deref())
        })
    }

    /// Invalidate entry
    pub fn invalidate(&self,
        url: &str,
    ) {
        if let Some((_, entry)) = self.entries.remove(url) {
            self.current_size.fetch_sub(entry.content.len(), std::sync::atomic::Ordering::SeqCst);
        }
    }

    /// Clear all entries
    pub fn clear(&self) {
        self.entries.clear();
        self.current_size.store(0, std::sync::atomic::Ordering::SeqCst);
    }

    fn ensure_space(
        &self,
        _needed: usize,
    ) {
        // Simple eviction: clear expired entries
        let expired: Vec<String> = self.entries
            .iter()
            .filter(|e| e.created_at.elapsed() > self.default_ttl)
            .map(|e| e.url.clone())
            .collect();
        
        for url in expired {
            if let Some((_, entry)) = self.entries.remove(&url) {
                self.current_size.fetch_sub(entry.content.len(), std::sync::atomic::Ordering::SeqCst);
            }
        }
    }

    /// Get statistics
    pub fn stats(&self) -> CacheStats {
        CacheStats {
            entries: self.entries.len(),
            size_bytes: self.current_size.load(std::sync::atomic::Ordering::SeqCst),
            max_size: self.max_size,
        }
    }
}

/// Cache statistics
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub entries: usize,
    pub size_bytes: usize,
    pub max_size: usize,
}
