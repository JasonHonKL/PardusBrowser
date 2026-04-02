//! Unified browser that combines navigation, interaction, and tab management.
//!
//! `Browser` replaces the separate `App` + `TabManager` + `Page` pattern with
//! a single entry point. All operations work on the active tab.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;

use pardus_debug::NetworkLog;

use crate::config::BrowserConfig;
use crate::interact::actions::InteractionResult;
use crate::interact::{FormState, ScrollDirection};
use crate::page::Page;
use crate::push::PushCache;
use crate::tab::{Tab, TabId, TabState};

/// Unified headless browser for AI agents.
///
/// Owns the HTTP client, tab state, and provides navigation + interaction
/// as a single cohesive API. Every operation targets the active tab.
pub struct Browser {
    pub http_client: reqwest::Client,
    pub config: BrowserConfig,
    pub network_log: Arc<Mutex<NetworkLog>>,
    pub push_cache: Arc<PushCache>,
    tabs: HashMap<TabId, Tab>,
    active_tab: Option<TabId>,
}

impl std::fmt::Debug for Browser {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Browser")
            .field("tab_count", &self.tabs.len())
            .field("active_tab", &self.active_tab)
            .finish()
    }
}

impl Browser {
    // -----------------------------------------------------------------------
    // Construction
    // -----------------------------------------------------------------------

    /// Create a new Browser with the given configuration.
    pub fn new(config: BrowserConfig) -> Self {
        let mut client_builder = reqwest::Client::builder()
            .user_agent(&config.user_agent)
            .timeout(std::time::Duration::from_millis(config.timeout_ms as u64));

        // Sandbox: disable cookie store for ephemeral sessions
        if !config.sandbox.ephemeral_session {
            client_builder = client_builder.cookie_store(true);
        }

        // Apply proxy configuration
        client_builder = Self::apply_proxy_config(client_builder, &config.proxy);

        // Certificate pinning: use custom TLS connector when pins are configured
        if let Some(pinning) = &config.cert_pinning {
            if !pinning.pins.is_empty() || !pinning.default_pins.is_empty() {
                client_builder = match crate::tls::pinned_client_builder(client_builder, pinning) {
                    Ok(builder) => builder,
                    Err(e) => {
                        tracing::warn!("certificate pinning setup failed, using default TLS: {}", e);
                        // Rebuild without pinning since builder was moved
                        let mut new_builder = reqwest::Client::builder()
                            .user_agent(&config.user_agent)
                            .timeout(std::time::Duration::from_millis(config.timeout_ms as u64));
                        if !config.sandbox.ephemeral_session {
                            new_builder = new_builder.cookie_store(true);
                        }
                        // Re-apply proxy config after rebuild
                        new_builder = Self::apply_proxy_config(new_builder, &config.proxy);
                        new_builder
                    }
                };
            }
        }

        let http_client = client_builder
            .build()
            .expect("failed to build HTTP client");

        let push_cache = Arc::new(PushCache::new(
            config.push.max_push_resources,
            config.push.push_cache_ttl_secs,
        ));

        Self {
            http_client,
            config,
            network_log: Arc::new(Mutex::new(NetworkLog::new())),
            push_cache,
            tabs: HashMap::new(),
            active_tab: None,
        }
    }

