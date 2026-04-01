use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::protocol::event_bus::EventSender;
use crate::protocol::node_map::NodeMap;
use crate::protocol::target::CdpSession;
use crate::error::{CdpError, CdpErrorBody};
use crate::protocol::message::CdpErrorResponse;

/// A Send+Sync entry for a CDP target (tab).
/// Stores raw HTML rather than parsed Page to avoid !Send types from scraper.
#[derive(Debug, Clone)]
pub struct TargetEntry {
    pub url: String,
    pub html: Option<String>,
    pub title: Option<String>,
    pub js_enabled: bool,
}

/// Shared state available to all domain handlers. All fields are Send+Sync.
pub struct DomainContext {
    /// The App instance (HTTP client, config, network log).
    pub app: Arc<pardus_core::App>,
    /// Target store: target_id -> TargetEntry.
    pub targets: Arc<Mutex<HashMap<String, TargetEntry>>>,
    /// Event bus sender for pushing events to clients.
    pub event_tx: EventSender,
    /// Node map for this session (backendNodeId <-> selector).
    pub node_map: Arc<Mutex<NodeMap>>,
}

impl DomainContext {
    /// Get raw HTML for a target.
    pub fn get_html(&self, target_id: &str) -> Option<String> {
        let targets = self.targets.blocking_lock();
        targets.get(target_id).and_then(|e| e.html.clone())
    }

    /// Get URL for a target.
    pub fn get_url(&self, target_id: &str) -> Option<String> {
        let targets = self.targets.blocking_lock();
        targets.get(target_id).map(|e| e.url.clone())
    }

    /// Get title for a target.
    pub fn get_title(&self, target_id: &str) -> Option<String> {
        let targets = self.targets.blocking_lock();
        targets.get(target_id).and_then(|e| e.title.clone())
    }

    /// Fetch a URL, store the result.
    pub async fn navigate(&self, target_id: &str, url: &str) -> anyhow::Result<()> {
        let response = self.app.http_client.get(url).send().await?;
        let html = response.text().await?;

        let title = extract_title_from_html(&html);

        let entry = TargetEntry {
            url: url.to_string(),
            html: Some(html),
            title,
            js_enabled: false,
        };

        let mut targets = self.targets.lock().await;
        targets.insert(target_id.to_string(), entry);

        Ok(())
    }

    /// Update target entry with new page data.
    pub fn update_target(&self, target_id: &str, page: &pardus_core::Page) {
        let title = page.title();
        let html_str = page.html.html().to_string();
        let url = page.url.clone();
        let mut targets = self.targets.blocking_lock();
        targets.insert(target_id.to_string(), TargetEntry {
            url,
            html: Some(html_str),
            title,
            js_enabled: false,
        });
    }
}

/// Per-method result from a domain handler.
pub enum HandleResult {
    /// Command succeeded, return this result.
    Success(Value),
    /// Command failed with this error.
    Error(CdpErrorResponse),
    /// Domain was enabled/disabled (state change, no result data).
    Ack,
}

#[async_trait]
pub trait CdpDomainHandler: Send + Sync {
    /// The domain name (e.g., "Page", "Runtime", "DOM").
    fn domain_name(&self) -> &'static str;

    /// Handle a command within this domain.
    async fn handle(
        &self,
        method: &str,
        params: Value,
        session: &mut CdpSession,
        ctx: &DomainContext,
    ) -> HandleResult;
}

pub fn method_not_found(domain: &str, method: &str) -> HandleResult {
    HandleResult::Error(CdpErrorResponse {
        id: 0,
        error: CdpErrorBody::from(&CdpError::MethodNotFound(format!("{}.{} not found", domain, method))),
        session_id: None,
    })
}

pub mod browser;
pub mod console;
pub mod css;
pub mod dom;
pub mod emulation;
pub mod input;
pub mod log;
pub mod network;
pub mod pardus_ext;
pub mod page;
pub mod performance;
pub mod runtime;
pub mod security;
pub mod target;

/// Extract title from HTML using a simple regex.
fn extract_title_from_html(html: &str) -> Option<String> {
    // Simple regex to extract <title> content
    let start = html.find("<title>")? + 7;
    let end = html.find("</title>")?;
    Some(html[start..end].to_string())
}
