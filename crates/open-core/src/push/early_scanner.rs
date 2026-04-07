//! Early HTML scanner for HTTP/2 push simulation.
//!
//! [`EarlyScanner`] extracts the HTML `<head>` section from raw HTML bytes
//! using regex (avoiding the cost of full `scraper::Html` parsing), then runs
//! the [`PreloadScanner`](crate::parser::preload_scanner::PreloadScanner) on
//! that subset to discover critical subresources (stylesheets, scripts, fonts,
//! preload hints) that should be fetched proactively.

use regex::Regex;
use std::sync::OnceLock;
use tracing::trace;

use crate::parser::preload_scanner::{PreloadScanner, Priority, ResourceHint, ResourceType};
use crate::resource::{Resource, ResourceKind};

/// Result of early scanning: a prioritized list of resources to pre-fetch.
#[derive(Debug, Clone)]
pub struct EarlyScanResult {
    /// Resources sorted by priority (highest first).
    pub resources: Vec<Resource>,
    /// Raw hints discovered (for debugging/logging).
    pub hints: Vec<ResourceHint>,
}

/// Fast early scanner that extracts critical subresources from the HTML
/// `<head>` section before full DOM parsing begins.
pub struct EarlyScanner {
    head_re: Regex,
    preload_scanner: PreloadScanner,
}