    /// Apply proxy configuration to the HTTP client builder.
    fn apply_proxy_config(
        mut builder: reqwest::ClientBuilder,
        proxy_config: &crate::config::ProxyConfig,
    ) -> reqwest::ClientBuilder {
        if !proxy_config.is_configured() {
            return builder;
        }

        // Parse no_proxy list into reqwest::NoProxy if present
        let no_proxy = proxy_config.no_proxy.as_ref().and_then(|np| {
            reqwest::NoProxy::from_string(np)
        });

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

    /// Create a default Browser wrapped in `Arc` for sharing.
    pub fn default_shared() -> Arc<Self> {
        Arc::new(Self::new(BrowserConfig::default()))
    }

    // -----------------------------------------------------------------------
    // Navigation
    // -----------------------------------------------------------------------

    /// Navigate to a URL. Creates a tab if none exists.
    ///
    /// Fetches the page, builds the parsed HTML, updates tab history.
    pub async fn navigate(&mut self, url: &str) -> anyhow::Result<&Tab> {
        // Sandbox: check navigation domain restriction
        if !self.config.sandbox.is_navigation_allowed(url) {
            anyhow::bail!("Navigation to '{}' blocked by sandbox policy", url);
        }

        if self.active_tab.is_none() {
            let id = self.create_tab(url);
            let tab = self.tabs.get_mut(&id).unwrap();
            tab.load_with_client(&self.http_client, &self.network_log, &self.config, false, 0).await?;
            self.active_tab = Some(id);
            return Ok(self.tabs.get(&id).unwrap());
        }

        let id = self.active_tab.unwrap();
        let tab = self.tabs.get_mut(&id).ok_or_else(|| anyhow::anyhow!("active tab missing"))?;
        tab.navigate_with_client(&self.http_client, &self.network_log, &self.config, url, false, 0).await?;
        Ok(self.tabs.get(&id).unwrap())
    }

    /// Navigate with JS execution enabled.
    pub async fn navigate_with_js(&mut self, url: &str, wait_ms: u32) -> anyhow::Result<&Tab> {
        // Sandbox: check navigation domain restriction
        if !self.config.sandbox.is_navigation_allowed(url) {
            anyhow::bail!("Navigation to '{}' blocked by sandbox policy", url);
        }

        if self.active_tab.is_none() {
            let id = self.create_tab(url);
            let tab = self.tabs.get_mut(&id).unwrap();
            tab.load_with_client(&self.http_client, &self.network_log, &self.config, true, wait_ms).await?;
            self.active_tab = Some(id);
            return Ok(self.tabs.get(&id).unwrap());
        }

        let id = self.active_tab.unwrap();
        let tab = self.tabs.get_mut(&id).ok_or_else(|| anyhow::anyhow!("active tab missing"))?;
        tab.navigate_with_client(&self.http_client, &self.network_log, &self.config, url, true, wait_ms).await?;
        Ok(self.tabs.get(&id).unwrap())
    }

    /// Reload the active tab.
    pub async fn reload(&mut self) -> anyhow::Result<&Tab> {
        let id = self.require_active_id()?;
        let tab = self.tabs.get_mut(&id).unwrap();
        tab.reload_with_client(&self.http_client, &self.network_log, &self.config).await?;
        Ok(self.tabs.get(&id).unwrap())
    }

    // -----------------------------------------------------------------------
    // Interactions — all operate on the active tab's page
    // -----------------------------------------------------------------------

    /// Click an element. If JS is enabled, dispatches click event in V8 DOM first.
    /// If the click produces navigation, the tab is updated.
    pub async fn click(&mut self, selector: &str) -> anyhow::Result<InteractionResult> {
        #[cfg(feature = "js")]
        if self.is_js_enabled() {
            let page = self.require_active_page()?;
            let app = self.temp_app();
            let result = crate::interact::js_interact::js_click(&app, page, selector).await?;
            drop(app);
            return self.apply_navigated_result(result);
        }

        let page = self.require_active_page()?;
        let handle = page.query(selector).ok_or_else(|| {
            anyhow::anyhow!("Element not found: {}", selector)
        })?;
        let app = self.temp_app();
        let result = crate::interact::actions::click(&app, page, &handle).await?;
        drop(app);
        self.apply_navigated_result(result)
    }

    /// Click an element by its element ID (shown in semantic tree as [#1], [#2], etc.)
    /// This is the preferred way for AI agents to click elements.
    pub async fn click_by_id(&mut self, id: usize) -> anyhow::Result<InteractionResult> {
        let page = self.require_active_page()?;
        let handle = page.find_by_element_id(id).ok_or_else(|| {
            anyhow::anyhow!("Element with ID {} not found", id)
        })?;

        #[cfg(feature = "js")]
        if self.is_js_enabled() {
            let selector = handle.selector.clone();
            let app = self.temp_app();
            let result = crate::interact::js_interact::js_click(&app, page, &selector).await?;
            drop(app);
            return self.apply_navigated_result(result);
        }

        let app = self.temp_app();
        let result = crate::interact::actions::click(&app, page, &handle).await?;
        drop(app);
        self.apply_navigated_result(result)
    }

    /// Type text into a form field.
    /// If JS is enabled, dispatches input/change events in V8 DOM.
    pub async fn type_text(&mut self, selector: &str, value: &str) -> anyhow::Result<InteractionResult> {
        #[cfg(feature = "js")]
        if self.is_js_enabled() {
            let page = self.require_active_page()?;
            let result = crate::interact::js_interact::js_type(page, selector, value).await?;
            return self.apply_navigated_result(result);
        }

        let page = self.require_active_page()?;
        let handle = page.query(selector).ok_or_else(|| {
            anyhow::anyhow!("Element not found: {}", selector)
        })?;
        crate::interact::actions::type_text(page, &handle, value)
    }

    /// Type text into a form field by its element ID (shown in semantic tree as [#1], [#2], etc.)
    /// This is the preferred way for AI agents to fill form fields.
    pub async fn type_by_id(&mut self, id: usize, value: &str) -> anyhow::Result<InteractionResult> {
        let page = self.require_active_page()?;
        let handle = page.find_by_element_id(id).ok_or_else(|| {
            anyhow::anyhow!("Element with ID {} not found", id)
        })?;

        #[cfg(feature = "js")]
        if self.is_js_enabled() {
            let selector = handle.selector.clone();
            return crate::interact::js_interact::js_type(page, &selector, value).await;
        }

        crate::interact::actions::type_text(page, &handle, value)
    }

    /// Submit a form with the given field values.
    /// If JS is enabled, dispatches submit event first and respects preventDefault.
    pub async fn submit(
        &mut self,
        form_selector: &str,
        state: &FormState,
    ) -> anyhow::Result<InteractionResult> {
        #[cfg(feature = "js")]
        if self.is_js_enabled() {
            let page = self.require_active_page()?;
            let app = self.temp_app();
            let result = crate::interact::js_interact::js_submit(&app, page, form_selector, state).await?;
            drop(app);
            return self.apply_navigated_result(result);
        }

        let page = self.require_active_page()?;
        let app = self.temp_app();
        let result = crate::interact::form::submit_form(&app, page, form_selector, state).await?;
        drop(app);
        self.apply_navigated_result(result)
    }

    /// Wait for a CSS selector to appear.
    pub async fn wait_for(
        &mut self,
        selector: &str,
        timeout_ms: u32,
    ) -> anyhow::Result<InteractionResult> {
        let page = self.require_active_page()?;
        let app = self.temp_app();
        let result = crate::interact::wait::wait_for_selector(
            &app, page, selector, timeout_ms, 500,
        ).await?;
        drop(app);
        Ok(result)
    }

    /// Scroll. If JS is enabled, dispatches scroll/wheel events in V8 DOM.
    /// Otherwise uses URL-based pagination detection.
    pub async fn scroll(&mut self, direction: ScrollDirection) -> anyhow::Result<InteractionResult> {
        #[cfg(feature = "js")]
        if self.is_js_enabled() {
            let page = self.require_active_page()?;
            let result = crate::interact::js_interact::js_scroll(page, direction).await?;
            return self.apply_navigated_result(result);
        }

        let page = self.require_active_page()?;
        let app = self.temp_app();
        let result = crate::interact::scroll::scroll(&app, page, direction).await?;
        drop(app);
        self.apply_navigated_result(result)
    }

    /// Toggle a checkbox or radio.
    pub fn toggle(&mut self, selector: &str) -> anyhow::Result<InteractionResult> {
        let page = self.require_active_page()?;
        let handle = page.query(selector).ok_or_else(|| {
            anyhow::anyhow!("Element not found: {}", selector)
        })?;
        crate::interact::actions::toggle(page, &handle)
    }

    /// Select an option in a `<select>` element.
    pub fn select_option(&mut self, selector: &str, value: &str) -> anyhow::Result<InteractionResult> {
        let page = self.require_active_page()?;
        let handle = page.query(selector).ok_or_else(|| {
            anyhow::anyhow!("Element not found: {}", selector)
        })?;
        crate::interact::actions::select_option(page, &handle, value)
    }

    // -----------------------------------------------------------------------
    // Tab management
    // -----------------------------------------------------------------------

    /// Create a new tab with the given URL (does not load it).
    pub fn create_tab(&mut self, url: impl Into<String>) -> TabId {
        let tab = Tab::new(url);
        let id = tab.id;
        self.tabs.insert(id, tab);
        id
    }

    /// Create a tab with custom configuration.
    pub fn create_tab_with_config(
        &mut self,
        url: impl Into<String>,
        config: crate::tab::tab::TabConfig,
    ) -> TabId {
        let tab = Tab::with_config(url, config);
        let id = tab.id;
        self.tabs.insert(id, tab);
        id
    }

    /// Create, activate, and load a tab.
    pub async fn open_tab(&mut self, url: impl Into<String>) -> anyhow::Result<&Tab> {
        let id = self.create_tab(url);
        self.switch_to(id).await
    }

    /// Switch to a tab by ID, loading it if needed.
    pub async fn switch_to(&mut self, id: TabId) -> anyhow::Result<&Tab> {
        if !self.tabs.contains_key(&id) {
            return Err(anyhow::anyhow!("Tab not found: {}", id));
        }
        self.active_tab = Some(id);
        let tab = self.tabs.get_mut(&id).unwrap();
        tab.activate();

        let needs_load = tab.page.is_none() && matches!(tab.state, TabState::Loading);
        if needs_load {
            tab.load_with_client(&self.http_client, &self.network_log, &self.config, tab.config.js_enabled, tab.config.wait_ms).await?;
        }

        Ok(self.tabs.get(&id).unwrap())
    }

    /// Close a tab. Returns true if it was the active tab.
    pub fn close_tab(&mut self, id: TabId) -> bool {
        if self.tabs.remove(&id).is_none() {
            return false;
        }
        let was_active = self.active_tab == Some(id);
        if was_active {
            self.active_tab = self.tabs.keys().next().copied();
            if let Some(new_id) = self.active_tab {
                if let Some(tab) = self.tabs.get_mut(&new_id) {
                    tab.activate();
                }
            }
        }
        was_active
    }

    /// Close all tabs.
    pub fn close_all(&mut self) {
        self.tabs.clear();
        self.active_tab = None;
    }

    /// Close all tabs except the active one.
    pub fn close_others(&mut self) {
        if let Some(active) = self.active_tab {
            self.tabs.retain(|id, _| *id == active);
        }
    }

    /// List all tabs.
    pub fn list_tabs(&self) -> Vec<&Tab> {
        self.tabs.values().collect()
    }

    /// Number of open tabs.
    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }

