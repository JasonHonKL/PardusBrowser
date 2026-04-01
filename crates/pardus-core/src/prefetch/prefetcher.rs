//! Prefetch worker for background loading

use bytes::Bytes;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, Semaphore};
use tokio::time::{Duration, Instant};
use tracing::{trace, debug, warn};

/// Prefetch configuration
#[derive(Debug, Clone)]
pub struct PrefetchConfig {
    /// Max concurrent prefetches
    pub max_concurrent: usize,
    /// Max predictions to prefetch
    pub max_predictions: usize,
    /// Minimum confidence threshold (0-1)
    pub min_confidence: f64,
    /// Cooldown between prefetches
    pub cooldown_ms: u64,
    /// Enable prefetching
    pub enabled: bool,
}

impl Default for PrefetchConfig {
    fn default() -> Self {
        Self {
            max_concurrent: 2,
            max_predictions: 3,
            min_confidence: 0.3,
            cooldown_ms: 100,
            enabled: true,
        }
    }
}

/// Prefetch job
#[derive(Debug, Clone)]
pub struct PrefetchJob {
    pub url: String,
    pub priority: u8,
    pub source_url: String,
}

/// Prefetch result
#[derive(Debug, Clone)]
pub struct PrefetchResult {
    pub url: String,
    pub success: bool,
    pub data: Option<Bytes>,
    pub duration_ms: u64,
}

/// Background prefetch worker
pub struct Prefetcher {
    config: PrefetchConfig,
    /// Queue of pending jobs
    queue: mpsc::Sender<PrefetchJob>,
    /// Statistics
    stats: parking_lot::Mutex<PrefetcherStats>,
    /// Semaphore for limiting concurrency
    semaphore: Arc<Semaphore>,
}

impl Prefetcher {
    pub fn new(config: PrefetchConfig) -> Self {
        let (tx, mut rx) = mpsc::channel::<PrefetchJob>(100);
        let semaphore = Arc::new(Semaphore::new(config.max_concurrent));
        
        let stats = parking_lot::Mutex::new(PrefetcherStats::default());
        
        // Spawn worker
        let worker_stats = stats.clone();
        tokio::spawn(async move {
            let client = reqwest::Client::builder()
                .timeout(Duration::from_secs(10))
                .build()
                .expect("failed to build prefetch client");
            
            while let Some(job) = rx.recv().await {
                let start = Instant::now();
                
                match client.get(&job.url).send().await {
                    Ok(response) => {
                        if let Ok(bytes) = response.bytes().await {
                            let duration = start.elapsed();
                            trace!("prefetched {} in {:?}", job.url, duration);
                            
                            let mut s = worker_stats.lock();
                            s.successful += 1;
                            s.total_bytes += bytes.len();
                        }
                    }
                    Err(e) => {
                        trace!("prefetch failed for {}: {}", job.url, e);
                        let mut s = worker_stats.lock();
                        s.failed += 1;
                    }
                }
                
                // Cooldown
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        });
        
        Self {
            config,
            queue: tx,
            stats,
            semaphore,
        }
    }

    /// Queue a prefetch job
    pub async fn queue(&self,
        job: PrefetchJob,
    ) {
        if !self.config.enabled {
            return;
        }
        
        let _ = self.queue.send(job).await;
    }

    /// Get current statistics
    pub fn stats(&self) -> PrefetcherStats {
        *self.stats.lock()
    }
}

/// Prefetcher statistics
#[derive(Debug, Clone, Copy, Default)]
pub struct PrefetcherStats {
    pub queued: usize,
    pub successful: usize,
    pub failed: usize,
    pub total_bytes: usize,
}

/// Intelligent prefetcher that learns from success/failure
pub struct AdaptivePrefetcher {
    base: Prefetcher,
    /// Success rate per URL pattern
    success_rates: parking_lot::RwLock<HashMap<String, f64>>,
}

impl AdaptivePrefetcher {
    pub fn new(config: PrefetchConfig) -> Self {
        let base = Prefetcher::new(config);
        
        Self {
            base,
            success_rates: parking_lot::RwLock::new(HashMap::new()),
        }
    }

    /// Update success rate for a pattern
    pub fn record_success(&self,
        pattern: &str,
        success: bool,
    ) {
        let mut rates = self.success_rates.write();
        let entry = rates.entry(pattern.to_string()).or_insert(0.5);
        
        // Exponential moving average
        let alpha = 0.1;
        let new_rate = if success {
            *entry * (1.0 - alpha) + alpha
        } else {
            *entry * (1.0 - alpha)
        };
        
        *entry = new_rate;
    }

    /// Check if we should prefetch a URL
    pub fn should_prefetch(&self,
        url: &str,
    ) -> bool {
        let rates = self.success_rates.read();
        
        // Check URL patterns (simple suffix matching)
        for (pattern, rate) in rates.iter() {
            if url.ends_with(pattern) && *rate > 0.5 {
                return true;
            }
        }
        
        // Default to prefetch if no data
        rates.is_empty()
    }
}
