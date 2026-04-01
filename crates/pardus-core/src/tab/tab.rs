//! Individual tab implementation
//!
//! A Tab wraps a Page and maintains tab-specific state.
//! Multiple tabs share the same App via Arc.

use std::sync::Arc;
use std::sync::Mutex;
use std::time::Instant;

use crate::Page;
use crate::app::App;
use crate::config::BrowserConfig;
use crate::interact::ElementHandle;
use crate::semantic::tree::SemanticTree;
use crate::navigation::graph::NavigationGraph;

use super::{TabId, TabState};

/// A browser tab with independent page state
///
/// Tabs share the App (HTTP client, config, network log) but maintain
/// their own page content, history, and state.
pub struct Tab {
    /// Unique identifier for this tab
    pub id: TabId,
    /// Current URL (may differ from page URL during navigation)
    pub url: String,
    /// Page title from last load
    pub title: Option<String>,
    /// The loaded page content (None while loading)
    pub page: Option<Page>,
    /// Current lifecycle state
    pub state: TabState,
    /// When the tab was created
    pub created_at: Instant,
    /// When the tab was last active
    pub last_active: Instant,
    /// Tab-specific configuration overrides
    pub config: TabConfig,
    /// Navigation history (previous URLs)
    pub history: Vec<String>,
    /// Current position in history
    pub history_index: usize,
}

impl std::fmt::Debug for Tab {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Tab")
            .field("id", &self.id)
            .field("url", &self.url)
            .field("title", &self.title)
            .field("state", &self.state)
            .field("page_loaded", &self.page.is_some())
            .field("history_len", &self.history.len())
            .field("history_index", &self.history_index)
            .finish()
    }
}

/// Tab-specific configuration that can override App defaults
#[derive(Debug, Clone, serde::Serialize)]
pub struct TabConfig {
    /// Enable JavaScript execution for this tab
    pub js_enabled: bool,
    /// Wait time for JS execution in milliseconds
    pub wait_ms: u32,
    /// Use stealth mode for this tab
    pub stealth: bool,
    /// Capture network log for this tab
    pub network_log: bool,
}

impl Default for TabConfig {
    fn default() -> Self {
        Self {
            js_enabled: false,
            wait_ms: 3000,
            stealth: false,
            network_log: false,
        }
    }
}

impl Tab {
    /// Create a new tab with the given URL
    ///
    /// The tab is created in Loading state. Call `load()` to fetch the page.
    pub fn new(url: impl Into<String>) -> Self {
        let url = url.into();
        let now = Instant::now();

        Self {
            id: TabId::new(),
            url: url.clone(),
            title: None,
            page: None,
            state: TabState::Loading,
            created_at: now,
            last_active: now,
            config: TabConfig::default(),
            history: vec![url],
            history_index: 0,
        }
    }

    /// Create a new tab with custom configuration
    pub fn with_config(url: impl Into<String>, config: TabConfig) -> Self {
        let mut tab = Self::new(url);
        tab.config = config;
        tab
    }

    // -------------------------------------------------------------------
    // Legacy App-based methods (kept for backward compatibility)
    // -------------------------------------------------------------------

    /// Load the page content using the shared App
    pub async fn load(&mut self, app: &Arc<App>) -> anyhow::Result<&Page> {
        self.state = TabState::Loading;
        self.last_active = Instant::now();

        let result = if self.config.js_enabled {
            Page::from_url_with_js(app, &self.url, self.config.wait_ms).await
        } else {
            Page::from_url(app, &self.url).await
        };

        match result {
            Ok(page) => {
                self.title = page.title();
                self.url = page.url.clone();
                if self.history_index < self.history.len() - 1 {
                    self.history.truncate(self.history_index + 1);
                }
                if self.history.last() != Some(&self.url) {
                    self.history.push(self.url.clone());
                    self.history_index = self.history.len() - 1;
                }
                self.state = TabState::Ready;
                self.page = Some(page);
                Ok(self.page.as_ref().unwrap())
            }
            Err(e) => {
                self.state = TabState::Error(e.to_string());
                Err(e)
            }
        }
    }

    /// Navigate to a new URL within this tab (App-based)
    pub async fn navigate(&mut self, app: &Arc<App>, url: &str) -> anyhow::Result<&Page> {
        self.state = TabState::Navigating;
        self.url = url.to_string();
        self.page = None;
        self.load(app).await
    }

    /// Reload the current page (App-based)
    pub async fn reload(&mut self, app: &Arc<App>) -> anyhow::Result<&Page> {
        self.state = TabState::Loading;
        self.page = None;
        self.load(app).await
    }

    /// Go back in history (App-based)
    pub async fn go_back(&mut self, app: &Arc<App>) -> anyhow::Result<Option<&Page>> {
        if self.history_index > 0 {
            self.history_index -= 1;
            self.url = self.history[self.history_index].clone();
            self.page = None;
            Ok(Some(self.load(app).await?))
        } else {
            Ok(None)
        }
    }

    /// Go forward in history (App-based)
    pub async fn go_forward(&mut self, app: &Arc<App>) -> anyhow::Result<Option<&Page>> {
        if self.history_index < self.history.len() - 1 {
            self.history_index += 1;
            self.url = self.history[self.history_index].clone();
            self.page = None;
            Ok(Some(self.load(app).await?))
        } else {
            Ok(None)
        }
    }

