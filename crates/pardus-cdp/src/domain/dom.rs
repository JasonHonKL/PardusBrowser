use async_trait::async_trait;
use serde_json::Value;

use crate::domain::{method_not_found, CdpDomainHandler, DomainContext, HandleResult};
use crate::error::{SERVER_ERROR, INVALID_PARAMS};
use crate::protocol::message::CdpErrorResponse;
use crate::protocol::node_map::NodeMap;
use crate::protocol::target::CdpSession;

pub struct DomDomain;

fn resolve_target_id(session: &CdpSession) -> &str {
    session.target_id.as_deref().unwrap_or("default")
}

#[async_trait]
impl CdpDomainHandler for DomDomain {
    fn domain_name(&self) -> &'static str {
        "DOM"
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
                session.enable_domain("DOM");
                HandleResult::Ack
            }
            "disable" => {
                session.disable_domain("DOM");
                HandleResult::Ack
            }
            "getDocument" => {
                let mut nm = ctx.node_map.lock().await;
                let doc = match (ctx.get_html(target_id), ctx.get_url(target_id)) {
                    (Some(html_str), Some(url)) => {
                        let page = pardus_core::Page::from_html(&html_str, &url);
                        build_document_tree(&page, &mut nm)
                    }
                    _ => empty_document(&mut nm),
                };
                HandleResult::Success(doc)
            }
            "describeNode" => {
                let node_id = params["backendNodeId"].as_i64()
                    .or(params["nodeId"].as_i64())
                    .unwrap_or(-1);
                let selector = {
                    let nm = ctx.node_map.lock().await;
                    nm.get_selector(node_id).map(|s| s.to_string())
                };

                if let Some(selector) = selector {
                    if let (Some(html_str), Some(url)) = (ctx.get_html(target_id), ctx.get_url(target_id)) {
                        let page = pardus_core::Page::from_html(&html_str, &url);
                        if let Some(el) = page.query(&selector) {
                            return HandleResult::Success(serde_json::json!({
                                "node": {
                                    "nodeId": node_id,
                                    "backendNodeId": node_id,
                                    "nodeType": 1,
                                    "nodeName": el.tag.to_uppercase(),
                                    "localName": el.tag,
                                    "childNodeCount": 0,
                                }
                            }));
                        }
                    }
                }
                HandleResult::Error(CdpErrorResponse {
                    id: 0,
                    error: crate::error::CdpErrorBody {
                        code: SERVER_ERROR,
                        message: format!("Node not found: {}", node_id),
                    },
                    session_id: None,
                })
            }
            "querySelector" => {
                let selector = params["selector"].as_str().unwrap_or("");
                if selector.is_empty() {
                    return HandleResult::Error(CdpErrorResponse {
                        id: 0,
                        error: crate::error::CdpErrorBody {
                            code: INVALID_PARAMS,
                            message: "Missing selector".to_string(),
                        },
                        session_id: None,
                    });
                }

                let mut nm = ctx.node_map.lock().await;
                let has_sel = match (ctx.get_html(target_id), ctx.get_url(target_id)) {
                    (Some(html_str), Some(url)) => {
                        let page = pardus_core::Page::from_html(&html_str, &url);
                        page.has_selector(selector)
                    }
                    _ => false,
                };
                if has_sel {
                    let node_id = nm.get_or_assign(selector);
                    HandleResult::Success(serde_json::json!({
                        "nodeId": node_id
                    }))
                } else {
                    HandleResult::Success(serde_json::json!({
                        "nodeId": 0
                    }))
                }
            }
            "querySelectorAll" => {
                let selector = params["selector"].as_str().unwrap_or("");
                let mut nm = ctx.node_map.lock().await;

                let node_ids: Vec<i64> = match (ctx.get_html(target_id), ctx.get_url(target_id)) {
                    (Some(html_str), Some(url)) => {
                        let page = pardus_core::Page::from_html(&html_str, &url);
                        page.query_all(selector).iter().map(|_| {
                            nm.get_or_assign(selector)
                        }).collect()
                    }
                    _ => vec![],
                };
                HandleResult::Success(serde_json::json!({
                    "nodeIds": node_ids
                }))
            }
            "getOuterHTML" => {
                let node_id = params["backendNodeId"].as_i64()
                    .or(params["nodeId"].as_i64())
                    .unwrap_or(-1);
                let selector = {
                    let nm = ctx.node_map.lock().await;
                    nm.get_selector(node_id).map(|s| s.to_string())
                };

                let html = match (selector, ctx.get_html(target_id), ctx.get_url(target_id)) {
                    (Some(sel), Some(html_str), Some(url)) => {
                        let page = pardus_core::Page::from_html(&html_str, &url);
                        let elements = page.query_all(&sel);
                        if !elements.is_empty() {
                            format!("<{}>...</{}>", elements[0].tag, elements[0].tag)
                        } else {
                            String::new()
                        }
                    }
                    (_, Some(html_str), _) => html_str,
                    _ => String::new(),
                };
                HandleResult::Success(serde_json::json!({
                    "outerHTML": html
                }))
            }
            "removeNode" => HandleResult::Ack,
            "pushNodesByBackendIdsToFrontend" => {
                let ids = params["backendNodeIds"].as_array()
                    .map(|arr| arr.iter().filter_map(|v| v.as_i64()).collect::<Vec<_>>())
                    .unwrap_or_default();
                let nodes: Vec<Value> = ids.iter().map(|&id| {
                    serde_json::json!({ "nodeId": id, "backendNodeId": id })
                }).collect();
                HandleResult::Success(serde_json::json!({ "nodes": nodes }))
            }
            _ => method_not_found("DOM", method),
        }
    }
}

