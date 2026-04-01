use std::path::PathBuf;

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

#[derive(Debug, Clone)]
pub struct BrowserConfig {
    pub cache_dir: PathBuf,
    pub user_agent: String,
    pub timeout_ms: u32,
    pub wait_ms: u32,
    /// External HTTP endpoint for screenshot rendering (e.g., "http://localhost:9223/screenshot").
    /// When set, the CDP server delegates screenshot capture to this service.
    pub screenshot_endpoint: Option<String>,
    /// Timeout in milliseconds for screenshot provider requests.
    pub screenshot_timeout_ms: u64,
    /// Default viewport width for screenshots.
    pub viewport_width: u32,
    /// Default viewport height for screenshots.
    pub viewport_height: u32,
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
        }
    }
}
