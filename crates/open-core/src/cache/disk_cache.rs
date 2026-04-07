//! Disk-based cache for persistent storage with HTTP cache semantics

use bytes::Bytes;
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};
use tracing::{debug, trace, warn};

/// Disk cache configuration
#[derive(Debug, Clone)]
pub struct DiskCacheConfig {
    pub cache_dir: PathBuf,
    pub max_size: usize,
    pub max_entries: usize,
    pub compression: bool,
    pub default_ttl_secs: u64,
}

impl Default for DiskCacheConfig {
    fn default() -> Self {
        let cache_dir = dirs::cache_dir()
            .unwrap_or_else(|| std::env::temp_dir())
            .join("open")
            .join("dom-cache");

        Self {
            cache_dir,
            max_size: 500 * 1024 * 1024,
            max_entries: 10000,
            compression: true,
            default_ttl_secs: 3600,
        }
    }
}

/// Cache entry metadata stored alongside disk data
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CacheMeta {
    pub url: String,
    pub content_type: Option<String>,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub cache_control: Option<String>,
    pub expires: Option<String>,
    pub date: Option<String>,
    pub age: Option<u64>,
    pub no_store: bool,
}

/// Disk-based persistent cache
pub struct DiskCache {
    config: DiskCacheConfig,
    index: parking_lot::Mutex<HashMap<String, CacheIndexEntry>>,
    current_size: std::sync::atomic::AtomicUsize,
}

