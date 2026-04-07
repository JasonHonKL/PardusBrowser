//! Compiled JavaScript module cache

use dashmap::DashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::time::Instant;
use tracing::{trace, debug};

/// Compiled JavaScript module
#[derive(Debug, Clone)]
pub struct CompiledModule {
    pub hash: u64,
    pub bytecode: Arc<[u8]>,
    pub compiled_at: Instant,
    pub source_len: usize,
}

/// Cache for compiled JS modules
pub struct ModuleCache {
    modules: DashMap<u64, CompiledModule>,
    max_size: usize,
    current_size: std::sync::atomic::AtomicUsize,
}

impl ModuleCache {
    pub fn new() -> Self {
        Self {
            modules: DashMap::new(),
            max_size: 100 * 1024 * 1024, // 100MB
            current_size: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    pub fn with_size(max_size: usize) -> Self {
        Self {
            modules: DashMap::new(),
            max_size,
            current_size: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    /// Get compiled module by source code
    pub fn get(&self,
        source: &str,
    ) -> Option<CompiledModule> {
        let hash = Self::hash_source(source);
        
        self.modules.get(&hash).map(|m| {
            trace!("module cache hit: hash={:x}", hash);
            m.clone()
        })
    }

    /// Insert compiled module
    pub fn insert(&self,
        source: &str,
        _result: &super::JsResult, // Would contain bytecode in real impl
    ) {
        let hash = Self::hash_source(source);
        let size = source.len(); // Estimate
        
        // Check if we need to evict
        self.ensure_space(size);
        
        let module = CompiledModule {
            hash,
            bytecode: Arc::new(vec![]), // Placeholder
            compiled_at: Instant::now(),
            source_len: source.len(),
        };
        
        if self.modules.insert(hash, module).is_none() {
            self.current_size.fetch_add(size, std::sync::atomic::Ordering::SeqCst);
        }
        
        trace!("cached module: hash={:x}, {} bytes", hash, size);
    }

    /// Insert with pre-compiled bytecode
    pub fn insert_compiled(&self,
        source: &str,
        bytecode: Arc<[u8]>,
    ) {
        let hash = Self::hash_source(source);
        let size = bytecode.len();
        
        self.ensure_space(size);
        
        let module = CompiledModule {
            hash,
            bytecode,
            compiled_at: Instant::now(),
            source_len: source.len(),
        };
        
        if self.modules.insert(hash, module).is_none() {
            self.current_size.fetch_add(size, std::sync::atomic::Ordering::SeqCst);
        }
    }

    /// Check if module is cached
    pub fn contains(&self,
        source: &str,
    ) -> bool {
        let hash = Self::hash_source(source);
        self.modules.contains_key(&hash)
    }

    /// Remove module from cache
    pub fn invalidate(&self,
        source: &str,
    ) {
        let hash = Self::hash_source(source);
        if let Some((_, module)) = self.modules.remove(&hash) {
            self.current_size.fetch_sub(module.bytecode.len(), std::sync::atomic::Ordering::SeqCst);
        }
    }

    /// Clear all modules
    pub fn clear(&self) {
        self.modules.clear();
        self.current_size.store(0, std::sync::atomic::Ordering::SeqCst);
    }

    fn hash_source(source: &str) -> u64 {
        let mut hasher = DefaultHasher::new();
        source.hash(&mut hasher);
        hasher.finish()
    }

    fn ensure_space(&self,
        needed: usize,
    ) {
        let current = self.current_size.load(std::sync::atomic::Ordering::SeqCst);
        if current + needed <= self.max_size {
            return;
        }

        // Simple LRU eviction - remove oldest entries
        let mut entries: Vec<_> = self.modules.iter().map(|e| {
            (*e.key(), e.value().compiled_at)
        }).collect();
        
        entries.sort_by_key(|(_, t)| *t);
        
        let mut freed = 0usize;
        for (hash, _) in entries {
            if current - freed + needed <= self.max_size {
                break;
            }
            if let Some((_, module)) = self.modules.remove(&hash) {
                freed += module.bytecode.len();
            }
        }

        self.current_size.fetch_sub(freed, std::sync::atomic::Ordering::SeqCst);
    }

    /// Cache statistics
    pub fn stats(&self) -> ModuleCacheStats {
        ModuleCacheStats {
            entries: self.modules.len(),
            size_bytes: self.current_size.load(std::sync::atomic::Ordering::SeqCst),
            max_size: self.max_size,
        }
    }
}

/// Cache statistics
#[derive(Debug, Clone)]
pub struct ModuleCacheStats {
    pub entries: usize,
    pub size_bytes: usize,
    pub max_size: usize,
}

/// Persistent module cache (disk-backed)
pub struct PersistentModuleCache {
    memory: ModuleCache,
    cache_dir: std::path::PathBuf,
}

impl PersistentModuleCache {
    pub fn new(cache_dir: std::path::PathBuf) -> anyhow::Result<Self> {
        std::fs::create_dir_all(&cache_dir)?;
        
        let memory = ModuleCache::with_size(50 * 1024 * 1024); // 50MB in memory
        
        Ok(Self {
            memory,
            cache_dir,
        })
    }

    pub fn get(&self,
        source: &str,
    ) -> Option<CompiledModule> {
        // Check memory first
        if let Some(module) = self.memory.get(source) {
            return Some(module);
        }
        
        // Check disk
        let hash = ModuleCache::hash_source(source);
        let path = self.cache_dir.join(format!("{:x}.module", hash));
        
        if let Ok(data) = std::fs::read(&path) {
            // Load into memory and return
            // ...
        }
        
        None
    }
}
