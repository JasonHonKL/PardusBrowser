use scraper::{Html, Selector, ElementRef};
use serde::Serialize;
use std::sync::Arc;
use std::time::Instant;
use url::Url;

use crate::app::App;
use crate::semantic::tree::{SemanticTree, SemanticRole, SemanticNode};
use crate::navigation::graph::NavigationGraph;
use crate::interact::element::{ElementHandle, element_to_handle};

use pardus_debug::{NetworkRecord, ResourceType, Initiator};

/// Serializable snapshot of a page's state.
///
/// Used to transfer page data over the wire (e.g., via CDP WebSocket)
/// without exposing the non-serializable `scraper::Html` type.
#[derive(Debug, Clone, Serialize)]
pub struct PageSnapshot {
    pub url: String,
    pub status: u16,
    pub content_type: Option<String>,
    pub title: Option<String>,
    pub html: String,
}

pub struct Page {
    pub url: String,
    pub status: u16,
    pub content_type: Option<String>,
    pub html: Html,
    pub base_url: String,
}

impl Page {
    pub async fn from_url(app: &Arc<App>, url: &str) -> anyhow::Result<Self> {
        let start = Instant::now();

        let response = app.http_client.get(url).send().await?;
        let status = response.status().as_u16();
        let final_url = response.url().to_string();
        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        let resp_headers: Vec<(String, String)> = response
            .headers()
            .iter()
            .filter_map(|(k, v)| Some((k.to_string(), v.to_str().ok()?.to_string())))
            .collect();

        let body = response.text().await?;
        let body_size = body.len();
        let timing_ms = start.elapsed().as_millis();

        record_main_request(app, url, &final_url, status, &content_type, body_size, timing_ms, &resp_headers);

        let html = Html::parse_document(&body);
        let base_url = Self::extract_base_url(&html, &final_url);

        Ok(Self {
            url: final_url,
            status,
            content_type,
            html,
            base_url,
        })
    }

    #[cfg(feature = "js")]
    pub async fn from_url_with_js(app: &Arc<App>, url: &str, wait_ms: u32) -> anyhow::Result<Self> {
        let start = Instant::now();

        let response = app.http_client.get(url).send().await?;
        let status = response.status().as_u16();
        let final_url = response.url().to_string();
        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        let resp_headers: Vec<(String, String)> = response
            .headers()
            .iter()
            .filter_map(|(k, v)| Some((k.to_string(), v.to_str().ok()?.to_string())))
            .collect();

        let body = response.text().await?;
        let body_size = body.len();
        let timing_ms = start.elapsed().as_millis();

        record_main_request(app, url, &final_url, status, &content_type, body_size, timing_ms, &resp_headers);

        let base_url = Self::extract_base_url(&Html::parse_document(&body), &final_url);
        let final_body = crate::js::execute_js(&body, &base_url, wait_ms).await?;

        let html = Html::parse_document(&final_body);
        let base_url = Self::extract_base_url(&html, &final_url);

        Ok(Self {
            url: final_url,
            status,
            content_type,
            html,
            base_url,
        })
    }

    /// Returns an error indicating JS support is not compiled in.
    #[cfg(not(feature = "js"))]
    pub async fn from_url_with_js(_app: &Arc<App>, _url: &str, _wait_ms: u32) -> anyhow::Result<Self> {
        anyhow::bail!("JavaScript execution is not available — rebuild with --features js");
    }

    pub fn from_html(html_str: &str, url: &str) -> Self {
        let html = Html::parse_document(html_str);
        let base_url = Self::extract_base_url(&html, url);
        Self {
            url: url.to_string(),
            status: 200,
            content_type: Some("text/html".to_string()),
            html,
            base_url,
        }
    }

    pub fn title(&self) -> Option<String> {
        let selector = Selector::parse("title").ok()?;
        self.html
            .select(&selector)
            .next()
            .map(|el| el.text().collect::<String>().trim().to_string())
    }

