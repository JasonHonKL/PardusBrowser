//! HTTP resource fetcher with optimized client

use super::ResourceConfig;
use bytes::Bytes;
use std::time::Duration;
use tracing::{trace, instrument};

/// Fetch options
#[derive(Debug, Clone)]
pub struct FetchOptions {
    pub timeout_ms: u64,
    pub follow_redirects: bool,
    pub accept_encoding: Vec<String>,
}

impl Default for FetchOptions {
    fn default() -> Self {
        Self {
            timeout_ms: 30000,
            follow_redirects: true,
            accept_encoding: vec!["gzip".to_string(), "br".to_string()],
        }
    }
}

/// Fetch result
#[derive(Debug, Clone)]
pub struct FetchResult {
    pub url: String,
    pub status: u16,
    pub body: Option<Bytes>,
    pub content_type: Option<String>,
    pub error: Option<String>,
    pub duration_ms: u64,
    pub size: usize,
}

impl FetchResult {
    pub fn success(
        url: &str,
        status: u16,
        body: Bytes,
        content_type: Option<String>,
        duration_ms: u64,
    ) -> Self {
        Self {
            url: url.to_string(),
            status,
            body: Some(body.clone()),
            content_type,
            error: None,
            duration_ms,
            size: body.len(),
        }
    }

    pub fn error(url: &str, error: impl Into<String>) -> Self {
        Self {
            url: url.to_string(),
            status: 0,
            body: None,
            content_type: None,
            error: Some(error.into()),
            duration_ms: 0,
            size: 0,
        }
    }

    pub fn is_success(&self) -> bool {
        self.error.is_none() && self.status >= 200 && self.status < 300
    }
}

/// High-performance resource fetcher
pub struct ResourceFetcher {
    client: reqwest::Client,
    config: ResourceConfig,
}

impl ResourceFetcher {
    pub fn new(config: ResourceConfig) -> Self {
        let client = reqwest::Client::builder()
            .pool_max_idle_per_host(config.max_concurrent)
            .http2_prior_knowledge()
            .http2_adaptive_window(true)
            .tcp_keepalive(Duration::from_secs(60))
            .timeout(Duration::from_secs(30))
            .build()
            .expect("failed to build HTTP client");

        Self { client, config }
    }

    /// Fetch a single resource
    #[instrument(skip(self), level = "trace")]
    pub async fn fetch(&self,
        url: &str,
    ) -> FetchResult {
        let start = std::time::Instant::now();
        trace!("fetching {}", url);

        match self.client.get(url).send().await {
            Ok(response) => {
                let status = response.status().as_u16();
                let content_type = response.headers()
                    .get(reqwest::header::CONTENT_TYPE)
                    .and_then(|v| v.to_str().ok())
                    .map(|s| s.to_string());

                match response.bytes().await {
                    Ok(body) => {
                        let elapsed = start.elapsed();
                        trace!("fetch complete: {} ({} bytes)", url, body.len());
                        FetchResult::success(url, status, body, content_type, elapsed.as_millis() as u64)
                    }
                    Err(e) => {
                        FetchResult::error(url, format!("body read error: {}", e))
                    }
                }
            }
            Err(e) => {
                FetchResult::error(url, format!("request error: {}", e))
            }
        }
    }

    /// Fetch with custom options
    pub async fn fetch_with_options(
        &self,
        url: &str,
        options: FetchOptions,
    ) -> FetchResult {
        let start = std::time::Instant::now();

        let mut request = self.client.get(url);

        if !options.follow_redirects {
            request = request.redirect(reqwest::redirect::Policy::none());
        }

        match request.send().await {
            Ok(response) => {
                let status = response.status().as_u16();
                let content_type = response.headers()
                    .get(reqwest::header::CONTENT_TYPE)
                    .and_then(|v| v.to_str().ok())
                    .map(|s| s.to_string());

                match response.bytes().await {
                    Ok(body) => {
                        let elapsed = start.elapsed();
                        FetchResult::success(url, status, body, content_type, elapsed.as_millis() as u64)
                    }
                    Err(e) => FetchResult::error(url, e),
                }
            }
            Err(e) => FetchResult::error(url, e),
        }
    }

    /// Quick check if resource exists (HEAD request)
    pub async fn exists(&self,
        url: &str,
    ) -> bool {
        match self.client.head(url).send().await {
            Ok(response) => response.status().is_success(),
            Err(_) => false,
        }
    }

    /// Get content length without downloading
    pub async fn content_length(&self,
        url: &str,
    ) -> Option<usize> {
        match self.client.head(url).send().await {
            Ok(response) => response.headers()
                .get(reqwest::header::CONTENT_LENGTH)
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse().ok()),
            Err(_) => None,
        }
    }
}

/// Cached fetcher with TTL
pub struct CachedFetcher {
    fetcher: ResourceFetcher,
    cache: dashmap::DashMap<String, (FetchResult, std::time::Instant)>,
    ttl: Duration,
}

impl CachedFetcher {
    pub fn new(config: ResourceConfig, ttl_secs: u64) -> Self {
        Self {
            fetcher: ResourceFetcher::new(config),
            cache: dashmap::DashMap::new(),
            ttl: Duration::from_secs(ttl_secs),
        }
    }

    pub async fn fetch(&self, url: &str) -> FetchResult {
        // Check cache
        if let Some(entry) = self.cache.get(url) {
            if entry.1.elapsed() < self.ttl {
                trace!("cache hit: {}", url);
                return entry.0.clone();
            }
        }

        // Fetch and cache
        let result = self.fetcher.fetch(url).await;
        if result.is_success() {
            self.cache.insert(url.to_string(), (result.clone(), std::time::Instant::now()));
        }

        result
    }

    /// Invalidate cache entry
    pub fn invalidate(&self,
        url: &str,
    ) {
        self.cache.remove(url);
    }

    /// Clear all cache
    pub fn clear(&self) {
        self.cache.clear();
    }
}
