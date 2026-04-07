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

/// Parse target HTML into DomDocument, apply a mutation, serialize back.
async fn mutate_dom<F>(
    ctx: &DomainContext,
    target_id: &str,
    f: F,
) -> HandleResult
where
    F: FnOnce(&mut open_core::js::dom::DomDocument, &NodeMap),
{
    let html_str = match ctx.get_html(target_id).await {
        Some(h) => h,
        None => return HandleResult::Ack,
    };
    let url = ctx.get_url(target_id).await.unwrap_or_default();

    let mut doc = open_core::js::dom::DomDocument::from_html(&html_str);
    let nm = ctx.node_map.lock().await;

    f(&mut doc, &nm);

    let new_html = doc.to_html();
    let title = doc.get_title();
    drop(nm);
    ctx.update_target_with_data(target_id, url, new_html, Some(title));

    HandleResult::Ack
}

#[async_trait(?Send)]
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
                let requested_frame_id = params["frameId"].as_str();
                let frame_tree_json = ctx.get_frame_tree_json(target_id).await;

                let (html_str, url) = if let Some(fid) = requested_frame_id {
                    match resolve_frame_html(fid, &frame_tree_json) {
                        Some(pair) => pair,
                        None => (ctx.get_html(target_id).await.unwrap_or_default(), ctx.get_url(target_id).await.unwrap_or_default()),
                    }
                } else {
                    (ctx.get_html(target_id).await.unwrap_or_default(), ctx.get_url(target_id).await.unwrap_or_default())
                };
                let mut nm = ctx.node_map.lock().await;
                let doc = if !html_str.is_empty() {
                    let page = open_core::Page::from_html(&html_str, &url);
                    build_document_tree(&page, &mut nm)
                } else {
                    empty_document(&mut nm)
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
                    if let (Some(html_str), Some(url)) = (ctx.get_html(target_id).await, ctx.get_url(target_id).await) {
                        let page = open_core::Page::from_html(&html_str, &url);
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
                let (html_str, url) = (ctx.get_html(target_id).await, ctx.get_url(target_id).await);
                let has_sel = match (html_str, url) {
                    (Some(html_str), Some(url)) => {
                        let page = open_core::Page::from_html(&html_str, &url);
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
                let (html_str, url) = (ctx.get_html(target_id).await, ctx.get_url(target_id).await);
                let mut nm = ctx.node_map.lock().await;

                let node_ids: Vec<i64> = match (html_str, url) {
                    (Some(html_str), Some(url)) => {
                        let page = open_core::Page::from_html(&html_str, &url);
                        page.query_all(selector).iter().enumerate().map(|(i, _)| {
                            let unique_key = format!("{}[{}]", selector, i);
                            nm.get_or_assign(&unique_key)
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

                let html = match (selector, ctx.get_html(target_id).await, ctx.get_url(target_id).await) {
                    (Some(sel), Some(html_str), Some(url)) => {
                        let page = open_core::Page::from_html(&html_str, &url);
                        let elements = page.query_all(&sel);
                        if !elements.is_empty() {
                            extract_outer_html(&html_str, &elements[0].selector)
                        } else {
                            String::new()
                        }
                    }
                    (None, _, _) => {
                        return HandleResult::Error(CdpErrorResponse {
                            id: 0,
                            error: crate::error::CdpErrorBody {
                                code: INVALID_PARAMS,
                                message: "No node specified".to_string(),
                            },
                            session_id: None,
                        });
                    }
                    _ => String::new(),
                };
                HandleResult::Success(serde_json::json!({
                    "outerHTML": html
                }))
            }
            "getInnerHTML" => {
                let node_id = params["backendNodeId"].as_i64()
                    .or(params["nodeId"].as_i64())
                    .unwrap_or(-1);
                let selector = {
                    let nm = ctx.node_map.lock().await;
                    nm.get_selector(node_id).map(|s| s.to_string())
                };

                let inner_html = match (selector, ctx.get_html(target_id).await) {
                    (Some(sel), Some(html_str)) => {
                        extract_inner_html(&html_str, &sel)
                    }
                    _ => String::new(),
                };
                HandleResult::Success(serde_json::json!({
                    "innerHTML": inner_html
                }))
            }
            "setAttributeValue" => {
                let node_id = params["nodeId"].as_i64().unwrap_or(-1);
                let attr_name = params["name"].as_str().unwrap_or("").to_string();
                let attr_value = params["value"].as_str().unwrap_or("").to_string();
                mutate_dom(ctx, target_id, |doc, nm| {
                    if let Some(selector) = nm.get_selector(node_id) {
                        if let Some(elem_id) = doc.query_selector(0, selector) {
                            doc.set_attribute(elem_id, &attr_name, &attr_value);
                        }
                    }
                }).await
            }
            "removeAttribute" => {
                let node_id = params["nodeId"].as_i64().unwrap_or(-1);
                let attr_name = params["name"].as_str().unwrap_or("").to_string();
                mutate_dom(ctx, target_id, |doc, nm| {
                    if let Some(selector) = nm.get_selector(node_id) {
                        if let Some(elem_id) = doc.query_selector(0, selector) {
                            doc.remove_attribute(elem_id, &attr_name);
                        }
                    }
                }).await
            }
            "removeNode" => {
                let node_id = params["nodeId"].as_i64().unwrap_or(-1);
                mutate_dom(ctx, target_id, |doc, nm| {
                    if let Some(selector) = nm.get_selector(node_id) {
                        if let Some(elem_id) = doc.query_selector(0, selector) {
                            if let Some(parent_id) = doc.get_parent(elem_id) {
                                doc.remove_child(parent_id, elem_id);
                            }
                        }
                    }
                }).await
            }
            "setNodeValue" => {
                let node_id = params["nodeId"].as_i64().unwrap_or(-1);
                let value = params["value"].as_str().unwrap_or("").to_string();
                mutate_dom(ctx, target_id, |doc, nm| {
                    if let Some(selector) = nm.get_selector(node_id) {
                        if let Some(elem_id) = doc.query_selector(0, selector) {
                            // For text nodes discovered as children of elements
                            let children = doc.get_children(elem_id);
                            for &child_id in &children {
                                if doc.get_node_type(child_id) == 3 {
                                    doc.set_node_value(child_id, &value);
                                    return;
                                }
                            }
                        }
                    }
                }).await
            }
            "setNodeName" => {
                let node_id = params["nodeId"].as_i64().unwrap_or(-1);
                let new_name = params["name"].as_str().unwrap_or("").to_string();
                mutate_dom(ctx, target_id, |doc, nm| {
                    if let Some(selector) = nm.get_selector(node_id) {
                        if let Some(elem_id) = doc.query_selector(0, selector) {
                            doc.set_node_name(elem_id, &new_name);
                        }
                    }
                }).await
            }
            "getBoxModel" => {
                HandleResult::Success(serde_json::json!({
                    "model": {
                        "content": [0, 0, 0, 0],
                        "padding": [0, 0, 0, 0],
                        "border": [0, 0, 0, 0],
                        "margin": [0, 0, 0, 0],
                        "width": 1280,
                        "height": 0,
                    }
                }))
            }
            "getNodeForLocation" => {
                let _x = params["x"].as_f64().unwrap_or(0.0);
                let _y = params["y"].as_f64().unwrap_or(0.0);
                let mut nm = ctx.node_map.lock().await;
                let backend_id = nm.get_or_assign("body");
                HandleResult::Success(serde_json::json!({
                    "backendNodeId": backend_id,
                    "nodeId": backend_id,
                    "frameId": resolve_target_id(session),
                }))
            }
            "highlightNode" => HandleResult::Ack,
            "hideHighlight" => HandleResult::Ack,
            "highlightRect" => HandleResult::Ack,
            "pushNodesByBackendIdsToFrontend" => {
                let ids = params["backendNodeIds"].as_array()
                    .map(|arr| arr.iter().filter_map(|v| v.as_i64()).collect::<Vec<_>>())
                    .unwrap_or_default();
                let nodes: Vec<Value> = ids.iter().map(|&id| {
                    serde_json::json!({ "nodeId": id, "backendNodeId": id })
                }).collect();
                HandleResult::Success(serde_json::json!({ "nodes": nodes }))
            }
            "resolveNode" => {
                let _object_id = params["objectId"].as_str().unwrap_or("");
                let mut nm = ctx.node_map.lock().await;
                let body_id = nm.get_or_assign("body");
                HandleResult::Success(serde_json::json!({
                    "object": {
                        "type": "object",
                        "subtype": "node",
                        "className": "HTMLBodyElement",
                        "description": "body",
                    },
                    "backendNodeId": body_id,
                }))
            }
            "requestNode" => {
                let _object_id = params["objectId"].as_str().unwrap_or("");
                let mut nm = ctx.node_map.lock().await;
                let body_id = nm.get_or_assign("body");
                HandleResult::Success(serde_json::json!({ "nodeId": body_id }))
            }
            "setFileInputFiles" => {
                let node_id = params["backendNodeId"].as_i64()
                    .or(params["nodeId"].as_i64())
                    .unwrap_or(-1);

                let selector = {
                    let nm = ctx.node_map.lock().await;
                    nm.get_selector(node_id).map(|s| s.to_string())
                };

                if let Some(selector) = selector {
                    let (html_str, url) = (ctx.get_html(target_id).await, ctx.get_url(target_id).await);
                    if let (Some(html_str), Some(url)) = (html_str, url) {
                        let page = open_core::Page::from_html(&html_str, &url);
                        if let Some(handle) = page.query(&selector) {
                            if handle.input_type.as_deref() == Some("file") || handle.action.as_deref() == Some("upload") {
                                let file_paths: Vec<std::path::PathBuf> = params["files"]
                                    .as_array()
                                    .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| std::path::PathBuf::from(s))).collect())
                                    .unwrap_or_default();

                                if file_paths.is_empty() {
                                    return HandleResult::Error(CdpErrorResponse {
                                        id: 0,
                                        error: crate::error::CdpErrorBody {
                                            code: INVALID_PARAMS,
                                            message: "No files specified".to_string(),
                                        },
                                        session_id: None,
                                    });
                                }

                                let max_size = 50 * 1024 * 1024;
                                match open_core::interact::upload::upload_files(&page, &handle, &file_paths, max_size) {
                                    Ok(files) => {
                                        let file_names: Vec<&str> = files.iter().map(|f| f.file_name.as_str()).collect();
                                        let count = file_names.len();
                                        return HandleResult::Success(serde_json::json!({
                                            "files": file_names,
                                            "count": count,
                                        }));
                                    }
                                    Err(e) => {
                                        return HandleResult::Error(CdpErrorResponse {
                                            id: 0,
                                            error: crate::error::CdpErrorBody {
                                                code: INVALID_PARAMS,
                                                message: e.to_string(),
                                            },
                                            session_id: None,
                                        });
                                    }
                                }
                            }
                        }
                    }
                }

                HandleResult::Error(CdpErrorResponse {
                    id: 0,
                    error: crate::error::CdpErrorBody {
                        code: INVALID_PARAMS,
                        message: "Node is not a file input".to_string(),
                    },
                    session_id: None,
                })
            }
            "getFileInfo" => {
                HandleResult::Error(CdpErrorResponse {
                    id: 0,
                    error: crate::error::CdpErrorBody {
                        code: SERVER_ERROR,
                        message: "getFileInfo not supported".to_string(),
                    },
                    session_id: None,
                })
            }
            "performSearch" => {
                let _query = params["query"].as_str().unwrap_or("");
                HandleResult::Success(serde_json::json!({
                    "resultCount": 0,
                    "searchId": format!("search-{}", uuid::Uuid::new_v4()),
                }))
            }
            "getSearchResults" => {
                let _search_id = params["searchId"].as_str().unwrap_or("");
                let from_index = params["fromIndex"].as_u64().unwrap_or(0);
                let _to_index = params["toIndex"].as_u64().unwrap_or(from_index);
                HandleResult::Success(serde_json::json!({
                    "nodeIds": [],
                }))
            }
            "discardSearchResults" => HandleResult::Ack,
            "requestChildNodes" => HandleResult::Ack,
            "collectClassNamesFromSubtree" => {
                let _node_id = params["nodeId"].as_i64().unwrap_or(-1);
                HandleResult::Success(serde_json::json!({
                    "classNames": []
                }))
            }
            "copyTo" => {
                let node_id = params["nodeId"].as_i64().unwrap_or(-1);
                let target_parent_id = params["targetNodeId"].as_i64().unwrap_or(-1);
                mutate_dom(ctx, target_id, |doc, nm| {
                    let source = nm.get_selector(node_id)
                        .and_then(|s| doc.query_selector(0, s));
                    let parent = nm.get_selector(target_parent_id)
                        .and_then(|s| doc.query_selector(0, s));
                    if let (Some(src_id), Some(par_id)) = (source, parent) {
                        doc.copy_to(src_id, par_id);
                    }
                }).await
            }
            "moveTo" => {
                let node_id = params["nodeId"].as_i64().unwrap_or(-1);
                let target_parent_id = params["targetNodeId"].as_i64().unwrap_or(-1);
                let before_id = params["insertBeforeNodeId"].as_i64();
                mutate_dom(ctx, target_id, |doc, nm| {
                    let source = nm.get_selector(node_id)
                        .and_then(|s| doc.query_selector(0, s));
                    let parent = nm.get_selector(target_parent_id)
                        .and_then(|s| doc.query_selector(0, s));
                    let before = before_id
                        .and_then(|id| nm.get_selector(id))
                        .and_then(|s| doc.query_selector(0, s));
                    if let (Some(src_id), Some(par_id)) = (source, parent) {
                        doc.move_to(src_id, par_id, before);
                    }
                }).await
            }
            "undo" => {
                mutate_dom(ctx, target_id, |doc, _nm| {
                    doc.undo();
                }).await
            }
            "redo" => {
                mutate_dom(ctx, target_id, |doc, _nm| {
                    doc.redo();
                }).await
            }
            "markUndoableState" => {
                mutate_dom(ctx, target_id, |doc, _nm| {
                    doc.mark_undoable_state();
                }).await
            }
            "focus" => HandleResult::Ack,
            "getFlattenedDocument" => {
                let (html_str, url) = (ctx.get_html(target_id).await, ctx.get_url(target_id).await);
                let mut nm = ctx.node_map.lock().await;
                let doc = match (html_str, url) {
                    (Some(html_str), Some(url)) => {
                        let page = open_core::Page::from_html(&html_str, &url);
                        build_document_tree(&page, &mut nm)
                    }
                    _ => empty_document(&mut nm),
                };
                HandleResult::Success(doc)
            }
            "getEmbeddedCSS" => HandleResult::Success(serde_json::json!({
                "embeddedCSS": []
            })),
            "getTopLayer" => HandleResult::Success(serde_json::json!({ "topLayerNodes": [] })),
            _ => method_not_found("DOM", method),
        }
    }
}

fn extract_outer_html(html: &str, selector: &str) -> String {
    let doc = scraper::Html::parse_document(html);
    let sel = match scraper::Selector::parse(selector) {
        Ok(s) => s,
        Err(_) => return String::new(),
    };
    match doc.select(&sel).next() {
        Some(el) => el.html(),
        None => String::new(),
    }
}

fn extract_inner_html(html: &str, selector: &str) -> String {
    let doc = scraper::Html::parse_document(html);
    let sel = match scraper::Selector::parse(selector) {
        Ok(s) => s,
        Err(_) => return String::new(),
    };
    match doc.select(&sel).next() {
        Some(el) => el.inner_html(),
        None => String::new(),
    }
}

fn build_document_tree(page: &open_core::Page, node_map: &mut NodeMap) -> Value {
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
        if let Some(ref action) = el.action {
            attrs.push(Value::String("data-action".to_string()));
            attrs.push(Value::String(action.clone()));
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
                "nodeId": doc_id,
                "backendNodeId": doc_id,
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

fn find_frame_by_id<'a>(frame: &'a serde_json::Value, id: &str) -> Option<&'a serde_json::Value> {
    let frame_id = frame.get("id")?.as_str()?;
    if frame_id == id {
        return Some(frame);
    }
    let children = frame.get("child_frames")?.as_array()?;
    for child in children {
        if let Some(found) = find_frame_by_id(child, id) {
            return Some(found);
        }
    }
    None
}

fn resolve_frame_html(frame_id: &str, frame_tree_json: &Option<String>) -> Option<(String, String)> {
    let json_str = frame_tree_json.as_ref()?;
    let tree: serde_json::Value = serde_json::from_str(json_str).ok()?;
    let frame = find_frame_by_id(&tree["root"], frame_id)?;
    let html = frame.get("html")?.as_str()?.to_string();
    let url = frame.get("url")?.as_str()?.to_string();
    Some((html, url))
}
