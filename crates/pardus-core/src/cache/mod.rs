//! High-performance caching layer for parsed DOMs and resources

pub mod disk_cache;
pub mod dom_cache;
pub mod http_cache_policy;
pub mod resource_cache;

pub use disk_cache::{DiskCache, DiskCacheConfig};
pub use dom_cache::{CacheKey, DomCache, DomCacheEntry};
pub use http_cache_policy::CachePolicy;
pub use resource_cache::{CachedResource, ResourceCache};

use std::sync::Arc;

/// Cache configuration
#[derive(Debug, Clone)]
pub struct CacheConfig {
    /// Memory cache size in MB
    pub memory_mb: usize,
    /// Disk cache size in MB
    pub disk_mb: usize,
    /// TTL for cached entries
    pub ttl_secs: u64,
    /// Enable compression
    pub compression: bool,
    /// Enable HTTP caching (ETag, Last-Modified, conditional requests)
    pub http_cache_enabled: bool,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            memory_mb: 100,
            disk_mb: 500,
            ttl_secs: 3600,
            compression: true,
            http_cache_enabled: true,
        }
    }
}

/// Unified cache manager
pub struct CacheManager {
    dom_cache: Arc<DomCache>,
    resource_cache: Arc<ResourceCache>,
    disk_cache: Option<Arc<DiskCache>>,
    config: CacheConfig,
}

impl CacheManager {
    pub fn new(config: CacheConfig) -> anyhow::Result<Self> {
        let dom_cache = Arc::new(DomCache::new(config.memory_mb * 1024 * 1024));
        let resource_cache = Arc::new(ResourceCache::new(config.memory_mb * 1024 * 1024));

        let disk_cache = if config.disk_mb > 0 {
            Some(Arc::new(DiskCache::new(DiskCacheConfig {
                max_size: config.disk_mb * 1024 * 1024,
                ..Default::default()
            })?))
        } else {
            None
        };

        Ok(Self {
            dom_cache,
            resource_cache,
            disk_cache,
            config,
        })
    }

    pub fn dom_cache(&self) -> Arc<DomCache> {
        self.dom_cache.clone()
    }

    pub fn resource_cache(&self) -> Arc<ResourceCache> {
        self.resource_cache.clone()
    }

    pub fn disk_cache(&self) -> Option<Arc<DiskCache>> {
        self.disk_cache.clone()
    }

    pub fn http_cache_enabled(&self) -> bool {
        self.config.http_cache_enabled
    }

    /// Create a new CacheManager sharing the same internal caches.
    /// Used when constructing temporary App views from Browser.
    pub fn clone_ref(&self) -> Self {
        Self {
            dom_cache: self.dom_cache.clone(),
            resource_cache: self.resource_cache.clone(),
            disk_cache: self.disk_cache.clone(),
            config: self.config.clone(),
        }
    }

    pub fn clear_all(&self) {
        self.dom_cache.clear();
        self.resource_cache.clear();
        if let Some(ref disk) = self.disk_cache {
            let _ = disk.clear();
        }
    }

    pub fn stats(&self) -> CacheStats {
        CacheStats {
            dom: self.dom_cache.stats(),
            resource: self.resource_cache.stats(),
            disk: self.disk_cache.as_ref().map(|d| d.stats()),
        }
    }
}

/// Cache statistics
#[derive(Debug)]
pub struct CacheStats {
    pub dom: crate::cache::dom_cache::CacheStats,
    pub resource: crate::cache::resource_cache::CacheStats,
    pub disk: Option<crate::cache::disk_cache::DiskStats>,
}
