//! Sandbox mode for restricted execution of untrusted content.
//!
//! Provides a `SandboxPolicy` that controls what capabilities are available
//! when processing web content — JS execution mode, network access, session
//! persistence, resource limits, and automatic tracker blocking.
//!
//! All restrictions are opt-in. The default (`SandboxPolicy::off()`) preserves
//! existing behavior with zero overhead.

/// Controls how JavaScript is allowed to execute inside the sandbox.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JsSandboxMode {
    /// JS execution disabled entirely. Equivalent to `js_enabled = false`.
    Disabled,
    /// Full JS execution with no sandbox restrictions (default).
    Full,
    /// Read-only DOM: querySelector, getAttribute, textContent reads work.
    /// Blocked: DOM mutation, fetch, SSE, timers, MutationObserver.
    ReadOnly,
    /// DOM reads + writes allowed, but no network access (fetch, SSE, WebSocket).
    /// Timers are allowed.
    NoNetwork,
}

/// Policy that controls what capabilities are available in the sandbox.
///
/// Use preset constructors for common configurations:
/// - [`SandboxPolicy::off()`] — no restrictions (default)
/// - [`SandboxPolicy::strict()`] — maximum security for untrusted content
/// - [`SandboxPolicy::moderate()`] — balanced security for semi-trusted content
/// - [`SandboxPolicy::minimal()`] — light restrictions for safe crawling
#[derive(Debug, Clone)]
pub struct SandboxPolicy {
    // -- JS Restrictions --
    /// JS execution mode.
    pub js_mode: JsSandboxMode,
    /// Override per-script timeout in ms. 0 = use default (2000ms).
    pub js_timeout_ms: u64,
    /// Override max scripts per page. 0 = use default (20).
    pub js_max_scripts: usize,
    /// Override max script size in bytes. 0 = use default (100KB).
    pub js_max_script_size: usize,
    /// Max DOM nodes allowed during JS execution. 0 = unlimited.
    pub js_max_dom_nodes: usize,

    // -- Network Restrictions --
    /// Block all fetch() calls from JS.
    pub block_js_fetch: bool,
    /// Block all SSE (EventSource) connections from JS.
    pub block_js_sse: bool,
    /// Restrict navigation to only allowed domains.
    pub restrict_navigation: bool,
    /// Domains allowed for navigation (when restrict_navigation is true).
    pub allowed_navigation_domains: Vec<String>,
    /// Disable prefetching.
    pub disable_prefetch: bool,
    /// Disable HTTP/2 push simulation.
    pub disable_push: bool,

    // -- Session Isolation --
    /// Ephemeral session: no cookie/localStorage persistence to disk.
    pub ephemeral_session: bool,
    /// Disable localStorage entirely.
    pub disable_local_storage: bool,

    // -- Content Restrictions --
    /// Max response body size in bytes. 0 = unlimited.
    pub max_page_size: usize,

    // -- Interceptors --
    /// Auto-install interceptors that block known tracking domains.
    pub auto_block_trackers: bool,
}

impl SandboxPolicy {
    /// No restrictions. Existing behavior preserved exactly.
    pub fn off() -> Self {
        Self {
            js_mode: JsSandboxMode::Full,
            js_timeout_ms: 0,
            js_max_scripts: 0,
            js_max_script_size: 0,
            js_max_dom_nodes: 0,
            block_js_fetch: false,
            block_js_sse: false,
            restrict_navigation: false,
            allowed_navigation_domains: Vec::new(),
            disable_prefetch: false,
            disable_push: false,
            ephemeral_session: false,
            disable_local_storage: false,
            max_page_size: 0,
            auto_block_trackers: false,
        }
    }

    /// Maximum security for completely untrusted content.
    ///
    /// - JS disabled
    /// - All JS network blocked
    /// - Ephemeral session (no persistence)
    /// - No prefetch or push
    /// - Auto-block trackers
    /// - 5MB page size limit
    pub fn strict() -> Self {
        Self {
            js_mode: JsSandboxMode::Disabled,
            js_timeout_ms: 500,
            js_max_scripts: 5,
            js_max_script_size: 10_000,
            js_max_dom_nodes: 1_000,
            block_js_fetch: true,
            block_js_sse: true,
            restrict_navigation: false,
            allowed_navigation_domains: Vec::new(),
            disable_prefetch: true,
            disable_push: true,
            ephemeral_session: true,
            disable_local_storage: true,
            max_page_size: 5 * 1024 * 1024, // 5MB
            auto_block_trackers: true,
        }
    }

