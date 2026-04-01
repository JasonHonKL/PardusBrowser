//! Fast regex-based preload scanner
//!
//! Scans HTML for resource hints without full parsing.
//! Runs in parallel with streaming parser.

use bytes::Bytes;
use regex::Regex;
use std::sync::OnceLock;
use smallvec::SmallVec;

/// Resource types for prioritization
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResourceType {
    Document,
    Stylesheet,
    Script,
    Image,
    Font,
    Media,
    Worker,
    Manifest,
    Other,
}

/// Priority hints for resource loading
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    Critical,    // Render-blocking
    High,        // Above-fold content
    Normal,      // Standard resources
    Low,         // Below-fold, deferred
    Lazy,        // Only when needed
}

/// A discovered resource hint
#[derive(Debug, Clone)]
pub struct ResourceHint {
    pub url: String,
    pub resource_type: ResourceType,
    pub priority: Priority,
    pub is_async: bool,
    pub is_defer: bool,
    pub is_module: bool,
    pub crossorigin: Option<String>,
}

/// Fast regex-based scanner for resource extraction
pub struct PreloadScanner {
    patterns: RegexSet,
}

use regex::RegexSet;

impl PreloadScanner {
    pub fn new() -> Self {
        let patterns = Self::build_patterns();
        Self { patterns }
    }

    fn build_patterns() -> RegexSet {
        RegexSet::new(&[
            // Link tags with various rel types
            r#"<link[^>]+href=["']?([^"'\s>]+)["']?[^>]*>"#,
            // Script tags
            r#"<script[^>]+src=["']?([^"'\s>]+)["']?[^>]*>"#,
            // Image tags
            r#"<img[^>]+src=["']?([^"'\s>]+)["']?[^>]*>"#,
            // Picture source
            r#"<source[^>]+srcset=["']?([^"'\s>]+)["']?[^>]*>"#,
            // Video/Audio sources
            r#"<(?:video|audio)[^>]+src=["']?([^"'\s>]+)["']?[^>]*>"#,
            // Preconnect hints
            r#"<link[^>]+rel=["']?preconnect["']?[^>]+href=["']?([^"'\s>]+)["']?"#,
            // DNS prefetch
            r#"<link[^>]+rel=["']?dns-prefetch["']?[^>]+href=["']?([^"'\s>]+)["']?"#,
            // Preload hints
            r#"<link[^>]+rel=["']?preload["']?[^>]+href=["']?([^"'\s>]+)["']?"#,
            // Modulepreload
            r#"<link[^>]+rel=["']?modulepreload["']?[^>]+href=["']?([^"'\s>]+)["']?"#,
            // Iframe src
            r#"<iframe[^>]+src=["']?([^"'\s>]+)["']?[^>]*>"#,
        ])
        .expect("valid regex patterns")
    }

    /// Scan HTML content and extract resource hints
    pub fn scan(&self,
        html: &[u8],
    ) -> Vec<ResourceHint> {
        let html_str = String::from_utf8_lossy(html);
        let mut hints = SmallVec::<[ResourceHint; 32]>::new();

        for mat in self.patterns.matches_iter(&html_str) {
            if let Some(url) = self.extract_url(mat.as_str()) {
                let hint = self.classify(mat.as_str(), url);
                hints.push(hint);
            }
        }

        hints.into_vec()
    }

    fn extract_url(&self, tag: &str) -> Option<String> {
        // Extract URL from attribute
        static HREF_RE: OnceLock<Regex> = OnceLock::new();
        let re = HREF_RE.get_or_init(|| {
            Regex::new(r#"(?:href|src|srcset)=["']?([^"'\s>]+)["']?"#).unwrap()
        });

        re.captures(tag)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().to_string())
    }

