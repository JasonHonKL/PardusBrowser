//! High-performance caching layer for parsed DOMs and resources

pub mod dom_cache;
pub mod resource_cache;
pub mod disk_cache;

pub use dom_cache::{DomCache, DomCacheEntry, CacheKey};
pub use resource_cache::{ResourceCache, CachedResource};
pub use disk_cache::{DiskCache, DiskCacheConfig};

use std::sync::Arc;
use std::time::Duration;

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
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            memory_mb: 100,
            disk_mb: 500,
            ttl_secs: 3600, // 1 hour
            compression: true,
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

    /// Get DOM cache
    pub fn dom_cache(&self) -> Arc<DomCache> {
        self.dom_cache.clone()
    }

    /// Get resource cache
    pub fn resource_cache(&self) -> Arc<ResourceCache> {
        self.resource_cache.clone()
    }

    /// Get disk cache
    pub fn disk_cache(&self) -> Option<Arc<DiskCache>> {
        self.disk_cache.clone()
    }

    /// Clear all caches
    pub fn clear_all(&self) {
        self.dom_cache.clear();
        self.resource_cache.clear();
        if let Some(ref disk) = self.disk_cache {
            let _ = disk.clear();
        }
    }

    /// Get cache statistics
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