    /// Find the first element matching a CSS selector.
    pub fn query(&self, selector: &str) -> Option<ElementHandle> {
        let sel = Selector::parse(selector).ok()?;
        let el = self.html.select(&sel).next()?;
        Some(element_to_handle(&el, &self.html))
    }

    /// Find all elements matching a CSS selector.
    pub fn query_all(&self, selector: &str) -> Vec<ElementHandle> {
        let sel = match Selector::parse(selector) {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };
        self.html
            .select(&sel)
            .map(|el| element_to_handle(&el, &self.html))
            .collect()
    }

    /// Find an element by its semantic role and optional name.
    pub fn find_by_role(&self, role: SemanticRole, name: Option<&str>) -> Option<ElementHandle> {
        let tree = self.semantic_tree();
        let node = find_node_by_role(&tree.root, &role, name)?;
        node_to_handle(&node, &self.html)
    }

    /// Find an element by its semantic action string and optional name.
    pub fn find_by_action(&self, action: &str, name: Option<&str>) -> Option<ElementHandle> {
        let tree = self.semantic_tree();
        let node = find_node_by_action(&tree.root, action, name)?;
        node_to_handle(&node, &self.html)
    }

    /// Get all interactive elements from the semantic tree.
    pub fn interactive_elements(&self) -> Vec<ElementHandle> {
        let tree = self.semantic_tree();
        let nodes = collect_interactive(&tree.root);
        nodes
            .into_iter()
            .filter_map(|node| node_to_handle(&node, &self.html))
            .collect()
    }

    /// Check if a CSS selector matches any element in the page.
    pub fn has_selector(&self, selector: &str) -> bool {
        Selector::parse(selector)
            .ok()
            .map(|s| self.html.select(&s).next().is_some())
            .unwrap_or(false)
    }

    /// Extract base URL from HTML (public version for form submission).
    pub(crate) fn extract_base_url_static(html: &Html, fallback: &str) -> String {
        Self::extract_base_url(html, fallback)
    }

    pub fn semantic_tree(&self) -> SemanticTree {
        SemanticTree::build(&self.html, &self.base_url)
    }

    /// Create a serializable snapshot of this page's state.
    pub fn snapshot(&self) -> PageSnapshot {
        PageSnapshot {
            url: self.url.clone(),
            status: self.status,
            content_type: self.content_type.clone(),
            title: self.title(),
            html: self.html.html(),
        }
    }

    pub fn navigation_graph(&self) -> NavigationGraph {
        NavigationGraph::build(&self.html, &self.url)
    }

    pub fn discover_subresources(&self, log: &Arc<std::sync::Mutex<pardus_debug::NetworkLog>>) {
        let start_id = {
            let log = log.lock().unwrap();
            log.next_id()
        };

        let subresources = pardus_debug::discover::discover_subresources(
            &self.html,
            &self.base_url,
            start_id,
        );

        let mut log = log.lock().unwrap();
        for record in subresources {
            log.push(record);
        }
    }

    pub async fn fetch_subresources(
        client: &reqwest::Client,
        log: &Arc<std::sync::Mutex<pardus_debug::NetworkLog>>,
    ) {
        pardus_debug::fetch::fetch_subresources(client, log, 6).await;
    }

    fn extract_base_url(html: &Html, fallback: &str) -> String {
        if let Ok(selector) = Selector::parse("base[href]") {
            if let Some(base_el) = html.select(&selector).next() {
                if let Some(href) = base_el.value().attr("href") {
                    if let Ok(resolved) = Url::parse(fallback)
                        .and_then(|base| base.join(href))
                    {
                        return resolved.to_string();
                    }
                }
            }
        }
        fallback.to_string()
    }
}

fn record_main_request(
    app: &Arc<App>,
    original_url: &str,
    final_url: &str,
    status: u16,
    content_type: &Option<String>,
    body_size: usize,
    timing_ms: u128,
    response_headers: &[(String, String)],
) {
    let mut record = NetworkRecord::fetched(
        1,
        "GET".to_string(),
        ResourceType::Document,
        "document · navigation".to_string(),
        final_url.to_string(),
        Initiator::Navigation,
    );
    record.status = Some(status);
    record.status_text = Some(http_status_text(status));
    record.content_type = content_type.clone();
    record.body_size = Some(body_size);
    record.timing_ms = Some(timing_ms);
    record.response_headers = response_headers.to_vec();

    if original_url != final_url {
        record.redirect_url = Some(final_url.to_string());
    }

    let mut log = app.network_log.lock().unwrap();
    log.push(record);
}