    // -----------------------------------------------------------------------
    // History
    // -----------------------------------------------------------------------

    /// Go back in the active tab's history.
    pub async fn go_back(&mut self) -> anyhow::Result<Option<&Tab>> {
        let id = self.require_active_id()?;
        let tab = self.tabs.get_mut(&id).unwrap();
        if tab.history_index > 0 {
            tab.history_index -= 1;
            tab.url = tab.history[tab.history_index].clone();
            tab.page = None;
            tab.load_with_client(&self.http_client, &self.network_log, &self.config, tab.config.js_enabled, tab.config.wait_ms).await?;
            Ok(Some(self.tabs.get(&id).unwrap()))
        } else {
            Ok(None)
        }
    }

    /// Go forward in the active tab's history.
    pub async fn go_forward(&mut self) -> anyhow::Result<Option<&Tab>> {
        let id = self.require_active_id()?;
        let tab = self.tabs.get_mut(&id).unwrap();
        if tab.history_index < tab.history.len() - 1 {
            tab.history_index += 1;
            tab.url = tab.history[tab.history_index].clone();
            tab.page = None;
            tab.load_with_client(&self.http_client, &self.network_log, &self.config, tab.config.js_enabled, tab.config.wait_ms).await?;
            Ok(Some(self.tabs.get(&id).unwrap()))
        } else {
            Ok(None)
        }
    }

