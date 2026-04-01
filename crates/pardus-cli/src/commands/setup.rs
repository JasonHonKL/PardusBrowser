//! Shared CLI setup helpers to reduce duplication between command handlers.

use anyhow::Result;
use std::sync::Arc;

/// Build a `BrowserConfig` from common CLI flags.
pub fn build_config(stealth: bool, proxy: Option<&str>) -> pardus_core::BrowserConfig {
    let mut config = pardus_core::BrowserConfig::default();
    if stealth {
        config.stealth = true;
    }
    if let Some(proxy_url) = proxy {
        config.proxy = Some(proxy_url.to_string());
    }
    config
}

/// Build an `Arc<App>` with optional session, auth, and custom headers.
///
/// This consolidates the session/app setup logic shared between
/// `navigate`, `interact`, and other CLI commands.
pub fn create_app(
    config: pardus_core::BrowserConfig,
    session: Option<&str>,
    auth: Option<&str>,
    headers: &[String],
) -> Result<Arc<pardus_core::App>> {
    let app = if let Some(session_name) = session {
        let store = Arc::new(pardus_core::SessionStore::load(session_name, &config.cache_dir)?);

        if let Some(auth_str) = auth {
            if let Some((name, value)) = pardus_core::session::parse_auth_header(auth_str) {
                store.add_header(&name, &value);
            }
        }
        for h in headers {
            if let Some((name, value)) = pardus_core::session::parse_custom_header(h) {
                store.add_header(&name, &value);
            }
        }

        Arc::new(pardus_core::App::with_session(config, store)?)
    } else {
        Arc::new(pardus_core::App::new(config)?)
    };

    Ok(app)
}