fn build_document_tree(page: &pardus_core::Page, node_map: &mut NodeMap) -> Value {
    let doc_id = node_map.get_or_assign("html");
    let head_id = node_map.get_or_assign("head");
    let body_id = node_map.get_or_assign("body");

    let title = page.title().unwrap_or_default();

    let body_children: Vec<Value> = page.interactive_elements().iter().map(|el| {
        let el_id = node_map.get_or_assign(&el.selector);
        let mut attrs = Vec::new();
        if let Some(ref id) = el.id {
            attrs.push(Value::String("id".to_string()));
            attrs.push(Value::String(id.clone()));
        }
        if let Some(ref href) = el.href {
            attrs.push(Value::String("href".to_string()));
            attrs.push(Value::String(href.clone()));
        }
        if let Some(ref name) = el.name {
            attrs.push(Value::String("name".to_string()));
            attrs.push(Value::String(name.clone()));
        }
        serde_json::json!({
            "nodeId": el_id,
            "backendNodeId": el_id,
            "nodeType": 1,
            "nodeName": el.tag.to_uppercase(),
            "localName": el.tag,
            "childNodeCount": 0,
            "attributes": attrs,
        })
    }).collect();

    let html_id = node_map.get_or_assign("html");
    let title_id = node_map.get_or_assign("title");

    serde_json::json!({
        "root": {
            "nodeId": doc_id,
            "backendNodeId": doc_id,
            "nodeType": 9,
            "nodeName": "#document",
            "localName": "",
            "childNodeCount": 1,
            "children": [{
                "nodeId": html_id,
                "backendNodeId": html_id,
                "nodeType": 1,
                "nodeName": "HTML",
                "localName": "html",
                "childNodeCount": 2,
                "children": [
                    {
                        "nodeId": head_id,
                        "backendNodeId": head_id,
                        "nodeType": 1,
                        "nodeName": "HEAD",
                        "localName": "head",
                        "childNodeCount": 1,
                        "children": [{
                            "nodeId": title_id,
                            "backendNodeId": title_id,
                            "nodeType": 1,
                            "nodeName": "TITLE",
                            "localName": "title",
                            "childNodeCount": 0,
                        }],
                    },
                    {
                        "nodeId": body_id,
                        "backendNodeId": body_id,
                        "nodeType": 1,
                        "nodeName": "BODY",
                        "localName": "body",
                        "childNodeCount": body_children.len(),
                        "children": body_children,
                    },
                ],
            }],
            "documentURL": page.url,
            "baseURL": page.base_url,
            "title": title,
        }
    })
}

fn empty_document(node_map: &mut NodeMap) -> Value {
    let doc_id = node_map.get_or_assign("html");
    serde_json::json!({
        "root": {
            "nodeId": doc_id,
            "backendNodeId": doc_id,
            "nodeType": 9,
            "nodeName": "#document",
            "localName": "",
            "childNodeCount": 0,
            "children": [],
            "documentURL": "about:blank",
            "baseURL": "about:blank",
            "title": "",
        }
    })
}
