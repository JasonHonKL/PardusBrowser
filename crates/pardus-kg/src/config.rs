use serde::{Deserialize, Serialize};

/// Configuration for a KG crawl.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlConfig {
    /// Maximum crawl depth from root (0 = root only).
    pub max_depth: usize,
    /// Maximum total pages to visit.
    pub max_pages: usize,
    /// Polite delay between requests in milliseconds.
    pub delay_ms: u64,
    /// Whether to verify transitions by following them.
    pub verify_transitions: bool,
    /// Whether to discover pagination transitions.
    pub discover_pagination: bool,
    /// Whether to discover hash navigation transitions.
    pub discover_hash_nav: bool,
    /// Whether to discover form submission transitions.
    pub discover_forms: bool,
}

impl Default for CrawlConfig {
    fn default() -> Self {
        Self {
            max_depth: 3,
            max_pages: 50,
            delay_ms: 200,
            verify_transitions: true,
            discover_pagination: true,
            discover_hash_nav: true,
            discover_forms: false,
        }
    }
}
