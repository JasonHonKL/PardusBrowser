use scraper::{Html, Selector, ElementRef};
use serde::Serialize;
use std::sync::Arc;
use std::time::Instant;
use url::Url;

use crate::app::App;
use crate::push::EarlyScanner;
use crate::resource::ResourceFetcher;
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
    /// CSP policy parsed from response headers (when CSP enforcement is enabled).
    pub csp: Option<crate::csp::CspPolicySet>,
}

impl Page {
    async fn fetch_html(app: &Arc<App>, url: &str) -> anyhow::Result<(String, u16, String, Option<String>, Option<crate::csp::CspPolicySet>)> {
        app.validate_url(url)?;

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

        // Sandbox: check max page size
        let config = app.config.read();
        if config.sandbox.max_page_size > 0 && body_size > config.sandbox.max_page_size {
            anyhow::bail!(
                "Page size ({} bytes) exceeds sandbox limit ({} bytes)",
                body_size,
                config.sandbox.max_page_size
            );
        }

        let timing_ms = start.elapsed().as_millis();

        record_main_request(app, url, &final_url, status, &content_type, body_size, timing_ms, &resp_headers);

        validate_content_type_pub(content_type.as_deref(), &final_url)?;

        // Sandbox: disable push if sandbox says so
        let push_enabled = config.push.enable_push && !config.sandbox.disable_push;

        // CSP: parse policy from response headers if enforcement is enabled
        let csp_policy = config.csp.parse_policy(&resp_headers);
        drop(config);

        spawn_push_fetches(&app.http_client, &body, &final_url, push_enabled);

        Ok((body, status, final_url, content_type, csp_policy))
    }

    #[must_use = "ignoring Result may silently swallow navigation errors"]
    pub async fn from_url(app: &Arc<App>, url: &str) -> anyhow::Result<Self> {
        let (body, status, final_url, content_type, csp) = Self::fetch_html(app, url).await?;

        let html = Html::parse_document(&body);
        let base_url = Self::extract_base_url(&html, &final_url, csp.as_ref());

        Ok(Self {
            url: final_url,
            status,
            content_type,
            html,
            base_url,
            csp,
        })
    }

    #[must_use = "ignoring Result may silently swallow navigation errors"]
    #[cfg(feature = "js")]
    pub async fn from_url_with_js(app: &Arc<App>, url: &str, wait_ms: u32) -> anyhow::Result<Self> {
        let (body, status, final_url, content_type, csp) = Self::fetch_html(app, url).await?;

        let base_url = Self::extract_base_url(&Html::parse_document(&body), &final_url, csp.as_ref());
        let final_body = crate::js::execute_js(&body, &base_url, wait_ms, Some(&app.config.read().sandbox)).await?;

        let html = Html::parse_document(&final_body);
        let base_url = Self::extract_base_url(&html, &final_url, csp.as_ref());

        Ok(Self {
            url: final_url,
            status,
            content_type,
            html,
            base_url,
            csp,
        })
    }

    /// Returns an error indicating JS support is not compiled in.
    #[cfg(not(feature = "js"))]
    pub async fn from_url_with_js(_app: &Arc<App>, _url: &str, _wait_ms: u32) -> anyhow::Result<Self> {
        anyhow::bail!("JavaScript execution is not available — rebuild with --features js");
    }

