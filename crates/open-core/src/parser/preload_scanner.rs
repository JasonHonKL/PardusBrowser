//! Fast regex-based preload scanner
//!
//! Scans HTML for resource hints without full parsing.
//! Runs in parallel with streaming parser.

use regex::Regex;
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
    link_re: Regex,
    script_re: Regex,
    img_re: Regex,
    source_re: Regex,
    media_re: Regex,
    iframe_re: Regex,
}

impl PreloadScanner {
    pub fn new() -> Self {
        Self {
            link_re: Regex::new(r#"<link[^>]+href=["']?([^"'\s>]+)["']?[^>]*>"#).unwrap(),
            script_re: Regex::new(r#"<script[^>]+src=["']?([^"'\s>]+)["']?[^>]*>"#).unwrap(),
            img_re: Regex::new(r#"<img[^>]+src=["']?([^"'\s>]+)["']?[^>]*>"#).unwrap(),
            source_re: Regex::new(r#"<source[^>]+srcset=["']?([^"'\s>]+)["']?[^>]*>"#).unwrap(),
            media_re: Regex::new(r#"<(?:video|audio)[^>]+src=["']?([^"'\s>]+)["']?[^>]*>"#).unwrap(),
            iframe_re: Regex::new(r#"<iframe[^>]+src=["']?([^"'\s>]+)["']?[^>]*>"#).unwrap(),
        }
    }

    /// Scan HTML content and extract resource hints
    pub fn scan(&self, html: &[u8]) -> Vec<ResourceHint> {
        let html_str = String::from_utf8_lossy(html);
        let mut hints = SmallVec::<[ResourceHint; 32]>::new();

        // Extract from link tags
        for caps in self.link_re.captures_iter(&html_str) {
            if let Some(url) = caps.get(1) {
                let full_match = caps.get(0).unwrap().as_str();
                let hint = self.classify_link(full_match, url.as_str().to_string());
                hints.push(hint);
            }
        }

        // Extract from script tags
        for caps in self.script_re.captures_iter(&html_str) {
            if let Some(url) = caps.get(1) {
                let full_match = caps.get(0).unwrap().as_str();
                let hint = self.classify_script(full_match, url.as_str().to_string());
                hints.push(hint);
            }
        }

        // Extract from img tags
        for caps in self.img_re.captures_iter(&html_str) {
            if let Some(url) = caps.get(1) {
                hints.push(ResourceHint {
                    url: url.as_str().to_string(),
                    resource_type: ResourceType::Image,
                    priority: Priority::Normal,
                    is_async: false,
                    is_defer: false,
                    is_module: false,
                    crossorigin: None,
                });
            }
        }

        // Extract from source tags
        for caps in self.source_re.captures_iter(&html_str) {
            if let Some(url) = caps.get(1) {
                hints.push(ResourceHint {
                    url: url.as_str().to_string(),
                    resource_type: ResourceType::Image,
                    priority: Priority::Low,
                    is_async: false,
                    is_defer: false,
                    is_module: false,
                    crossorigin: None,
                });
            }
        }

        // Extract from media tags
        for caps in self.media_re.captures_iter(&html_str) {
            if let Some(url) = caps.get(1) {
                hints.push(ResourceHint {
                    url: url.as_str().to_string(),
                    resource_type: ResourceType::Media,
                    priority: Priority::Low,
                    is_async: false,
                    is_defer: false,
                    is_module: false,
                    crossorigin: None,
                });
            }
        }

        // Extract from iframe tags
        for caps in self.iframe_re.captures_iter(&html_str) {
            if let Some(url) = caps.get(1) {
                hints.push(ResourceHint {
                    url: url.as_str().to_string(),
                    resource_type: ResourceType::Document,
                    priority: Priority::Low,
                    is_async: false,
                    is_defer: false,
                    is_module: false,
                    crossorigin: None,
                });
            }
        }

        hints.into_vec()
    }

    fn classify_link(&self, tag: &str, url: String) -> ResourceHint {
        let lower = tag.to_lowercase();

        let resource_type = if lower.contains("rel=\"stylesheet\"") || lower.contains("rel='stylesheet'") {
            ResourceType::Stylesheet
        } else if lower.contains("rel=\"preload\"") {
            if lower.contains("as=\"font\"") {
                ResourceType::Font
            } else if lower.contains("as=\"image\"") {
                ResourceType::Image
            } else if lower.contains("as=\"script\"") {
                ResourceType::Script
            } else if lower.contains("as=\"style\"") {
                ResourceType::Stylesheet
            } else {
                ResourceType::Other
            }
        } else if lower.contains("rel=\"modulepreload\"") {
            ResourceType::Script
        } else if lower.contains("rel=\"manifest\"") {
            ResourceType::Manifest
        } else {
            ResourceType::Other
        };

        let priority = if lower.contains("rel=\"preconnect\"") || lower.contains("rel=\"dns-prefetch\"") {
            Priority::Critical
        } else if resource_type == ResourceType::Stylesheet {
            Priority::Critical
        } else if lower.contains("rel=\"preload\"") || lower.contains("rel=\"modulepreload\"") {
            Priority::High
        } else {
            Priority::Normal
        };

        let crossorigin = if lower.contains("crossorigin") {
            if lower.contains("use-credentials") {
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
            is_async: false,
            is_defer: false,
            is_module: lower.contains("modulepreload"),
            crossorigin,
        }
    }

    fn classify_script(&self, tag: &str, url: String) -> ResourceHint {
        let lower = tag.to_lowercase();

        let is_async = lower.contains("async");
        let is_defer = lower.contains("defer");
        let is_module = lower.contains("type=\"module\"") || lower.contains("type='module'");

        let priority = if is_async || is_defer {
            Priority::Low
        } else {
            Priority::High
        };

        let crossorigin = if lower.contains("crossorigin") {
            if lower.contains("use-credentials") {
                Some("use-credentials".to_string())
            } else {
                Some("anonymous".to_string())
            }
        } else {
            None
        };

        ResourceHint {
            url,
            resource_type: ResourceType::Script,
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

        let async_hint = hints.iter().find(|h| h.url == "/async.js").unwrap();
        assert!(async_hint.is_async);
        assert!(async_hint.is_defer);
    }

    #[test]
    fn test_scanner_extracts_preconnect() {
        let html = r#"<link rel="preconnect" href="https://cdn.example.com">"#;
        let scanner = PreloadScanner::new();
        let hints = scanner.scan(html.as_bytes());

        assert_eq!(hints[0].url, "https://cdn.example.com");
        assert_eq!(hints[0].priority, Priority::Critical);
    }
}