fn http_status_text(status: u16) -> String {
    match status {
        200 => "OK",
        301 => "Moved Permanently",
        302 => "Found",
        303 => "See Other",
        304 => "Not Modified",
        307 => "Temporary Redirect",
        308 => "Permanent Redirect",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        500 => "Internal Server Error",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        _ => "",
    }.to_string()
}

// ---------------------------------------------------------------------------
// Semantic tree search helpers
// ---------------------------------------------------------------------------

fn find_node_by_role<'a>(
    node: &'a SemanticNode,
    target_role: &SemanticRole,
    target_name: Option<&str>,
) -> Option<&'a SemanticNode> {
    if std::mem::discriminant(&node.role) == std::mem::discriminant(target_role) {
        match target_name {
            Some(name) => {
                if node.name.as_deref() == Some(name) {
                    return Some(node);
                }
            }
            None => return Some(node),
        }
    }
    for child in &node.children {
        if let Some(found) = find_node_by_role(child, target_role, target_name) {
            return Some(found);
        }
    }
    None
}

fn find_node_by_action<'a>(
    node: &'a SemanticNode,
    action: &str,
    target_name: Option<&str>,
) -> Option<&'a SemanticNode> {
    if node.action.as_deref() == Some(action) {
        match target_name {
            Some(name) => {
                if node.name.as_deref() == Some(name) {
                    return Some(node);
                }
            }
            None => return Some(node),
        }
    }
    for child in &node.children {
        if let Some(found) = find_node_by_action(child, action, target_name) {
            return Some(found);
        }
    }
    None
}

fn collect_interactive(node: &SemanticNode) -> Vec<&SemanticNode> {
    let mut result = Vec::new();
    if node.is_interactive {
        result.push(node);
    }
    for child in &node.children {
        result.extend(collect_interactive(child));
    }
    result
}

/// Try to find a scraper ElementRef matching a SemanticNode.
/// Uses tag, id, name, href, and text to locate the element.
fn node_to_handle(node: &SemanticNode, html: &Html) -> Option<ElementHandle> {
    let candidates = build_node_selectors(node);

    for candidate in candidates {
        if let Ok(sel) = Selector::parse(&candidate) {
            for el in html.select(&sel) {
                if element_matches_node(&el, node) {
                    return Some(element_to_handle(&el, html));
                }
            }
        }
    }

    None
}

fn build_node_selectors(node: &SemanticNode) -> Vec<String> {
    let mut selectors = Vec::new();

    // If the node has an href, try a[href="..."]
    if let Some(href) = &node.href {
        selectors.push(format!("{}[href=\"{}\"]", node.tag, href));
    }

    // Tag-based
    match node.tag.as_str() {
        "a" | "button" => {
            if let Some(_name) = &node.name {
                // Can't easily select by text content with CSS,
                // so just use tag
            }
        }
        "input" => {
            // Could try input[type="..."]
        }
        _ => {}
    }

    // Generic tag selector (last resort)
    selectors.push(node.tag.clone());

    selectors
}

fn element_matches_node(el: &ElementRef, node: &SemanticNode) -> bool {
    let tag = el.value().name();
    if tag != node.tag {
        return false;
    }

    // Check href for links
    if node.tag == "a" {
        if let Some(node_href) = &node.href {
            if el.value().attr("href") != Some(node_href.as_str()) {
                // The href might be resolved differently, but check anyway
            }
        }
    }

    // Check name for inputs
    if matches!(node.tag.as_str(), "input" | "select" | "textarea") {
        if let Some(node_name) = &node.name {
            if el.value().attr("name") != Some(node_name.as_str()) {
                return false;
            }
        }
    }

    true
}