    // -----------------------------------------------------------------------
    // Accessors
    // -----------------------------------------------------------------------

    /// Get the currently active tab.
    pub fn active_tab(&self) -> Option<&Tab> {
        self.active_tab.and_then(|id| self.tabs.get(&id))
    }

    /// Get the currently active tab (mutable).
    pub fn active_tab_mut(&mut self) -> Option<&mut Tab> {
        self.active_tab.and_then(move |id| self.tabs.get_mut(&id))
    }

    /// Get the active tab's page.
    pub fn current_page(&self) -> Option<&Page> {
        self.active_tab().and_then(|t| t.page.as_ref())
    }

    /// Get the active tab's URL.
    pub fn current_url(&self) -> Option<&str> {
        self.active_tab().map(|t| t.url.as_str())
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    fn require_active_id(&self) -> anyhow::Result<TabId> {
        self.active_tab.ok_or_else(|| anyhow::anyhow!("No active tab"))
    }

    fn require_active_page(&self) -> anyhow::Result<&Page> {
        self.current_page().ok_or_else(|| anyhow::anyhow!("No page loaded in active tab"))
    }

    /// Check if the active tab has JS execution enabled.
    #[allow(dead_code)]
    fn is_js_enabled(&self) -> bool {
        self.active_tab().map(|t| t.config.js_enabled).unwrap_or(false)
    }

    /// Create a temporary `Arc<App>` that borrows from Browser's fields.
    /// This lets us reuse the existing interact functions unchanged.
    fn temp_app(&self) -> Arc<crate::app::App> {
        Arc::new(crate::app::App {
            http_client: self.http_client.clone(),
            config: parking_lot::RwLock::new(self.config.clone()),
            network_log: self.network_log.clone(),
        })
    }

    /// If an interaction produced a `Navigated` result, update the active tab.
    fn apply_navigated_result(&mut self, result: InteractionResult) -> anyhow::Result<InteractionResult> {
        if let InteractionResult::Navigated(new_page) = result {
            let id = self.active_tab.unwrap();
            let tab = self.tabs.get_mut(&id).unwrap();
            tab.update_page(new_page);
            Ok(InteractionResult::Navigated(
                tab.page.as_ref().unwrap().clone_shallow()
            ))
        } else if let InteractionResult::Scrolled { url, page: new_page } = result {
            let id = self.active_tab.unwrap();
            let tab = self.tabs.get_mut(&id).unwrap();
            tab.update_page(new_page);
            let url_clone = url.clone();
            Ok(InteractionResult::Scrolled {
                url: url_clone,
                page: tab.page.as_ref().unwrap().clone_shallow(),
            })
        } else {
            Ok(result)
        }
    }
}

impl Default for Browser {
    fn default() -> Self {
        Self::new(BrowserConfig::default())
    }
}