impl EarlyScanner {
    pub fn new() -> Self {
        static HEAD_RE: OnceLock<Regex> = OnceLock::new();
        let head_re = HEAD_RE
            .get_or_init(|| Regex::new(r#"(?is)<head[^>]*>(.*?)</head>"#).unwrap())
            .clone();

        Self {
            head_re,
            preload_scanner: PreloadScanner::new(),
        }
    }

    /// Scan raw HTML bytes for critical subresources in the `<head>`.
    ///
    /// Returns resources sorted by priority. Only fetchable resources are
    /// included — `preconnect` and `dns-prefetch` hints are excluded because
    /// they are connection hints, not resource URLs.
    pub fn scan(&self, html: &str, base_url: &str) -> EarlyScanResult {
        let head_html = self.extract_head(html);
        let hints = self.preload_scanner.scan(head_html.as_bytes());
        trace!(
            "early scanner: found {} hints in <head> ({} bytes scanned)",
            hints.len(),
            head_html.len(),
        );

        let mut resources = Vec::with_capacity(hints.len());
        for hint in &hints {
            if should_prefetch(hint) {
                let resolved = resolve_url(base_url, &hint.url);
                let kind = resource_kind_from_hint(hint);
                let priority = priority_to_u8(hint.priority);
                resources.push(Resource {
                    url: resolved,
                    kind,
                    priority,
                    size_hint: None,
                });
            }
        }

        // Sort: lowest u8 = highest priority
        resources.sort_by_key(|r| r.priority);
        // Cap at a reasonable limit to avoid overwhelming the connection
        resources.truncate(32);

        EarlyScanResult { resources, hints }
    }

    /// Scan raw HTML and return only the resolved URLs (for quick checks).
    pub fn scan_urls(&self, html: &str, base_url: &str) -> Vec<String> {
        let result = self.scan(html, base_url);
        result.resources.into_iter().map(|r| r.url).collect()
    }

    fn extract_head<'a>(&self, html: &'a str) -> &'a str {
        // Try to match <head>...</head>
        if let Some(caps) = self.head_re.captures(html) {
            if let Some(m) = caps.get(1) {
                return m.as_str();
            }
        }
        // Fallback: if no </head> found, scan everything up to the first <body>
        static BODY_RE: OnceLock<Regex> = OnceLock::new();
        let body_re = BODY_RE.get_or_init(|| Regex::new(r#"(?i)<body[^>]*>"#).unwrap());
        if let Some(m) = body_re.find(html) {
            &html[..m.start()]
        } else {
            // No <body> tag — scan everything (might be a partial document)
            html
        }
    }
}

impl Default for EarlyScanner {
    fn default() -> Self {
        Self::new()
    }
}

fn should_prefetch(hint: &ResourceHint) -> bool {
    match hint.resource_type {
        // Always pre-fetch these
        ResourceType::Stylesheet | ResourceType::Script | ResourceType::Font => true,
        ResourceType::Image => hint.priority <= Priority::High,
        // Skip connection hints — they don't represent fetchable resources
        ResourceType::Document if hint.url.starts_with("http") => true,
        // Skip worker, manifest, other non-critical resources
        _ => false,
    }
}

fn resource_kind_from_hint(hint: &ResourceHint) -> ResourceKind {
    match hint.resource_type {
        ResourceType::Stylesheet => ResourceKind::Stylesheet,
        ResourceType::Script => ResourceKind::Script,
        ResourceType::Image => ResourceKind::Image,
        ResourceType::Font => ResourceKind::Font,
        ResourceType::Media => ResourceKind::Media,
        ResourceType::Document => ResourceKind::Document,
        _ => ResourceKind::Other,
    }
}

fn priority_to_u8(priority: Priority) -> u8 {
    match priority {
        Priority::Critical => 0,
        Priority::High => 32,
        Priority::Normal => 96,
        Priority::Low => 160,
        Priority::Lazy => 224,
    }
}

fn resolve_url(base: &str, relative: &str) -> String {
    if relative.starts_with("http://")
        || relative.starts_with("https://")
        || relative.starts_with("data:")
    {
        return relative.to_string();
    }
    url::Url::parse(base)
        .ok()
        .and_then(|b| b.join(relative).ok())
        .map(|u| u.to_string())
        .unwrap_or_else(|| relative.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scan_stylesheets() {
        let html = r#"<!DOCTYPE html><html><head>
            <link rel="stylesheet" href="/main.css">
            <link rel="stylesheet" href="/vendor.css">
        </head><body></body></html>"#;
        let scanner = EarlyScanner::new();
        let result = scanner.scan(html, "https://example.com");
        assert_eq!(result.resources.len(), 2);
        assert!(result
            .resources
            .iter()
            .any(|r| r.url == "https://example.com/main.css"));
        assert!(result
            .resources
            .iter()
            .any(|r| r.url == "https://example.com/vendor.css"));
        assert!(result
            .resources
            .iter()
            .all(|r| r.kind == ResourceKind::Stylesheet));
    }

    #[test]
    fn test_scan_scripts() {
        let html = r#"<!DOCTYPE html><html><head>
            <script src="/app.js"></script>
        </head><body></body></html>"#;
        let scanner = EarlyScanner::new();
        let result = scanner.scan(html, "https://example.com");
        assert_eq!(result.resources.len(), 1);
        assert_eq!(result.resources[0].url, "https://example.com/app.js");
        assert_eq!(result.resources[0].kind, ResourceKind::Script);
    }

    #[test]
    fn test_scan_preload_font() {
        let html = r#"<!DOCTYPE html><html><head>
            <link rel="preload" href="/font.woff2" as="font" crossorigin>
        </head><body></body></html>"#;
        let scanner = EarlyScanner::new();
        let result = scanner.scan(html, "https://example.com");
        assert_eq!(result.resources.len(), 1);
        assert_eq!(result.resources[0].kind, ResourceKind::Font);
    }

    #[test]
    fn test_scan_preload_image_critical() {
        let html = r#"<!DOCTYPE html><html><head>
            <link rel="preload" href="/hero.jpg" as="image">
        </head><body></body></html>"#;
        let scanner = EarlyScanner::new();
        let result = scanner.scan(html, "https://example.com");
        assert_eq!(result.resources.len(), 1);
        assert_eq!(result.resources[0].kind, ResourceKind::Image);
    }

    #[test]
    fn test_scan_skips_preconnect() {
        let html = r#"<!DOCTYPE html><html><head>
            <link rel="preconnect" href="https://cdn.example.com">
            <link rel="dns-prefetch" href="https://fonts.googleapis.com">
        </head><body></body></html>"#;
        let scanner = EarlyScanner::new();
        let result = scanner.scan(html, "https://example.com");
        assert_eq!(result.resources.len(), 0);
    }

    #[test]
    fn test_scan_priority_ordering() {
        let html = r#"<!DOCTYPE html><html><head>
            <link rel="stylesheet" href="/critical.css">
            <link rel="preload" href="/font.woff2" as="font">
            <script src="/app.js" defer></script>
        </head><body></body></html>"#;
        let scanner = EarlyScanner::new();
        let result = scanner.scan(html, "https://example.com");
        // Stylesheet should be first (priority 0), then font, then deferred script
        assert_eq!(result.resources[0].kind, ResourceKind::Stylesheet);
        assert!(result
            .resources
            .iter()
            .all(|r| r.priority <= result.resources.last().unwrap().priority));
    }

    #[test]
    fn test_scan_absolute_urls() {
        let html = r#"<!DOCTYPE html><html><head>
            <link rel="stylesheet" href="https://cdn.example.com/reset.css">
        </head><body></body></html>"#;
        let scanner = EarlyScanner::new();
        let result = scanner.scan(html, "https://example.com");
        assert_eq!(result.resources[0].url, "https://cdn.example.com/reset.css");
    }

    #[test]
    fn test_scan_no_head_tag() {
        let html = r#"<link rel="stylesheet" href="/fallback.css"><body>test</body>"#;
        let scanner = EarlyScanner::new();
        let result = scanner.scan(html, "https://example.com");
        // Fallback: scan until <body>
        assert!(result
            .resources
            .iter()
            .any(|r| r.url == "https://example.com/fallback.css"));
    }

    #[test]
    fn test_scan_empty_head() {
        let html = r#"<!DOCTYPE html><html><head></head><body>test</body></html>"#;
        let scanner = EarlyScanner::new();
        let result = scanner.scan(html, "https://example.com");
        assert!(result.resources.is_empty());
    }

    #[test]
    fn test_scan_max_32_resources() {
        let mut head = String::from("<head>");
        for i in 0..40 {
            head.push_str(&format!(
                r#"<link rel="stylesheet" href="/style{}.css">"#,
                i
            ));
        }
        head.push_str("</head>");
        let html = format!("<html>{}<body></body></html>", head);
        let scanner = EarlyScanner::new();
        let result = scanner.scan(&html, "https://example.com");
        assert_eq!(result.resources.len(), 32);
    }

    #[test]
    fn test_scan_urls_convenience() {
        let html = r#"<!DOCTYPE html><html><head>
            <link rel="stylesheet" href="/main.css">
        </head><body></body></html>"#;
        let scanner = EarlyScanner::new();
        let urls = scanner.scan_urls(html, "https://example.com");
        assert_eq!(urls, vec!["https://example.com/main.css"]);
    }

    #[test]
    fn test_scan_data_url_skipped() {
        let html = r#"<!DOCTYPE html><html><head>
            <link rel="stylesheet" href="data:text/css,body{}">
        </head><body></body></html>"#;
        let scanner = EarlyScanner::new();
        let result = scanner.scan(html, "https://example.com");
        // data: URLs are not resolved, they pass through as-is
        assert!(!result.resources.is_empty());
        assert_eq!(result.resources[0].url, "data:text/css,body{}");
    }
}
