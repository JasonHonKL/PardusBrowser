//! Shared HTTP client factory.
//!
//! Eliminates duplicate `reqwest::Client` construction across the codebase.

use crate::config::BrowserConfig;
use std::sync::OnceLock;
use std::time::Duration;

fn build_client(config: &BrowserConfig) -> reqwest::Client {
    let mut builder = reqwest::Client::builder()
        .user_agent(&config.user_agent)
        .timeout(Duration::from_millis(config.timeout_ms as u64))
        .cookie_store(true)
        .pool_max_idle_per_host(config.connection_pool.max_idle_per_host)
        .pool_idle_timeout(Duration::from_secs(
            config.connection_pool.idle_timeout_secs,
        ))
        .tcp_keepalive(Duration::from_secs(
            config.connection_pool.tcp_keepalive_secs,
        ));

    if config.connection_pool.enable_http2 {
        builder = builder.http2_prior_knowledge().http2_adaptive_window(true);
    }

    builder.build().expect("failed to build HTTP client")
}

pub fn shared_client(config: &BrowserConfig) -> reqwest::Client {
    // We don't use a global singleton here because config can vary per Browser/App.
    // The caller should store and reuse the returned client.
    build_client(config)
}

/// Lightweight client for JS fetch ops (long-lived, does not depend on BrowserConfig).
pub fn fetch_client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(Duration::from_millis(10_000))
            .pool_max_idle_per_host(10)
            .pool_idle_timeout(Duration::from_secs(60))
            .tcp_keepalive(Duration::from_secs(60))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new())
    })
}
