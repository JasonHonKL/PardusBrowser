use std::path::PathBuf;

/// Connection pool configuration for HTTP client.
#[derive(Debug, Clone)]
pub struct ConnectionPoolConfig {
    /// Maximum idle connections per host (default: 32)
    pub max_idle_per_host: usize,
    /// Idle connection timeout in seconds (default: 90)
    pub idle_timeout_secs: u64,
    /// TCP keepalive interval in seconds (default: 60)
    pub tcp_keepalive_secs: u64,
    /// Enable HTTP/2 (default: true)
    pub enable_http2: bool,
}

impl Default for ConnectionPoolConfig {
    fn default() -> Self {
        Self {
            max_idle_per_host: 32,
            idle_timeout_secs: 90,
            tcp_keepalive_secs: 60,
            enable_http2: true,
        }
    }
}

fn default_cache_dir() -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        if let Some(home) = std::env::var_os("HOME") {
            let p = PathBuf::from(home).join("Library/Caches/pardus-browser");
            if p.parent().map_or(false, |d| d.exists()) {
                return p;
            }
        }
    }
    #[cfg(target_os = "linux")]
    {
        if let Some(xdg) = std::env::var_os("XDG_CACHE_HOME") {
            return PathBuf::from(xdg).join("pardus-browser");
        }
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join(".cache/pardus-browser");
        }
    }
    PathBuf::from("/tmp/pardus-browser")
}

/// Configuration for HTTP/2 push simulation.
#[derive(Debug, Clone)]
pub struct PushConfig {
    /// Enable client-side push simulation (default: true).
    pub enable_push: bool,
    /// Maximum number of resources in the push cache (default: 32).
    pub max_push_resources: usize,
    /// Push cache TTL in seconds (default: 30).
    pub push_cache_ttl_secs: u64,
}

impl Default for PushConfig {
    fn default() -> Self {
        Self {
            enable_push: true,
            max_push_resources: 32,
            push_cache_ttl_secs: 30,
        }
    }
}

#[derive(Debug, Clone)]
pub struct BrowserConfig {
    pub cache_dir: PathBuf,
    pub user_agent: String,
    pub timeout_ms: u32,
    pub wait_ms: u32,
    pub screenshot_endpoint: Option<String>,
    pub screenshot_timeout_ms: u64,
    pub viewport_width: u32,
    pub viewport_height: u32,
    pub connection_pool: ConnectionPoolConfig,
    pub push: PushConfig,
}

impl BrowserConfig {
    pub fn effective_user_agent(&self) -> &str {
        &self.user_agent
    }
}

impl Default for BrowserConfig {
    fn default() -> Self {
        Self {
            cache_dir: default_cache_dir(),
            user_agent: format!("PardusBrowser/{}", env!("CARGO_PKG_VERSION")),
            timeout_ms: 10_000,
            wait_ms: 3_000,
            screenshot_endpoint: None,
            screenshot_timeout_ms: 10_000,
            viewport_width: 1280,
            viewport_height: 720,
            connection_pool: ConnectionPoolConfig::default(),
            push: PushConfig::default(),
        }
    }
}
