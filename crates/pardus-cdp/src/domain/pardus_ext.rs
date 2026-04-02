use async_trait::async_trait;
use serde_json::Value;

use crate::domain::{method_not_found, CdpDomainHandler, DomainContext, HandleResult};
use crate::error::SERVER_ERROR;
use crate::protocol::message::CdpErrorResponse;
use crate::protocol::target::CdpSession;

pub struct PardusDomain;

fn resolve_target_id(session: &CdpSession) -> &str {
    session.target_id.as_deref().unwrap_or("default")
}

async fn get_page_data(ctx: &DomainContext, target_id: &str) -> Option<(String, String)> {
    let html = ctx.get_html(target_id).await?;
    let url = ctx.get_url(target_id).await.unwrap_or_default();
    Some((html, url))
}

#[async_trait]
impl CdpDomainHandler for PardusDomain {
    fn domain_name(&self) -> &'static str {
        "Pardus"
    }

    async fn handle(
        &self,
        method: &str,
        params: Value,
        session: &mut CdpSession,
        ctx: &DomainContext,
    ) -> HandleResult {
        let target_id = resolve_target_id(session);

        match method {
            "enable" => {
                session.enable_domain("Pardus");
                HandleResult::Ack
            }
            "disable" => {
                session.disable_domain("Pardus");
                HandleResult::Ack
            }
            "semanticTree" => {
                match get_page_data(ctx, target_id).await {
                    Some((html_str, url)) => {
                        let page = pardus_core::Page::from_html(&html_str, &url);
                        let tree = page.semantic_tree();
                        let result = serde_json::to_value(&tree).unwrap_or(serde_json::json!({
                            "error": "Failed to serialize semantic tree"
                        }));
                        HandleResult::Success(serde_json::json!({
                            "semanticTree": result
                        }))
                    }
                    None => HandleResult::Error(CdpErrorResponse {
                        id: 0,
                        error: crate::error::CdpErrorBody {
                            code: SERVER_ERROR,
                            message: "No active page".to_string(),
                        },
                        session_id: None,
                    }),
                }
            }
            "interact" => {
                let action = params["action"].as_str().unwrap_or("").to_string();
                let selector = params["selector"].as_str().unwrap_or("").to_string();
                let value = params["value"].as_str().unwrap_or("").to_string();
                let fields_param = params.get("fields").cloned();

                let result = handle_interact(&action, &selector, &value, target_id, &fields_param, ctx).await;
                HandleResult::Success(result)
            }
            "getNavigationGraph" => {
                match get_page_data(ctx, target_id).await {
                    Some((html_str, url)) => {
                        let page = pardus_core::Page::from_html(&html_str, &url);
                        let graph = page.navigation_graph();
                        let result = serde_json::to_value(&graph).unwrap_or(serde_json::json!({
                            "error": "Failed to serialize navigation graph"
                        }));
                        HandleResult::Success(serde_json::json!({
                            "navigationGraph": result
                        }))
                    }
                    None => HandleResult::Error(CdpErrorResponse {
                        id: 0,
                        error: crate::error::CdpErrorBody {
                            code: SERVER_ERROR,
                            message: "No active page".to_string(),
                        },
                        session_id: None,
                    }),
                }
            }
            "detectActions" => {
                match get_page_data(ctx, target_id).await {
                    Some((html_str, url)) => {
                        let page = pardus_core::Page::from_html(&html_str, &url);
                        let elements = page.interactive_elements();
                        let actions: Vec<Value> = elements.iter().map(|el| {
                            serde_json::json!({
                                "selector": el.selector,
                                "tag": el.tag,
                                "action": el.action,
                                "label": el.label,
                                "href": el.href,
                                "disabled": el.is_disabled,
                            })
                        }).collect();
                        HandleResult::Success(serde_json::json!({
                            "actions": actions
                        }))
                    }
                    None => HandleResult::Error(CdpErrorResponse {
                        id: 0,
                        error: crate::error::CdpErrorBody {
                            code: SERVER_ERROR,
                            message: "No active page".to_string(),
                        },
                        session_id: None,
                    }),
                }
            }
            _ => method_not_found("Pardus", method),
        }
    }
}

async fn handle_interact(
    action: &str,
    selector: &str,
    value: &str,
    target_id: &str,
    fields_param: &Option<Value>,
    ctx: &DomainContext,
) -> Value {
    match action {
        "click" => {
            let page_data = get_page_data(ctx, target_id).await;
            let href = page_data.as_ref().and_then(|(html_str, url)| {
                let page = pardus_core::Page::from_html(&html_str, &url);
                page.query(selector).and_then(|el| el.href.clone())
            });

            if let Some(href) = href {
                match ctx.navigate(target_id, &href).await {
                    Ok(()) => serde_json::json!({ "success": true, "action": "click", "selector": selector }),
                    Err(e) => serde_json::json!({ "success": false, "error": e.to_string() }),
                }
            } else if let Some((html_str, url)) = page_data {
                let page = pardus_core::Page::from_html(&html_str, &url);
                let exists = page.query(selector).is_some();
                if exists {
                    serde_json::json!({ "success": true, "action": "click", "selector": selector, "note": "Element exists but is not a link" })
                } else {
                    serde_json::json!({ "success": false, "error": "Element not found" })
                }
            } else {
                serde_json::json!({ "success": false, "error": "No active page" })
            }
        }
        "type" => {
            match get_page_data(ctx, target_id).await {
                Some((html_str, url)) => {
                    let page = pardus_core::Page::from_html(&html_str, &url);
                    match page.query(selector) {
                        Some(handle) => {
                            match pardus_core::App::type_text(&page, &handle, value) {
                                Ok(_) => serde_json::json!({ "success": true, "action": "type", "selector": selector }),
                                Err(e) => serde_json::json!({ "success": false, "error": e.to_string() }),
                            }
                        }
                        None => serde_json::json!({ "success": false, "error": "Element not found" }),
                    }
                }
                None => serde_json::json!({ "success": false, "error": "No active page" }),
            }
        }
        "submit" => {
            let form_found = get_page_data(ctx, target_id).await
                .map(|(html_str, url)| {
                    let page = pardus_core::Page::from_html(&html_str, &url);
                    page.query(selector).is_some()
                })
                .unwrap_or(false);

            if form_found {
                let _ = fields_param;
                serde_json::json!({ "success": true, "action": "submit", "selector": selector, "note": "Form element found" })
            } else {
                serde_json::json!({ "success": false, "error": "Form not found" })
            }
        }
        _ => serde_json::json!({
            "success": false,
            "error": format!("Unknown action '{}'", action)
        }),
    }
}
