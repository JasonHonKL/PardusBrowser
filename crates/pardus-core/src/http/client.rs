//! Shared HTTP client factory.
//!
//! Eliminates duplicate `reqwest::Client` construction across the codebase.

use crate::config::{BrowserConfig, ProxyConfig};
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

    // Apply proxy configuration
    builder = apply_proxy_config(builder, &config.proxy);

    // Certificate pinning: use custom TLS connector when pins are configured
    if let Some(pinning) = &config.cert_pinning {
        if !pinning.pins.is_empty() || !pinning.default_pins.is_empty() {
            match crate::tls::pinned_client_builder(builder, pinning) {
                Ok(b) => {
                    return b
                        .build()
                        .expect("failed to build HTTP client with pinned TLS");
                }
                Err(e) => {
                    tracing::warn!("certificate pinning setup failed, using default TLS: {}", e);
                    // Fall through to rebuild without pinning
                }
            }
            // Rebuild builder since it was moved in the failed case
            builder = reqwest::Client::builder()
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
            // Re-apply proxy config
            builder = apply_proxy_config(builder, &config.proxy);
        }
    }

    builder.build().expect("failed to build HTTP client")
}

/// Apply proxy configuration to the HTTP client builder.
fn apply_proxy_config(
    mut builder: reqwest::ClientBuilder,
    proxy_config: &ProxyConfig,
) -> reqwest::ClientBuilder {
    if !proxy_config.is_configured() {
        return builder;
    }

    // Parse no_proxy list into reqwest::NoProxy if present
    let no_proxy = proxy_config
        .no_proxy
        .as_ref()
        .and_then(|np| reqwest::NoProxy::from_string(np));

    // Apply all_proxy first (lowest priority, applied first)
    if let Some(all_url) = &proxy_config.all_proxy {
        match reqwest::Proxy::all(all_url) {
            Ok(mut proxy) => {
                if let Some(ref np) = no_proxy {
                    proxy = proxy.no_proxy(Some(np.clone()));
                }
                builder = builder.proxy(proxy);
            }
            Err(e) => {
                tracing::warn!("Failed to configure all_proxy '{}': {}", all_url, e);
            }
        }
    }

    // Apply HTTP proxy
    if let Some(http_url) = &proxy_config.http_proxy {
        match reqwest::Proxy::http(http_url) {
            Ok(mut proxy) => {
                if let Some(ref np) = no_proxy {
                    proxy = proxy.no_proxy(Some(np.clone()));
                }
                builder = builder.proxy(proxy);
            }
            Err(e) => {
                tracing::warn!("Failed to configure http_proxy '{}': {}", http_url, e);
            }
        }
    }

    // Apply HTTPS proxy
    if let Some(https_url) = &proxy_config.https_proxy {
        match reqwest::Proxy::https(https_url) {
            Ok(mut proxy) => {
                if let Some(ref np) = no_proxy {
                    proxy = proxy.no_proxy(Some(np.clone()));
                }
                builder = builder.proxy(proxy);
            }
            Err(e) => {
                tracing::warn!("Failed to configure https_proxy '{}': {}", https_url, e);
            }
        }
    }

    builder
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