    // -------------------------------------------------------------------
    // Browser integration methods (take reqwest::Client directly)
    // -------------------------------------------------------------------

    /// Load the page using a raw reqwest client.
    pub async fn load_with_client(
        &mut self,
        client: &reqwest::Client,
        network_log: &Arc<Mutex<pardus_debug::NetworkLog>>,
        js_enabled: bool,
        wait_ms: u32,
    ) -> anyhow::Result<&Page> {
        self.state = TabState::Loading;
        self.last_active = Instant::now();

        let app = Arc::new(App {
            http_client: client.clone(),
            config: BrowserConfig::default(),
            network_log: network_log.clone(),
        });

        let result = if js_enabled {
            Page::from_url_with_js(&app, &self.url, wait_ms).await
        } else {
            Page::from_url(&app, &self.url).await
        };

        match result {
            Ok(page) => {
                self.title = page.title();
                self.url = page.url.clone();
                if self.history_index < self.history.len() - 1 {
                    self.history.truncate(self.history_index + 1);
                }
                if self.history.last() != Some(&self.url) {
                    self.history.push(self.url.clone());
                    self.history_index = self.history.len() - 1;
                }
                self.state = TabState::Ready;
                self.page = Some(page);
                Ok(self.page.as_ref().unwrap())
            }
            Err(e) => {
                self.state = TabState::Error(e.to_string());
                Err(e)
            }
        }
    }

    /// Navigate to a new URL using a raw reqwest client.
    pub async fn navigate_with_client(
        &mut self,
        client: &reqwest::Client,
        network_log: &Arc<Mutex<pardus_debug::NetworkLog>>,
        url: &str,
        js_enabled: bool,
        wait_ms: u32,
    ) -> anyhow::Result<&Page> {
        self.state = TabState::Navigating;
        self.url = url.to_string();
        self.page = None;
        self.load_with_client(client, network_log, js_enabled, wait_ms).await
    }

    /// Reload using a raw reqwest client.
    pub async fn reload_with_client(
        &mut self,
        client: &reqwest::Client,
        network_log: &Arc<Mutex<pardus_debug::NetworkLog>>,
    ) -> anyhow::Result<&Page> {
        self.state = TabState::Loading;
        self.page = None;
        self.load_with_client(client, network_log, self.config.js_enabled, self.config.wait_ms).await
    }

    /// Update the tab with a new page (e.g., after click navigation).
    pub fn update_page(&mut self, page: Page) {
        self.title = page.title();
        self.url = page.url.clone();
        if self.history_index < self.history.len() - 1 {
            self.history.truncate(self.history_index + 1);
        }
        if self.history.last() != Some(&self.url) {
            self.history.push(self.url.clone());
            self.history_index = self.history.len() - 1;
        }
        self.state = TabState::Ready;
        self.page = Some(page);
    }

    // -------------------------------------------------------------------
    // Convenience query methods (delegate to Page)
    // -------------------------------------------------------------------

    /// Check if the tab can go back in history
    pub fn can_go_back(&self) -> bool {
        self.history_index > 0
    }

    /// Check if the tab can go forward in history
    pub fn can_go_forward(&self) -> bool {
        self.history_index < self.history.len() - 1
    }

    /// Get history length
    pub fn history_len(&self) -> usize {
        self.history.len()
    }

    /// Mark tab as active (updates last_active timestamp)
    pub fn activate(&mut self) {
        self.last_active = Instant::now();
    }

    /// Get formatted info for display
    pub fn info(&self) -> TabInfo {
        TabInfo {
            id: self.id,
            url: self.url.clone(),
            title: self.title.clone(),
            state: self.state.clone(),
            can_go_back: self.can_go_back(),
            can_go_forward: self.can_go_forward(),
            history_len: self.history.len(),
        }
    }

    /// Query the page for the first element matching a CSS selector.
    pub fn query(&self, selector: &str) -> Option<ElementHandle> {
        self.page.as_ref()?.query(selector)
    }

    /// Query the page for all elements matching a CSS selector.
    pub fn query_all(&self, selector: &str) -> Vec<ElementHandle> {
        self.page.as_ref().map(|p| p.query_all(selector)).unwrap_or_default()
    }

    /// Get the semantic tree of the current page.
    pub fn semantic_tree(&self) -> Option<SemanticTree> {
        self.page.as_ref().map(|p| p.semantic_tree())
    }

    /// Get the navigation graph of the current page.
    pub fn navigation_graph(&self) -> Option<NavigationGraph> {
        self.page.as_ref().map(|p| p.navigation_graph())
    }

    /// Get all interactive elements from the current page.
    pub fn interactive_elements(&self) -> Vec<ElementHandle> {
        self.page.as_ref().map(|p| p.interactive_elements()).unwrap_or_default()
    }
}

/// Serializable tab information for display/debugging
#[derive(Debug, Clone, serde::Serialize)]
pub struct TabInfo {
    pub id: TabId,
    pub url: String,
    pub title: Option<String>,
    pub state: TabState,
    pub can_go_back: bool,
    pub can_go_forward: bool,
    pub history_len: usize,
}
