//! Disk-based cache for persistent storage

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use blake3::Hash;
use bytes::Bytes;
use tracing::{trace, debug, warn};

/// Disk cache configuration
#[derive(Debug, Clone)]
pub struct DiskCacheConfig {
    pub cache_dir: PathBuf,
    pub max_size: usize,      // bytes
    pub max_entries: usize,
    pub compression: bool,
}

impl Default for DiskCacheConfig {
    fn default() -> Self {
        let cache_dir = dirs::cache_dir()
            .unwrap_or_else(|| std::env::temp_dir())
            .join("pardus")
            .join("dom-cache");
        
        Self {
            cache_dir,
            max_size: 500 * 1024 * 1024, // 500MB
            max_entries: 10000,
            compression: true,
        }
    }
}

/// Disk-based persistent cache
pub struct DiskCache {
    config: DiskCacheConfig,
    /// Index of cached entries
    index: parking_lot::Mutex<HashMap<String, CacheIndexEntry>>,
    current_size: std::sync::atomic::AtomicUsize,
}

/// Index entry for cache metadata
#[derive(Debug, Clone)]
struct CacheIndexEntry {
    key: String,
    file_path: PathBuf,
    size: usize,
    created: SystemTime,
    accessed: SystemTime,
}

impl DiskCache {
    /// Create new disk cache
    pub fn new(config: DiskCacheConfig) -> anyhow::Result<Self> {
        fs::create_dir_all(&config.cache_dir)?;
        
        let cache = Self {
            config,
            index: parking_lot::Mutex::new(HashMap::new()),
            current_size: std::sync::atomic::AtomicUsize::new(0),
        };
        
        // Load existing index
        cache.load_index()?;
        
        Ok(cache)
    }

    /// Get entry from disk cache
    pub fn get(&self,
        key: &str,
    ) -> Option<Bytes> {
        let index_entry = {
            let idx = self.index.lock();
            idx.get(key)?.clone()
        };
        
        // Check if file exists
        if !index_entry.file_path.exists() {
            self.remove(key);
            return None;
        }
        
        // Read file
        match fs::read(&index_entry.file_path) {
            Ok(data) => {
                // Update access time
                let mut idx = self.index.lock();
                if let Some(entry) = idx.get_mut(key) {
                    entry.accessed = SystemTime::now();
                }
                
                trace!("disk cache hit: {}", key);
                Some(Bytes::from(data))
            }
            Err(e) => {
                warn!("failed to read cache file: {}", e);
                self.remove(key);
                None
            }
        }
    }

    /// Insert into disk cache
    pub fn insert(&self,
        key: &str,
        data: &Bytes,
    ) -> anyhow::Result<()> {
        let size = data.len();
        
        // Check if we need to evict
        self.ensure_space(size)?;
        
        // Compute file path
        let file_name = format!("{}.cache", blake3::hash(key.as_bytes()));
        let file_path = self.config.cache_dir.join(&file_name);
        
        // Write data
        let mut file = File::create(&file_path)?;
        file.write_all(data)?;
        drop(file);
        
        // Update index
        let entry = CacheIndexEntry {
            key: key.to_string(),
            file_path: file_path.clone(),
            size,
            created: SystemTime::now(),
            accessed: SystemTime::now(),
        };
        
        {
            let mut idx = self.index.lock();
            if let Some(existing) = idx.insert(key.to_string(), entry) {
                self.current_size.fetch_sub(existing.size, std::sync::atomic::Ordering::SeqCst);
            }
        }
        
        self.current_size.fetch_add(size, std::sync::atomic::Ordering::SeqCst);
        
        // Save index periodically
        if self.index.lock().len() % 100 == 0 {
            let _ = self.save_index();
        }
        
        debug!("wrote to disk cache: {} ({} bytes)", key, size);
        Ok(())
    }

    /// Remove entry from cache
    pub fn remove(&self,
        key: &str,
    ) {
        let entry = {
            let mut idx = self.index.lock();
            idx.remove(key)
        };
        
        if let Some(entry) = entry {
            let _ = fs::remove_file(&entry.file_path);
            self.current_size.fetch_sub(entry.size, std::sync::atomic::Ordering::SeqCst);
        }
    }

    /// Clear all entries
    pub fn clear(&self) -> anyhow::Result<()> {
        // Remove all files
        for entry in fs::read_dir(&self.config.cache_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().map(|e| e == "cache").unwrap_or(false) {
                let _ = fs::remove_file(path);
            }
        }
        
        // Clear index
        self.index.lock().clear();
        self.current_size.store(0, std::sync::atomic::Ordering::SeqCst);
        
        // Remove index file
        let index_path = self.config.cache_dir.join("index.json");
        let _ = fs::remove_file(index_path);
        
        Ok(())
    }

    /// Get cache statistics
    pub fn stats(&self) -> DiskStats {
        let idx = self.index.lock();
        DiskStats {
            entries: idx.len(),
            size_bytes: self.current_size.load(std::sync::atomic::Ordering::SeqCst),
            max_size: self.config.max_size,
            max_entries: self.config.max_entries,
        }
    }

    /// Ensure we have space
    fn ensure_space(&self,
        needed: usize,
    ) -> anyhow::Result<()> {
        if needed > self.config.max_size {
            return Err(anyhow::anyhow!("entry too large for cache"));
        }
        
        let current = self.current_size.load(std::sync::atomic::Ordering::SeqCst);
        if current + needed <= self.config.max_size {
            return Ok(());
        }
        
        // Evict oldest entries
        let mut idx = self.index.lock();
        
        // Sort by access time (oldest first)
        let mut entries: Vec<_> = idx.values().collect();
        entries.sort_by_key(|e| e.accessed);
        
        let mut freed = 0usize;
        for entry in entries {
            if current - freed + needed <= self.config.max_size {
                break;
            }
            
            idx.remove(&entry.key);
            let _ = fs::remove_file(&entry.file_path);
            freed += entry.size;
        }
        
        self.current_size.fetch_sub(freed, std::sync::atomic::Ordering::SeqCst);
        
        Ok(())
    }

    /// Load index from disk
    fn load_index(&self) -> anyhow::Result<()> {
        let index_path = self.config.cache_dir.join("index.json");
        
        if !index_path.exists() {
            return Ok(());
        }
        
        let data = fs::read_to_string(&index_path)?;
        let entries: Vec<CacheIndexEntry> = serde_json::from_str(&data)?;
        
        let mut idx = self.index.lock();
        let mut total_size = 0usize;
        
        for entry in entries {
            if entry.file_path.exists() {
                total_size += entry.size;
                idx.insert(entry.key.clone(), entry);
            }
        }
        
        self.current_size.store(total_size, std::sync::atomic::Ordering::SeqCst);
        
        debug!("loaded disk cache index: {} entries", idx.len());
        Ok(())
    }

    /// Save index to disk
    fn save_index(&self) -> anyhow::Result<()> {
        let index_path = self.config.cache_dir.join("index.json");
        
        let idx = self.index.lock();
        let entries: Vec<_> = idx.values().cloned().collect();
        
        let json = serde_json::to_string_pretty(&entries)?;
        fs::write(&index_path, json)?;
        
        Ok(())
    }
}

/// Disk cache statistics
#[derive(Debug, Clone)]
pub struct DiskStats {
    pub entries: usize,
    pub size_bytes: usize,
    pub max_size: usize,
    pub max_entries: usize,
}
