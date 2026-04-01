//! Individual tab implementation
//!
//! A Tab wraps a Page and maintains tab-specific state.
//! Multiple tabs share the same App via Arc.

use std::sync::Arc;
use std::time::Instant;

use crate::Page;
use crate::app::App;

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

    /// Load the page content using the shared App
    ///
    /// This fetches the URL and builds the semantic tree.
    /// Updates state to Ready on success, Error on failure.
    pub async fn load(
        &mut self,
        app: &Arc<App>,
    ) -> anyhow::Result<&Page> {
        self.state = TabState::Loading;
        self.last_active = Instant::now();

        // Determine which Page::from_url method to use based on config
        let result = if self.config.js_enabled {
            Page::from_url_with_js(app, &self.url, self.config.wait_ms).await
        } else {
            Page::from_url(app, &self.url).await
        };

        match result {
            Ok(page) => {
                self.title = page.title();
                self.url = page.url.clone();
                // Update history
                if self.history_index < self.history.len() - 1 {
                    // Truncate forward history on new navigation
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

    /// Navigate to a new URL within this tab
    ///
    /// This is like load() but clears the current page first
    /// and preserves history.
    pub async fn navigate(
        &mut self,
        app: &Arc<App>,
        url: &str,
    ) -> anyhow::Result<&Page> {
        self.state = TabState::Navigating;
        self.url = url.to_string();
        self.page = None;
        self.load(app).await
    }

    /// Reload the current page
    pub async fn reload(
        &mut self,
        app: &Arc<App>,
    ) -> anyhow::Result<&Page> {
        self.state = TabState::Loading;
        self.page = None;
        self.load(app).await
    }

    /// Go back in history
    ///
    /// Returns true if navigation occurred, false if at beginning of history
    pub async fn go_back(
        &mut self,
        app: &Arc<App>,
    ) -> anyhow::Result<Option<&Page>> {
        if self.history_index > 0 {
            self.history_index -= 1;
            self.url = self.history[self.history_index].clone();
            self.page = None;
            Ok(Some(self.load(app).await?))
        } else {
            Ok(None)
        }
    }

    /// Go forward in history
    ///
    /// Returns true if navigation occurred, false if at end of history
    pub async fn go_forward(
        &mut self,
        app: &Arc<App>,
    ) -> anyhow::Result<Option<&Page>> {
        if self.history_index < self.history.len() - 1 {
            self.history_index += 1;
            self.url = self.history[self.history_index].clone();
            self.page = None;
            Ok(Some(self.load(app).await?))
        } else {
            Ok(None)
        }
    }

    /// Get the semantic tree of the current page
    ///
    /// Returns None if page not loaded or in error state
    pub fn semantic_tree(&self) -> Option<crate::semantic::tree::SemanticTree> {
        self.page.as_ref().map(|p| p.semantic_tree())
    }

    /// Get the navigation graph of the current page
    ///
    /// Returns None if page not loaded
    pub fn navigation_graph(&self) -> Option<crate::navigation::graph::NavigationGraph> {
        self.page.as_ref().map(|p| p.navigation_graph())
    }

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
