use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::protocol::event_bus::EventBus;
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
    pub event_bus: Arc<EventBus>,
    /// Node map for this session (backendNodeId <-> selector).
    pub node_map: Arc<Mutex<NodeMap>>,
}

impl DomainContext {
    pub async fn get_html(&self, target_id: &str) -> Option<String> {
        let targets = self.targets.lock().await;
        targets.get(target_id).and_then(|e| e.html.clone())
    }

    pub async fn get_url(&self, target_id: &str) -> Option<String> {
        let targets = self.targets.lock().await;
        targets.get(target_id).map(|e| e.url.clone())
    }

    pub async fn get_title(&self, target_id: &str) -> Option<String> {
        let targets = self.targets.lock().await;
        targets.get(target_id).and_then(|e| e.title.clone())
    }

    pub async fn get_target_entry(&self, target_id: &str) -> Option<TargetEntry> {
        let targets = self.targets.lock().await;
        targets.get(target_id).cloned()
    }

    pub async fn navigate(&self, target_id: &str, url: &str) -> anyhow::Result<()> {
        let (final_url, html_str, title) = {
            let page = pardus_core::Page::from_url(&self.app, url).await?;
            (page.url, page.html.html().to_string(), page.title())
        };
        let mut targets = self.targets.lock().await;
        targets.insert(target_id.to_string(), TargetEntry {
            url: final_url,
            html: Some(html_str),
            title,
            js_enabled: false,
        });
        Ok(())
    }

    pub fn update_target_with_data(&self, target_id: &str, url: String, html: String, title: Option<String>) {
        let mut targets = self.targets.blocking_lock();
        targets.insert(target_id.to_string(), TargetEntry {
            url,
            html: Some(html),
            title,
            js_enabled: false,
        });
    }
}

pub enum HandleResult {
    Success(Value),
    Error(CdpErrorResponse),
    Ack,
}

impl HandleResult {
    pub fn with_request_id(self, id: u64) -> Self {
        match self {
            HandleResult::Success(v) => HandleResult::Success(v),
            HandleResult::Error(err) => HandleResult::Error(CdpErrorResponse {
                id,
                ..err
            }),
            HandleResult::Ack => HandleResult::Ack,
        }
    }
}

#[async_trait]
pub trait CdpDomainHandler: Send + Sync {
    fn domain_name(&self) -> &'static str;

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
