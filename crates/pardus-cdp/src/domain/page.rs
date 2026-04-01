use async_trait::async_trait;
use serde_json::Value;

use crate::domain::{method_not_found, CdpDomainHandler, DomainContext, HandleResult};
use crate::error::{CdpError, CdpErrorBody};
use crate::protocol::message::{CdpErrorResponse, CdpEvent};
use crate::protocol::target::CdpSession;

pub struct PageDomain;

fn invalid_params(msg: &str) -> HandleResult {
    HandleResult::Error(CdpErrorResponse {
        id: 0,
        error: CdpErrorBody::from(&CdpError::InvalidParams(msg.to_string())),
        session_id: None,
    })
}

fn server_error(msg: impl std::fmt::Display) -> HandleResult {
    HandleResult::Error(CdpErrorResponse {
        id: 0,
        error: CdpErrorBody::from(&CdpError::ServerError(msg.to_string())),
        session_id: None,
    })
}

fn now_timestamp() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
        * 1000.0
}

fn resolve_target_id(session: &CdpSession) -> &str {
    session.target_id.as_deref().unwrap_or("default")
}

#[async_trait]
impl CdpDomainHandler for PageDomain {
    fn domain_name(&self) -> &'static str {
        "Page"
    }

    async fn handle(
        &self,
        method: &str,
        params: Value,
        session: &mut CdpSession,
        ctx: &DomainContext,
    ) -> HandleResult {
        match method {
            "enable" => {
                session.enable_domain("Page");
                HandleResult::Ack
            }
            "disable" => {
                session.disable_domain("Page");
                HandleResult::Ack
            }
            "navigate" => {
                let url = params["url"].as_str().unwrap_or("");
                if url.is_empty() {
                    return invalid_params("Missing url parameter");
                }
                let target_id = resolve_target_id(session).to_string();

                match ctx.navigate(&target_id, url).await {
                    Ok(()) => {
                        let _ = ctx.event_tx.send(CdpEvent {
                            method: "Page.frameNavigated".to_string(),
                            params: serde_json::json!({
                                "frame": { "id": target_id, "url": url, "mimeType": "text/html" }
                            }),
                            session_id: Some(session.session_id.clone()),
                        });
                        let _ = ctx.event_tx.send(CdpEvent {
                            method: "Page.domContentEventFired".to_string(),
                            params: serde_json::json!({ "timestamp": now_timestamp() }),
                            session_id: Some(session.session_id.clone()),
                        });
                        let _ = ctx.event_tx.send(CdpEvent {
                            method: "Page.loadEventFired".to_string(),
                            params: serde_json::json!({ "timestamp": now_timestamp() }),
                            session_id: Some(session.session_id.clone()),
                        });

                        HandleResult::Success(serde_json::json!({ "frameId": target_id }))
                    }
                    Err(e) => server_error(e),
                }
            }
            "reload" => {
                let target_id = resolve_target_id(session).to_string();
                let url = {
                    let targets = ctx.targets.lock().await;
                    targets.get(&target_id).map(|t| t.url.clone()).unwrap_or_else(|| "about:blank".to_string())
                };
                match ctx.navigate(&target_id, &url).await {
                    Ok(()) => {
                        let _ = ctx.event_tx.send(CdpEvent {
                            method: "Page.loadEventFired".to_string(),
                            params: serde_json::json!({ "timestamp": now_timestamp() }),
                            session_id: Some(session.session_id.clone()),
                        });
                        HandleResult::Ack
                    }
                    Err(e) => server_error(e),
                }
            }
            "goBack" => HandleResult::Ack,
            "goForward" => HandleResult::Ack,
            "getFrameTree" => {
                let target_id = resolve_target_id(session).to_string();
                let targets = ctx.targets.lock().await;
                let (frame_id, url) = targets.get(&target_id)
                    .map(|t| (target_id.clone(), t.url.clone()))
                    .unwrap_or_else(|| ("main".to_string(), "about:blank".to_string()));
                HandleResult::Success(serde_json::json!({
                    "frameTree": {
                        "frame": { "id": frame_id, "url": url, "mimeType": "text/html" },
                        "childFrames": [],
                    }
                }))
            }
            _ => method_not_found("Page", method),
        }
    }
}