    pub fn from_html(html_str: &str, url: &str) -> Self {
        let html = Html::parse_document(html_str);
        let base_url = Self::extract_base_url(&html, url, None);
        Self {
            url: url.to_string(),
            status: 200,
            content_type: Some("text/html".to_string()),
            html,
            base_url,
            csp: None,
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

    /// Find an interactive element by its element ID (e.g., 1, 2, 3).
    /// This is the preferred way for AI agents to reference elements.
    pub fn find_by_element_id(&self, id: usize) -> Option<ElementHandle> {
        let tree = self.semantic_tree();
        let node = find_node_by_element_id(&tree.root, id)?;
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
        Self::extract_base_url(html, fallback, None)
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

    /// Create a shallow clone by re-parsing the HTML source.
    /// Needed because `scraper::Html` is not `Clone`.
    pub fn clone_shallow(&self) -> Self {
        Self {
            url: self.url.clone(),
            status: self.status,
            content_type: self.content_type.clone(),
            html: Html::parse_document(&self.html.html()),
            base_url: self.base_url.clone(),
            csp: self.csp.clone(),
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

    /// Resolve a URL relative to this page's base URL, preserving
    /// query parameters from the current URL when the relative URL
    /// contains only a query component (e.g., `?page=2`).
    ///
    /// Standard `Url::join` would replace all existing query params
    /// with the new ones. This method merges them instead.
    pub fn resolve_url_preserve_query(&self, href: &str) -> String {
        let base = match Url::parse(&self.base_url) {
            Ok(u) => u,
            Err(_) => return href.to_string(),
        };

        // If href is a query-only string (starts with "?"), merge params
        if href.starts_with('?') {
            let mut merged = base.clone();
            let relative = match Url::parse(&format!("https://dummy.com{}", href)) {
                Ok(u) => u,
                Err(_) => return base.join(href)
                    .map(|u| u.to_string())
                    .unwrap_or_else(|_| href.to_string()),
            };

            let mut pairs: Vec<(String, String)> = base
                .query_pairs()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect();

            for (k, v) in relative.query_pairs() {
                if let Some(existing) = pairs.iter_mut().find(|(ek, _)| *ek == k) {
                    existing.1 = v.to_string();
                } else {
                    pairs.push((k.to_string(), v.to_string()));
                }
            }

            {
                let mut qp = merged.query_pairs_mut();
                qp.clear();
                for (k, v) in &pairs {
                    qp.append_pair(k, v);
                }
            }
            return merged.to_string();
        }

        // For all other hrefs, standard resolution
        base.join(href)
            .map(|u| u.to_string())
            .unwrap_or_else(|_| href.to_string())
    }

    fn extract_base_url(html: &Html, fallback: &str, csp: Option<&crate::csp::CspPolicySet>) -> String {
        if let Ok(selector) = Selector::parse("base[href]") {
            if let Some(base_el) = html.select(&selector).next() {
                if let Some(href) = base_el.value().attr("href") {
                    if let Ok(resolved) = Url::parse(fallback)
                        .and_then(|base| base.join(href))
                    {
                        // CSP: check base-uri directive
                        if let Some(csp_policy) = csp {
                            if let Ok(fallback_url) = Url::parse(fallback) {
                                let origin = fallback_url.origin();
                                if let Ok(resolved_url) = Url::parse(&resolved.to_string()) {
                                    let check = csp_policy.check_base_uri(&origin, &resolved_url);
                                    if !check.allowed {
                                        if let Some(ref directive) = check.violated_directive {
                                            crate::csp::report_violation(&crate::csp::CspViolation {
                                                document_uri: fallback.to_string(),
                                                blocked_uri: resolved.to_string(),
                                                effective_directive: directive.clone(),
                                                original_policy: String::new(),
                                                disposition: crate::csp::Disposition::Enforce,
                                                status_code: 0,
                                            });
                                        }
                                        return fallback.to_string();
                                    }
                                }
                            }
                        }
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

/// Validate that the response content type is HTML-compatible.
/// Returns an error for binary or non-text responses (e.g. audio, images).
pub(crate) fn validate_content_type_pub(content_type: Option<&str>, url: &str) -> anyhow::Result<()> {
    if let Some(ct) = content_type {
        let ct_lower = ct.to_lowercase();
        let is_html = ct_lower.contains("text/html")
            || ct_lower.contains("application/xhtml")
            || ct_lower.contains("application/xml");
        let is_text = ct_lower.starts_with("text/");

        if !is_html && !is_text {
            anyhow::bail!(
                "Unsupported content type '{}' for URL '{}'. Expected HTML or text content.",
                ct.split(';').next().unwrap_or(ct).trim(),
                url
            );
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// HTTP/2 push simulation: speculative early resource fetching
// ---------------------------------------------------------------------------

fn spawn_push_fetches(
    client: &reqwest::Client,
    html_body: &str,
    base_url: &str,
    enabled: bool,
) {
    if !enabled {
        return;
    }

    let scanner = EarlyScanner::new();
    let result = scanner.scan(html_body, base_url);

    if result.resources.is_empty() {
        return;
    }

    let resources: Vec<crate::resource::Resource> = result.resources;
    let client = client.clone();

    tokio::spawn(async move {
        let config = crate::resource::ResourceConfig::default();
        let fetcher = ResourceFetcher::new(client, config);

        for resource in &resources {
            if let Err(e) = tokio::time::timeout(
                std::time::Duration::from_secs(5),
                fetcher.fetch(&resource.url),
            )
            .await
            {
                tracing::trace!("push fetch failed for {}: {}", resource.url, e);
            }
        }
    });
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

fn find_node_by_element_id<'a>(node: &'a SemanticNode, target_id: usize) -> Option<&'a SemanticNode> {
    if node.element_id == Some(target_id) {
        return Some(node);
    }
    for child in &node.children {
        if let Some(found) = find_node_by_element_id(child, target_id) {
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
/// Uses the pre-computed selector stored in the node for reliable lookup.
fn node_to_handle(node: &SemanticNode, html: &Html) -> Option<ElementHandle> {
    // Use the pre-computed selector if available
    if let Some(selector_str) = &node.selector {
        if let Ok(sel) = Selector::parse(selector_str) {
            if let Some(el) = html.select(&sel).next() {
                // Use the pre-computed selector to ensure consistency
                return Some(build_handle_with_selector(&el, selector_str.clone()));
            }
        }
    }

    // Fallback: try to build selectors from node attributes
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

/// Build an ElementHandle with a specific pre-computed selector.
fn build_handle_with_selector(el: &ElementRef, selector: String) -> ElementHandle {
    use crate::interact::element::{compute_action, compute_label};

    let tag = el.value().name().to_lowercase();
    let name_attr = el.value().attr("name").map(|s| s.to_string());
    let href = el.value().attr("href").map(|s| s.to_string());
    let input_type = el.value().attr("type").map(|s| s.to_string());
    let value = el.value().attr("value").map(|s| s.to_string());
    let id = el.value().attr("id").map(|s| s.to_string());
    let is_disabled = el.value().attr("disabled").is_some();

    let action = compute_action(&tag, input_type.as_deref());
    let label = compute_label(&tag, el);

    ElementHandle {
        selector,
        tag,
        id,
        name: name_attr,
        action,
        is_disabled,
        href,
        label,
        input_type,
        value,
    }
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
