//! V8 Isolate pool for fast JS execution

use std::collections::VecDeque;
use std::sync::Arc;
use parking_lot::Mutex;
use tokio::sync::{Semaphore, OwnedSemaphorePermit};
use tracing::{trace, debug};

/// Configuration for isolates in the pool
#[derive(Debug, Clone)]
pub struct IsolateConfig {
    pub initial_heap_size: usize,
    pub max_heap_size: usize,
    pub warmup_script: Option<String>,
}

impl Default for IsolateConfig {
    fn default() -> Self {
        Self {
            initial_heap_size: 8 * 1024 * 1024,    // 8MB
            max_heap_size: 128 * 1024 * 1024,      // 128MB
            warmup_script: None,
        }
    }
}

/// A pooled V8 isolate
pub struct PooledIsolate {
    pub id: usize,
    pub created_at: std::time::Instant,
    pub executions: u64,
    // Actual V8 isolate would be here
}

impl PooledIsolate {
    pub fn new(id: usize) -> Self {
        Self {
            id,
            created_at: std::time::Instant::now(),
            executions: 0,
        }
    }

    pub fn mark_executed(&mut self) {
        self.executions += 1;
    }
}

/// Pool of pre-warmed V8 isolates
pub struct IsolatePool {
    size: usize,
    available: Semaphore,
    isolates: Mutex<VecDeque<PooledIsolate>>,
    config: IsolateConfig,
}

impl IsolatePool {
    /// Create new isolate pool
    pub fn new(size: usize) -> Self {
        let isolates = Mutex::new(VecDeque::with_capacity(size));
        
        // Pre-warm isolates
        for i in 0..size {
            isolates.lock().push_back(PooledIsolate::new(i));
        }
        
        Self {
            size,
            available: Semaphore::new(size),
            isolates,
            config: IsolateConfig::default(),
        }
    }

    /// Acquire an isolate from the pool
    pub async fn acquire(&self,
    ) -> anyhow::Result<PooledIsolate> {
        let _permit = self.available.acquire().await?;
        
        let isolate = self.isolates.lock().pop_front()
            .ok_or_else(|| anyhow::anyhow!("no isolates available"))?;
        
        trace!("acquired isolate {}", isolate.id);
        Ok(isolate)
    }

    /// Return an isolate to the pool
    pub fn release(&self,
        mut isolate: PooledIsolate,
    ) {
        isolate.mark_executed();
        self.isolates.lock().push_back(isolate);
        self.available.add_permits(1);
        trace!("released isolate back to pool");
    }

    /// Current available isolates
    pub fn available_count(&self) -> usize {
        self.isolates.lock().len()
    }

    /// Total pool size
    pub fn size(&self) -> usize {
        self.size
    }
}

/// Auto-returning isolate guard
pub struct IsolateGuard {
    isolate: Option<PooledIsolate>,
    pool: Arc<IsolatePool>,
}

impl IsolateGuard {
    pub fn new(isolate: PooledIsolate, pool: Arc<IsolatePool>) -> Self {
        Self {
            isolate: Some(isolate),
            pool,
        }
    }

    pub fn isolate(&mut self) -> &mut PooledIsolate {
        self.isolate.as_mut().unwrap()
    }
}

impl Drop for IsolateGuard {
    fn drop(&mut self) {
        if let Some(isolate) = self.isolate.take() {
            self.pool.release(isolate);
        }
    }
}