fn default_system_time() -> SystemTime {
    SystemTime::UNIX_EPOCH
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct CacheIndexEntry {
    key: String,
    file_path: PathBuf,
    size: usize,
    #[serde(default = "default_system_time")]
    created: SystemTime,
    #[serde(default = "default_system_time")]
    accessed: SystemTime,
    #[serde(default)]
    meta: Option<CacheMeta>,
}

impl CacheIndexEntry {
    fn is_expired(&self, default_ttl: Duration) -> bool {
        if let Some(ref meta) = self.meta {
            if meta.no_store {
                return true;
            }
            if meta.cache_control.as_deref().is_some_and(|cc| {
                cc.split(',')
                    .any(|d| d.trim().eq_ignore_ascii_case("no-cache"))
            }) {
                return true;
            }
        }
        self.accessed.elapsed().unwrap_or(Duration::ZERO) > default_ttl
    }
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

    /// Get entry from disk cache, checking freshness
    pub fn get(&self, key: &str) -> Option<Bytes> {
        let index_entry = {
            let idx = self.index.lock();
            idx.get(key)?.clone()
        };

        if !index_entry.file_path.exists() {
            self.remove(key);
            return None;
        }

        let default_ttl = Duration::from_secs(self.config.default_ttl_secs);
        if index_entry.is_expired(default_ttl) {
            trace!("disk cache entry expired: {}", key);
            self.remove(key);
            return None;
        }

        match fs::read(&index_entry.file_path) {
            Ok(data) => {
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

    /// Get metadata for a cached entry
    pub fn get_meta(&self, key: &str) -> Option<CacheMeta> {
        let idx = self.index.lock();
        idx.get(key).and_then(|e| e.meta.clone())
    }

    /// Insert into disk cache with optional metadata
    pub fn insert(&self, key: &str, data: &Bytes) -> anyhow::Result<()> {
        self.insert_with_meta(key, data, None)
    }

    /// Insert into disk cache with HTTP cache metadata
    pub fn insert_with_meta(
        &self,
        key: &str,
        data: &Bytes,
        meta: Option<CacheMeta>,
    ) -> anyhow::Result<()> {
        if meta.as_ref().is_some_and(|m| m.no_store) {
            return Ok(());
        }

        let size = data.len();

        self.ensure_space(size)?;

        let file_name = format!("{}.cache", blake3::hash(key.as_bytes()));
        let file_path = self.config.cache_dir.join(&file_name);

        let mut file = File::create(&file_path)?;
        file.write_all(data)?;
        drop(file);

        let entry = CacheIndexEntry {
            key: key.to_string(),
            file_path: file_path.clone(),
            size,
            created: SystemTime::now(),
            accessed: SystemTime::now(),
            meta,
        };

        {
            let mut idx = self.index.lock();
            if let Some(existing) = idx.insert(key.to_string(), entry) {
                self.current_size
                    .fetch_sub(existing.size, std::sync::atomic::Ordering::SeqCst);
            }
        }

        self.current_size
            .fetch_add(size, std::sync::atomic::Ordering::SeqCst);

        if self.index.lock().len() % 100 == 0 {
            if let Err(e) = self.save_index() {
                warn!("failed to persist disk cache index: {}", e);
            }
        }

        debug!("wrote to disk cache: {} ({} bytes)", key, size);
        Ok(())
    }

    /// Remove entry from cache
    pub fn remove(&self, key: &str) {
        let entry = {
            let mut idx = self.index.lock();
            idx.remove(key)
        };

        if let Some(entry) = entry {
            let _ = fs::remove_file(&entry.file_path);
            self.current_size
                .fetch_sub(entry.size, std::sync::atomic::Ordering::SeqCst);
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
        self.current_size
            .store(0, std::sync::atomic::Ordering::SeqCst);

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
    fn ensure_space(&self, needed: usize) -> anyhow::Result<()> {
        if needed > self.config.max_size {
            return Err(anyhow::anyhow!("entry too large for cache"));
        }

        let current = self.current_size.load(std::sync::atomic::Ordering::SeqCst);
        if current + needed <= self.config.max_size {
            return Ok(());
        }

        let default_ttl = Duration::from_secs(self.config.default_ttl_secs);
        let mut idx = self.index.lock();

        // First evict expired/no-store entries
        let expired_keys: Vec<String> = idx
            .iter()
            .filter(|(_, e)| e.is_expired(default_ttl))
            .map(|(k, _)| k.clone())
            .collect();

        let mut freed = 0usize;
        for key in &expired_keys {
            if current - freed + needed <= self.config.max_size {
                break;
            }
            if let Some(entry) = idx.remove(key) {
                let _ = fs::remove_file(&entry.file_path);
                freed += entry.size;
            }
        }

        // Also enforce max_entries limit
        if idx.len() > self.config.max_entries {
            let excess = idx.len() - self.config.max_entries;
            let mut lru_entries: Vec<_> = idx.values().collect();
            lru_entries.sort_by_key(|e| e.accessed);
            let keys_to_remove: Vec<String> = lru_entries
                .iter()
                .take(excess)
                .map(|e| e.key.clone())
                .collect();
            for key in &keys_to_remove {
                if let Some(entry) = idx.get(key) {
                    let _ = fs::remove_file(&entry.file_path);
                    freed += entry.size;
                }
                idx.remove(key);
            }
        }

        if current - freed + needed <= self.config.max_size {
            drop(idx);
            self.current_size
                .fetch_sub(freed, std::sync::atomic::Ordering::SeqCst);
            return Ok(());
        }

        // Then evict LRU entries
        let mut entries: Vec<_> = idx.values().collect();
        entries.sort_by_key(|e| e.accessed);

        // Collect keys to remove
        let keys_to_remove: Vec<_> = entries
            .iter()
            .take_while(|_| current - freed + needed > self.config.max_size)
            .map(|e| (e.key.clone(), e.file_path.clone(), e.size))
            .collect();

        drop(entries);

        for (key, file_path, size) in keys_to_remove {
            idx.remove(&key);
            let _ = fs::remove_file(&file_path);
            freed += size;
        }

        drop(idx);
        self.current_size
            .fetch_sub(freed, std::sync::atomic::Ordering::SeqCst);

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

        self.current_size
            .store(total_size, std::sync::atomic::Ordering::SeqCst);

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