    /// Balanced security for semi-trusted content.
    ///
    /// - JS in read-only mode (DOM reads, no mutation/network)
    /// - JS network blocked
    /// - Ephemeral cookies
    /// - Auto-block trackers
    /// - 10MB page size limit
    pub fn moderate() -> Self {
        Self {
            js_mode: JsSandboxMode::ReadOnly,
            js_timeout_ms: 1000,
            js_max_scripts: 10,
            js_max_script_size: 50_000,
            js_max_dom_nodes: 5_000,
            block_js_fetch: true,
            block_js_sse: true,
            restrict_navigation: false,
            allowed_navigation_domains: Vec::new(),
            disable_prefetch: true,
            disable_push: false,
            ephemeral_session: true,
            disable_local_storage: false,
            max_page_size: 10 * 1024 * 1024, // 10MB
            auto_block_trackers: true,
        }
    }

    /// Light restrictions for safe crawling.
    ///
    /// - JS fully enabled
    /// - Auto-block trackers
    /// - Ephemeral cookies
    /// - Disable prefetch
    pub fn minimal() -> Self {
        Self {
            js_mode: JsSandboxMode::NoNetwork,
            js_timeout_ms: 0,
            js_max_scripts: 0,
            js_max_script_size: 0,
            js_max_dom_nodes: 0,
            block_js_fetch: true,
            block_js_sse: false,
            restrict_navigation: false,
            allowed_navigation_domains: Vec::new(),
            disable_prefetch: true,
            disable_push: false,
            ephemeral_session: true,
            disable_local_storage: false,
            max_page_size: 0,
            auto_block_trackers: true,
        }
    }

    /// Returns true if the sandbox has no restrictions active.
    pub fn is_off(&self) -> bool {
        self.js_mode == JsSandboxMode::Full
            && !self.block_js_fetch
            && !self.block_js_sse
            && !self.restrict_navigation
            && !self.disable_prefetch
            && !self.disable_push
            && !self.ephemeral_session
            && !self.disable_local_storage
            && self.max_page_size == 0
            && !self.auto_block_trackers
    }

    /// Check if a navigation URL is allowed by the sandbox policy.
    pub fn is_navigation_allowed(&self, url: &str) -> bool {
        if !self.restrict_navigation {
            return true;
        }
        let host = url::Url::parse(url)
            .ok()
            .and_then(|u| u.host_str().map(|h| h.to_lowercase()));
        match host {
            Some(h) => self
                .allowed_navigation_domains
                .iter()
                .any(|d| h == d.to_lowercase() || h.ends_with(&format!(".{}", d.to_lowercase()))),
            None => false,
        }
    }
}

impl Default for SandboxPolicy {
    fn default() -> Self {
        Self::off()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_off_is_default() {
        let default = SandboxPolicy::default();
        assert_eq!(default.js_mode, JsSandboxMode::Full);
        assert!(default.is_off());
    }

    #[test]
    fn test_off_has_no_restrictions() {
        let policy = SandboxPolicy::off();
        assert!(policy.is_off());
        assert!(!policy.block_js_fetch);
        assert!(!policy.ephemeral_session);
    }

    #[test]
    fn test_strict_is_not_off() {
        let policy = SandboxPolicy::strict();
        assert!(!policy.is_off());
        assert_eq!(policy.js_mode, JsSandboxMode::Disabled);
        assert!(policy.block_js_fetch);
        assert!(policy.ephemeral_session);
        assert!(policy.max_page_size > 0);
    }

    #[test]
    fn test_moderate_readonly() {
        let policy = SandboxPolicy::moderate();
        assert_eq!(policy.js_mode, JsSandboxMode::ReadOnly);
        assert!(policy.block_js_fetch);
        assert!(policy.ephemeral_session);
    }

    #[test]
    fn test_minimal_nonetwork() {
        let policy = SandboxPolicy::minimal();
        assert_eq!(policy.js_mode, JsSandboxMode::NoNetwork);
        assert!(policy.block_js_fetch);
        assert!(!policy.block_js_sse);
    }

    #[test]
    fn test_navigation_allowed_no_restriction() {
        let policy = SandboxPolicy::off();
        assert!(policy.is_navigation_allowed("https://evil.com"));
    }

    #[test]
    fn test_navigation_allowed_with_restriction() {
        let mut policy = SandboxPolicy::off();
        policy.restrict_navigation = true;
        policy.allowed_navigation_domains = vec!["example.com".to_string(), "trusted.org".to_string()];

        assert!(policy.is_navigation_allowed("https://example.com/page"));
        assert!(policy.is_navigation_allowed("https://sub.example.com/page"));
        assert!(policy.is_navigation_allowed("https://trusted.org"));
        assert!(!policy.is_navigation_allowed("https://evil.com"));
        assert!(!policy.is_navigation_allowed("https://notexample.com"));
    }

    #[test]
    fn test_navigation_case_insensitive() {
        let mut policy = SandboxPolicy::off();
        policy.restrict_navigation = true;
        policy.allowed_navigation_domains = vec!["Example.COM".to_string()];

        assert!(policy.is_navigation_allowed("https://example.com"));
        assert!(policy.is_navigation_allowed("https://EXAMPLE.COM/page"));
    }
}