    fn classify(&self,
        tag: &str,
        url: String,
    ) -> ResourceHint {
        let lower = tag.to_lowercase();

        // Determine resource type
        let resource_type = if lower.contains("<link") {
            if lower.contains("rel=\"stylesheet\"") || lower.contains("rel='stylesheet'") || lower.contains("rel=stylesheet") {
                ResourceType::Stylesheet
            } else if lower.contains("rel=\"preload\"") {
                if lower.contains("as=\"font\"") || lower.contains("as='font'") {
                    ResourceType::Font
                } else if lower.contains("as=\"image\"") {
                    ResourceType::Image
                } else {
                    ResourceType::Other
                }
            } else if lower.contains("rel=\"manifest\"") {
                ResourceType::Manifest
            } else {
                ResourceType::Other
            }
        } else if lower.contains("<script") {
            ResourceType::Script
        } else if lower.contains("<img") {
            ResourceType::Image
        } else if lower.contains("<video") || lower.contains("<audio") {
            ResourceType::Media
        } else if lower.contains("<iframe") {
            ResourceType::Document
        } else {
            ResourceType::Other
        };

        // Determine priority
        let priority = if lower.contains("rel=\"preconnect\"") || lower.contains("rel=\"dns-prefetch\"") {
            Priority::Critical
        } else if resource_type == ResourceType::Stylesheet && !lower.contains("media=") {
            Priority::Critical
        } else if lower.contains("async") || lower.contains("defer") {
            Priority::Low
        } else if resource_type == ResourceType::Script {
            Priority::High
        } else {
            Priority::Normal
        };

        let is_async = lower.contains("async");
        let is_defer = lower.contains("defer");
        let is_module = lower.contains("type=\"module\"") || lower.contains("type='module'");

        let crossorigin = if lower.contains("crossorigin") {
            if lower.contains("crossorigin=\"use-credentials\"") || lower.contains("crossorigin='use-credentials'") {
                Some("use-credentials".to_string())
            } else {
                Some("anonymous".to_string())
            }
        } else {
            None
        };

        ResourceHint {
            url,
            resource_type,
            priority,
            is_async,
            is_defer,
            is_module,
            crossorigin,
        }
    }
}

impl Default for PreloadScanner {
    fn default() -> Self {
        Self::new()
    }
}

/// Sort hints by priority (critical first)
pub fn sort_by_priority(hints: &mut [ResourceHint]) {
    hints.sort_by_key(|h| std::cmp::Reverse(h.priority));
}

/// Filter hints by type
pub fn filter_by_type(hints: &[ResourceHint], ty: ResourceType) -> impl Iterator<Item = &ResourceHint> {
    hints.iter().filter(move |h| h.resource_type == ty)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scanner_extracts_stylesheets() {
        let html = r#"
            <link rel="stylesheet" href="/style.css">
            <link rel="stylesheet" href="https://example.com/other.css">
        "#;
        let scanner = PreloadScanner::new();
        let hints = scanner.scan(html.as_bytes());

        assert_eq!(hints.len(), 2);
        assert!(hints.iter().any(|h| h.url == "/style.css" && h.resource_type == ResourceType::Stylesheet));
    }

    #[test]
    fn test_scanner_extracts_scripts() {
        let html = r#"
            <script src="/app.js"></script>
            <script src="/async.js" async defer></script>
        "#;
        let scanner = PreloadScanner::new();
        let hints = scanner.scan(html.as_bytes());

        assert_eq!(hints.len(), 2);
        let async_hint = hints.iter().find(|h| h.url == "/async.js").unwrap();
        assert!(async_hint.is_async);
        assert!(async_hint.is_defer);
    }

    #[test]
    fn test_scanner_extracts_preconnect() {
        let html = r#"<link rel="preconnect" href="https://cdn.example.com">"#;
        let scanner = PreloadScanner::new();
        let hints = scanner.scan(html.as_bytes());

        assert_eq!(hints.len(), 1);
        assert_eq!(hints[0].url, "https://cdn.example.com");
        assert_eq!(hints[0].priority, Priority::Critical);
    }
}
