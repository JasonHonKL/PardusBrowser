//! V8 snapshot management for fast startup

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use parking_lot::RwLock;
use tracing::{trace, debug, warn};
use bytes::Bytes;

/// Serialized V8 snapshot data
#[derive(Debug, Clone)]
pub struct SnapshotData {
    pub data: Arc<[u8]>,
    pub hash: u64,
    pub created_at: std::time::Instant,
    pub modules_included: Vec<String>,
}

impl SnapshotData {
    pub fn new(data: Arc<[u8]>, modules: Vec<String>) -> Self {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::Hasher;
        
        let mut hasher = DefaultHasher::new();
        hasher.write(&data);
        let hash = hasher.finish();
        
        Self {
            data,
            hash,
            created_at: std::time::Instant::now(),
            modules_included: modules,
        }
    }

    pub fn size(&self) -> usize {
        self.data.len()
    }
}

/// Snapshot manager for creating and caching V8 snapshots
pub struct SnapshotManager {
    /// Cache of created snapshots
    snapshots: RwLock<HashMap<String, Arc<SnapshotData>>>,
    /// Base snapshot (empty isolate)
    base_snapshot: RwLock<Option<Arc<SnapshotData>>>,
    /// Max snapshots to keep in memory
    max_snapshots: usize,
}

impl SnapshotManager {
    pub fn new() -> anyhow::Result<Self> {
        Ok(Self {
            snapshots: RwLock::new(HashMap::new()),
            base_snapshot: RwLock::new(None),
            max_snapshots: 10,
        })
    }

    /// Create a snapshot from a script
    pub async fn create(&self,
        name: &str,
        script: &str,
    ) -> anyhow::Result<Arc<SnapshotData>> {
        trace!("creating snapshot: {}", name);
        
        // In real implementation, this would use V8's snapshot API
        // For now, create a placeholder
        let data: Vec<u8> = script.as_bytes().to_vec();
        let snapshot = Arc::new(SnapshotData::new(
            Arc::from(data.into_boxed_slice()),
            vec![name.to_string()],
        ));
        
        // Store in cache
        {
            let mut snaps = self.snapshots.write();
            snaps.insert(name.to_string(), snapshot.clone());
            
            // Evict old snapshots if needed
            if snaps.len() > self.max_snapshots {
                let oldest = snaps.keys().next().cloned();
                if let Some(key) = oldest {
                    snaps.remove(&key);
                }
            }
        }
        
        debug!("created snapshot {} ({} bytes)", name, snapshot.size());
        Ok(snapshot)
    }

    /// Get cached snapshot
    pub fn get(&self,
        name: &str,
    ) -> Option<Arc<SnapshotData>> {
        self.snapshots.read().get(name).cloned()
    }

    /// Get or create base snapshot
    pub async fn get_base(&self,
    ) -> anyhow::Result<Arc<SnapshotData>> {
        // Check if we already have a base snapshot
        if let Some(base) = self.base_snapshot.read().as_ref() {
            return Ok(base.clone());
        }
        
        // Create base snapshot with common polyfills
        let base_script = r#"console.log('base snapshot');"#;
        
        let snapshot = self.create("base", base_script).await?;
        
        *self.base_snapshot.write() = Some(snapshot.clone());
        
        Ok(snapshot)
    }

    /// Create incremental snapshot from base
    pub async fn create_incremental(&self,
        name: &str,
        base: &SnapshotData,
        additional_script: &str,
    ) -> anyhow::Result<Arc<SnapshotData>> {
        trace!("creating incremental snapshot: {}", name);
        
        // Combine base + additional
        let combined = format!("// Base\n// Additional\n{}", additional_script);
        self.create(name, &combined).await
    }

    /// Persist snapshot to disk
    pub fn persist(&self,
        name: &str,
        path: &Path,
    ) -> anyhow::Result<()> {
        let snapshot = self.snapshots.read()
            .get(name)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("snapshot not found"))?;
        
        std::fs::write(path, &*snapshot.data)?;
        debug!("persisted snapshot {} to {:?}", name, path);
        
        Ok(())
    }

    /// Load snapshot from disk
    pub fn load(&self,
        name: &str,
        path: &Path,
    ) -> anyhow::Result<()> {
        let data = std::fs::read(path)?;
        let snapshot = Arc::new(SnapshotData::new(
            Arc::from(data.into_boxed_slice()),
            vec![name.to_string()],
        ));
        
        self.snapshots.write().insert(name.to_string(), snapshot);
        debug!("loaded snapshot {} from {:?}", name, path);
        
        Ok(())
    }

    /// Invalidate a snapshot
    pub fn invalidate(&self,
        name: &str,
    ) {
        self.snapshots.write().remove(name);
        trace!("invalidated snapshot: {}", name);
    }

    /// Clear all snapshots
    pub fn clear(&self) {
        self.snapshots.write().clear();
        *self.base_snapshot.write() = None;
    }

    /// Get snapshot statistics
    pub fn stats(&self) -> SnapshotStats {
        let snaps = self.snapshots.read();
        let total_size = snaps.values().map(|s| s.size()).sum();
        
        SnapshotStats {
            count: snaps.len(),
            total_size_bytes: total_size,
            max_snapshots: self.max_snapshots,
        }
    }
}

/// Snapshot statistics
#[derive(Debug, Clone)]
pub struct SnapshotStats {
    pub count: usize,
    pub total_size_bytes: usize,
    pub max_snapshots: usize,
}

/// Snapshot builder for creating complex snapshots
pub struct SnapshotBuilder {
    scripts: Vec<String>,
    modules: Vec<String>,
}

impl SnapshotBuilder {
    pub fn new() -> Self {
        Self {
            scripts: Vec::new(),
            modules: Vec::new(),
        }
    }

    pub fn add_script(&mut self,
        script: &str,
    ) -> &mut Self {
        self.scripts.push(script.to_string());
        self
    }

    pub fn add_module(&mut self,
        name: &str,
        source: &str,
    ) -> &mut Self {
        self.modules.push(format!("{}\n{}", name, source));
        self
    }

    pub fn build(&self) -> String {
        self.scripts.join("\n")
    }
}

impl Default for SnapshotBuilder {
    fn default() -> Self {
        Self::new()
    }
}
