use crate::config::BrowserConfig;
use crate::interact::{ElementHandle, FormState, InteractionResult, ScrollDirection};
use pardus_debug::NetworkLog;
use std::sync::Arc;
use std::sync::Mutex;

pub struct App {
    pub http_client: reqwest::Client,
    pub config: BrowserConfig,
    pub network_log: Arc<Mutex<NetworkLog>>,
}

impl App {
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
        }
    }

    pub fn default() -> Arc<Self> {
        Arc::new(Self::new(BrowserConfig::default()))
    }

    /// Click on an element identified by handle.
    pub async fn click(
        self: &Arc<Self>,
        page: &crate::Page,
        handle: &ElementHandle,
    ) -> anyhow::Result<InteractionResult> {
        crate::interact::actions::click(self, page, handle).await
    }

    /// Click on an element identified by CSS selector.
    pub async fn click_selector(
        self: &Arc<Self>,
        page: &crate::Page,
        selector: &str,
    ) -> anyhow::Result<InteractionResult> {
        match page.query(selector) {
            Some(handle) => self.click(page, &handle).await,
            None => Ok(InteractionResult::ElementNotFound {
                selector: selector.to_string(),
                reason: "no element matches selector".to_string(),
            }),
        }
    }

    /// Type a value into a form field.
    pub fn type_text(
        page: &crate::Page,
        handle: &ElementHandle,
        value: &str,
    ) -> anyhow::Result<InteractionResult> {
        crate::interact::actions::type_text(page, handle, value)
    }

    /// Submit a form with the given field values.
    pub async fn submit_form(
        self: &Arc<Self>,
        page: &crate::Page,
        form_selector: &str,
        state: &FormState,
    ) -> anyhow::Result<InteractionResult> {
        crate::interact::form::submit_form(self, page, form_selector, state).await
    }

    /// Toggle a checkbox or radio.
    pub fn toggle(
        page: &crate::Page,
        handle: &ElementHandle,
    ) -> anyhow::Result<InteractionResult> {
        crate::interact::actions::toggle(page, handle)
    }

    /// Select an option in a <select> element.
    pub fn select_option(
        page: &crate::Page,
        handle: &ElementHandle,
        value: &str,
    ) -> anyhow::Result<InteractionResult> {
        crate::interact::actions::select_option(page, handle, value)
    }

    /// Wait for a CSS selector to appear.
    pub async fn wait_for_selector(
        self: &Arc<Self>,
        page: &crate::Page,
        selector: &str,
        timeout_ms: u32,
        interval_ms: u32,
    ) -> anyhow::Result<InteractionResult> {
        crate::interact::wait::wait_for_selector(self, page, selector, timeout_ms, interval_ms).await
    }

    /// Scroll the page.
    pub async fn scroll(
        self: &Arc<Self>,
        page: &crate::Page,
        direction: ScrollDirection,
    ) -> anyhow::Result<InteractionResult> {
        crate::interact::scroll::scroll(self, page, direction).await
    }
}
