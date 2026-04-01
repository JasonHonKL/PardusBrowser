//! Resource scheduler with HTTP/2 prioritization

use super::{Resource, ResourceConfig, ResourceKind};
use super::priority::PriorityQueue;
use super::fetcher::{ResourceFetcher, FetchResult};

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, Semaphore};
use tokio::task::JoinSet;
use tracing::{trace, instrument, debug};
use url::Url;

/// Task for resource fetching
#[derive(Debug, Clone)]
pub struct ResourceTask {
    pub url: String,
    pub kind: ResourceKind,
    pub priority: u8,
    pub origin: String,
}

impl ResourceTask {
    pub fn new(url: String, kind: ResourceKind, priority: u8) -> Self {
        let origin = Self::extract_origin(&url);
        Self { url, kind, priority, origin }
    }

    fn extract_origin(url: &str) -> String {
        Url::parse(url)
            .ok()
            .map(|u| u.origin().ascii_serialization())
            .unwrap_or_default()
    }
}

impl From<Resource> for ResourceTask {
    fn from(r: Resource) -> Self {
        Self::new(r.url, r.kind, r.priority)
    }
}

/// Schedule result
#[derive(Debug)]
pub struct ScheduleResult {
    pub tasks: Vec<ResourceTask>,
    pub results: Vec<FetchResult>,
    pub duration_ms: u64,
}

/// High-performance resource scheduler
pub struct ResourceScheduler {
    config: ResourceConfig,
    fetcher: ResourceFetcher,
    /// Per-origin semaphores for limiting concurrency
    origin_semaphores: parking_lot::Mutex<HashMap<String, Arc<Semaphore>>>,
}

impl ResourceScheduler {
    pub fn new(config: ResourceConfig) -> Self {
        let fetcher = ResourceFetcher::new(config.clone());
        Self {
            config,
            fetcher,
            origin_semaphores: parking_lot::Mutex::new(HashMap::new()),
        }
    }

    /// Schedule a batch of tasks with optimal ordering
    #[instrument(skip(self, tasks), level = "debug")]
    pub async fn schedule_batch(
        self: Arc<Self>,
        tasks: Vec<ResourceTask>,
    ) -> Vec<FetchResult> {
        let start = std::time::Instant::now();
        debug!("scheduling {} tasks", tasks.len());

        // Sort by priority (critical first, then by origin grouping)
        let mut queue = PriorityQueue::new();
        for task in tasks {
            queue.push(task.priority, task);
        }

        // Group by origin for connection reuse
        let by_origin = self.group_by_origin(&queue.into_vec());

        // Spawn fetches for each origin group
        let mut results = Vec::new();
        let mut join_set = JoinSet::new();

        for (origin, origin_tasks) in by_origin {
            let scheduler = self.clone();
            join_set.spawn(async move {
                scheduler.fetch_origin_group(origin, origin_tasks).await
            });
        }

        // Collect results
        while let Some(Ok(group_results)) = join_set.join_next().await {
            results.extend(group_results);
        }

        let elapsed = start.elapsed();
        debug!("batch fetch completed in {:?}, {} results", elapsed, results.len());

        results
    }

    /// Group tasks by origin for connection optimization
    fn group_by_origin(
        &self,
        tasks: &[ResourceTask],
    ) -> HashMap<String, Vec<ResourceTask>> {
        let mut groups: HashMap<String, Vec<ResourceTask>> = HashMap::new();

        for task in tasks {
            groups.entry(task.origin.clone())
                .or_default()
                .push(task.clone());
        }

        groups
    }

    /// Fetch all resources from a single origin
    async fn fetch_origin_group(
        self: Arc<Self>,
        origin: String,
        tasks: Vec<ResourceTask>,
    ) -> Vec<FetchResult> {
        let semaphore = self.get_origin_semaphore(&origin);
        let mut results = Vec::new();

        for task in tasks {
            let permit = semaphore.clone().acquire_owned().await;
            if permit.is_err() {
                results.push(FetchResult::error(&task.url, "semaphore closed"));
                continue;
            }

            let result = self.fetcher.fetch(&task.url).await;
            results.push(result);

            // Keep permit alive until fetch completes
            drop(permit);
        }

        results
    }

    /// Get or create semaphore for origin
    fn get_origin_semaphore(&self,
        origin: &str,
    ) -> Arc<Semaphore> {
        let mut semaphores = self.origin_semaphores.lock();
        semaphores.entry(origin.to_string())
            .or_insert_with(|| Arc::new(Semaphore::new(self.config.max_concurrent)))
            .clone()
    }

    /// Schedule with priority hints for HTTP/2
    pub async fn schedule_with_priority(
        &self,
        tasks: Vec<ResourceTask>,
        _priority_hints: HashMap<String, u8>,
    ) -> Vec<FetchResult> {
        // HTTP/2 prioritization is handled by the HTTP client
        // We just ensure proper ordering here
        let mut queue = PriorityQueue::new();
        for task in tasks {
            queue.push(task.priority, task);
        }

        let mut results = Vec::new();
        for (_, task) in queue.drain() {
            let result = self.fetcher.fetch(&task.url).await;
            results.push(result);
        }

        results
    }
}

/// Critical path resource fetcher
pub struct CriticalPathFetcher {
    scheduler: Arc<ResourceScheduler>,
}

impl CriticalPathFetcher {
    pub fn new(scheduler: Arc<ResourceScheduler>) -> Self {
        Self { scheduler }
    }

    /// Fetch render-blocking resources first
    pub async fn fetch_critical(
        &self,
        resources: Vec<Resource>,
    ) -> Vec<FetchResult> {
        // Separate critical from non-critical
        let (critical, non_critical): (Vec<_>, Vec<_>) = resources.into_iter()
            .partition(|r| matches!(r.kind, ResourceKind::Stylesheet | ResourceKind::Script));

        // Fetch critical first
        let critical_tasks: Vec<_> = critical.into_iter()
            .map(|r| ResourceTask::from(r))
            .collect();

        let mut results = self.scheduler.clone().schedule_batch(critical_tasks).await;

        // Then fetch non-critical in parallel
        if !non_critical.is_empty() {
            let non_critical_tasks: Vec<_> = non_critical.into_iter()
                .map(|r| ResourceTask::from(r))
                .collect();
            let more_results = self.scheduler.clone().schedule_batch(non_critical_tasks).await;
            results.extend(more_results);
        }

        results
    }
}

/// Stream fetcher for progressive loading
pub struct StreamingResourceFetcher {
    tx: mpsc::Sender<FetchResult>,
}

impl StreamingResourceFetcher {
    pub fn new() -> (Self, mpsc::Receiver<FetchResult>) {
        let (tx, rx) = mpsc::channel(100);
        (Self { tx }, rx)
    }

    pub async fn fetch_streaming(
        &self,
        scheduler: Arc<ResourceScheduler>,
        resources: Vec<ResourceTask>,
    ) -> anyhow::Result<()> {
        // Spawn fetch tasks that send results as they complete
        for task in resources {
            let tx = self.tx.clone();
            let sched = scheduler.clone();

            tokio::spawn(async move {
                let fetcher = ResourceFetcher::new(sched.config.clone());
                let result = fetcher.fetch(&task.url).await;
                let _ = tx.send(result).await;
            });
        }

        Ok(())
    }
}
