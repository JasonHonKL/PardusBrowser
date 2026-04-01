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
use crate::tab::{Tab, TabId, TabState};

/// Unified headless browser for AI agents.
///
/// Owns the HTTP client, tab state, and provides navigation + interaction
/// as a single cohesive API. Every operation targets the active tab.
pub struct Browser {
    pub http_client: reqwest::Client,
    pub config: BrowserConfig,
    pub network_log: Arc<Mutex<NetworkLog>>,
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
        let http_client = reqwest::Client::builder()
            .user_agent(&config.user_agent)
            .timeout(std::time::Duration::from_millis(config.timeout_ms as u64))
            .cookie_store(true)
            .build()
            .expect("failed to build HTTP client");

        Self {
            http_client,
            config,
            network_log: Arc::new(Mutex::new(NetworkLog::new())),
            tabs: HashMap::new(),
            active_tab: None,
        }
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
        if self.active_tab.is_none() {
            let id = self.create_tab(url);
            let tab = self.tabs.get_mut(&id).unwrap();
            tab.load_with_client(&self.http_client, &self.network_log, false, 0).await?;
            self.active_tab = Some(id);
            return Ok(self.tabs.get(&id).unwrap());
        }

        let id = self.active_tab.unwrap();
        let tab = self.tabs.get_mut(&id).ok_or_else(|| anyhow::anyhow!("active tab missing"))?;
        tab.navigate_with_client(&self.http_client, &self.network_log, url, false, 0).await?;
        Ok(self.tabs.get(&id).unwrap())
    }

    /// Navigate with JS execution enabled.
    pub async fn navigate_with_js(&mut self, url: &str, wait_ms: u32) -> anyhow::Result<&Tab> {
        if self.active_tab.is_none() {
            let id = self.create_tab(url);
            let tab = self.tabs.get_mut(&id).unwrap();
            tab.load_with_client(&self.http_client, &self.network_log, true, wait_ms).await?;
            self.active_tab = Some(id);
            return Ok(self.tabs.get(&id).unwrap());
        }

        let id = self.active_tab.unwrap();
        let tab = self.tabs.get_mut(&id).ok_or_else(|| anyhow::anyhow!("active tab missing"))?;
        tab.navigate_with_client(&self.http_client, &self.network_log, url, true, wait_ms).await?;
        Ok(self.tabs.get(&id).unwrap())
    }

    /// Reload the active tab.
    pub async fn reload(&mut self) -> anyhow::Result<&Tab> {
        let id = self.require_active_id()?;
        let tab = self.tabs.get_mut(&id).unwrap();
        tab.reload_with_client(&self.http_client, &self.network_log).await?;
        Ok(self.tabs.get(&id).unwrap())
    }

    // -----------------------------------------------------------------------
    // Interactions — all operate on the active tab's page
    // -----------------------------------------------------------------------

    /// Click an element. If the click produces navigation, the tab is updated.
    pub async fn click(&mut self, selector: &str) -> anyhow::Result<InteractionResult> {
        let page = self.require_active_page()?;

        let handle = page.query(selector).ok_or_else(|| {
            anyhow::anyhow!("Element not found: {}", selector)
        })?;

        // We need an App-compatible reference for interact functions.
        // Build a temporary App view to call existing interact logic.
        let app = self.temp_app();
        let result = crate::interact::actions::click(&app, page, &handle).await?;
        drop(app); // release borrow

        self.apply_navigated_result(result)
    }

    /// Type text into a form field.
    pub fn type_text(&mut self, selector: &str, value: &str) -> anyhow::Result<InteractionResult> {
        let page = self.require_active_page()?;
        let handle = page.query(selector).ok_or_else(|| {
            anyhow::anyhow!("Element not found: {}", selector)
        })?;
        crate::interact::actions::type_text(page, &handle, value)
    }

    /// Submit a form with the given field values.
    pub async fn submit(
        &mut self,
        form_selector: &str,
        state: &FormState,
    ) -> anyhow::Result<InteractionResult> {
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
        // Wait does not navigate, so just return
        Ok(result)
    }

    /// Scroll (URL-based pagination detection).
    pub async fn scroll(&mut self, direction: ScrollDirection) -> anyhow::Result<InteractionResult> {
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
            tab.load_with_client(&self.http_client, &self.network_log, tab.config.js_enabled, tab.config.wait_ms).await?;
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
            tab.load_with_client(&self.http_client, &self.network_log, tab.config.js_enabled, tab.config.wait_ms).await?;
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
            tab.load_with_client(&self.http_client, &self.network_log, tab.config.js_enabled, tab.config.wait_ms).await?;
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

    /// Create a temporary `Arc<App>` that borrows from Browser's fields.
    /// This lets us reuse the existing interact functions unchanged.
    fn temp_app(&self) -> Arc<crate::app::App> {
        Arc::new(crate::app::App {
            http_client: self.http_client.clone(),
            config: self.config.clone(),
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
