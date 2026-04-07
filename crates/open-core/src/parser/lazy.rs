//! Lazy DOM parsing
//!
//! Delays full DOM construction until needed.
//! Uses incremental parsing for selective node materialization.

use bytes::Bytes;
use parking_lot::RwLock;
use scraper::{Html, Selector, ElementRef};
use std::sync::{Arc, OnceLock};
use tracing::{trace, instrument};

/// Lazy HTML document - keeps source until full parse needed
#[derive(Debug)]
pub struct LazyHtml {
    source: Bytes,
    parsed: OnceLock<Html>,
}

impl LazyHtml {
    /// Create from raw HTML bytes
    pub fn from_bytes(source: Bytes) -> Self {
        Self {
            source,
            parsed: OnceLock::new(),
        }
    }

    /// Create from string
    pub fn from_string(source: impl Into<String>) -> Self {
        Self::from_bytes(Bytes::from(source.into()))
    }

    /// Get parsed HTML, parsing if needed
    pub fn parsed(&self) -> &Html {
        self.parsed.get_or_init(|| {
            trace!("lazily parsing {} bytes", self.source.len());
            Html::parse_document(std::str::from_utf8(&self.source).unwrap_or(""))
        })
    }

    /// Check if already parsed
    pub fn is_parsed(&self) -> bool {
        self.parsed.get().is_some()
    }

    /// Get source bytes
    pub fn source(&self) -> &Bytes {
        &self.source
    }

    /// Force parsing
    pub fn force_parse(&self) {
        let _ = self.parsed();
    }

    /// Memory usage
    pub fn memory_estimate(&self) -> usize {
        self.source.len() + if self.is_parsed() {
            // Estimate DOM memory at ~3x source size
            self.source.len() * 3
        } else {
            0
        }
    }
}

/// Lazy DOM that can be partially materialized
#[derive(Debug)]
pub struct LazyDom {
    /// Original source
    source: Bytes,
    /// Full scraper Html (lazy)
    full_dom: OnceLock<Html>,
    /// Element count estimate
    element_count: usize,
}

impl LazyDom {
    /// Create empty DOM
    pub fn empty() -> Self {
        Self {
            source: Bytes::new(),
            full_dom: OnceLock::new(),
            element_count: 0,
        }
    }

    /// Create from bytes (infallible, parses lazily)
    pub fn from_bytes(bytes: Bytes) -> Self {
        let html_str = std::str::from_utf8(&bytes).unwrap_or("");
        let element_count = html_str.matches('<').count();
        Self {
            source: bytes,
            full_dom: OnceLock::new(),
            element_count,
        }
    }

    /// Parse from bytes
    pub fn parse_bytes(bytes: &Bytes) -> anyhow::Result<Self> {
        let html_str = std::str::from_utf8(bytes)?;
        // Quick count of elements
        let element_count = html_str.matches('<').count();

        Ok(Self {
            source: bytes.clone(),
            full_dom: OnceLock::new(),
            element_count,
        })
    }

    /// Parse from string
    pub fn parse(html: &str) -> anyhow::Result<Arc<Self>> {
        let element_count = html.matches('<').count();
        let dom = Self {
            source: Bytes::from(html.to_string()),
            full_dom: OnceLock::new(),
            element_count,
        };
        Ok(Arc::new(dom))
    }

    /// Get full scraper Html, parsing if needed
    #[instrument(level = "trace", skip(self))]
    pub fn full_dom(&self) -> &Html {
        self.full_dom.get_or_init(|| {
            trace!("materializing full DOM, {} bytes", self.source.len());
            Html::parse_document(
                std::str::from_utf8(&self.source).unwrap_or("")
            )
        })
    }

    /// Check if full DOM is materialized
    pub fn is_materialized(&self) -> bool {
        self.full_dom.get().is_some()
    }

    /// Element count estimate
    pub fn element_count(&self) -> usize {
        self.element_count
    }

    /// Query with CSS selector (triggers full parse)
    pub fn select(&self, selector: &str) -> Option<ElementRef<'_>> {
        let html = self.full_dom();
        Selector::parse(selector)
            .ok()
            .and_then(|sel| html.select(&sel).next())
    }

    /// Extract text from specific element without full parse
    /// Uses fast path for simple selectors
    pub fn extract_text(&self, tag: &str) -> Option<String> {
        // For simple tag selectors, use regex extraction without full parse
        if !self.is_materialized() && !tag.contains([' ', '>', '+', '~', '#', '.', '[', ':']) {
            self.fast_extract_text(tag)
        } else {
            // Fall back to full DOM
            self.full_dom()
                .select(&Selector::parse(tag).ok()?)
                .next()
                .map(|el| el.text().collect())
        }
    }

    /// Fast text extraction without DOM construction
    fn fast_extract_text(&self, tag: &str) -> Option<String> {
        let html = std::str::from_utf8(&self.source).ok()?;
        // Use simple string search instead of regex for speed
        let start_tag = format!("<{}", tag);
        let end_tag = format!("</{}>", tag);
        
        if let Some(start_pos) = html.find(&start_tag) {
            let after_start = &html[start_pos + start_tag.len()..];
            if let Some(tag_end) = after_start.find('>') {
                let content_start = start_pos + start_tag.len() + tag_end + 1;
                if let Some(end_pos) = html[content_start..].find(&end_tag) {
                    return Some(html[content_start..content_start + end_pos].to_string());
                }
            }
        }
        None
    }

    /// Get source
    pub fn source(&self) -> &[u8] {
        &self.source
    }

    /// Memory usage estimate
    pub fn memory_estimate(&self) -> usize {
        self.source.len() + if self.is_materialized() {
            self.element_count * 128 // ~128 bytes per element
        } else {
            0
        }
    }
}

impl Default for LazyDom {
    fn default() -> Self {
        Self::empty()
    }
}

/// Trait for lazy parseable types
pub trait LazyParse {
    /// Type when fully parsed
    type Parsed;

    /// Check if parsed
    fn is_parsed(&self) -> bool;

    /// Force parsing
    fn force_parse(&self) -> &Self::Parsed;
}

impl LazyParse for LazyDom {
    type Parsed = Html;

    fn is_parsed(&self) -> bool {
        self.is_materialized()
    }

    fn force_parse(&self) -> &Self::Parsed {
        self.full_dom()
    }
}

/// Incremental parser for partial document parsing
pub struct IncrementalParser {
    chunks: Vec<Bytes>,
    #[allow(dead_code)]
    position: usize,
}

impl IncrementalParser {
    pub fn new() -> Self {
        Self {
            chunks: Vec::new(),
            position: 0,
        }
    }

    /// Feed a chunk
    pub fn feed(&mut self, chunk: Bytes) {
        self.chunks.push(chunk);
    }

    /// Parse completed portion
    pub fn parse_partial(&self) -> Option<Html> {
        let content: Vec<u8> = self.chunks.iter()
            .flat_map(|c| c.as_ref())
            .copied()
            .collect();

        let html_str = std::str::from_utf8(&content).ok()?;
        Some(Html::parse_fragment(html_str))
    }

    /// Check if complete document received
    pub fn is_complete(&self) -> bool {
        let content: Vec<u8> = self.chunks.iter()
            .flat_map(|c| c.as_ref())
            .copied()
            .collect();

        if let Ok(s) = std::str::from_utf8(&content) {
            s.contains("</html>") || s.contains("</HTML>")
        } else {
            false
        }
    }
}

/// Lazy semantic tree - computes nodes on demand
pub struct LazySemanticTree {
    html: Arc<LazyDom>,
    // Computed nodes cache
    computed_nodes: RwLock<std::collections::HashMap<String, serde_json::Value>>,
}

impl LazySemanticTree {
    pub fn new(html: Arc<LazyDom>) -> Self {
        Self {
            html,
            computed_nodes: RwLock::new(std::collections::HashMap::new()),
        }
    }

    /// Query semantic elements without full parse
    pub fn query_semantic(
        &self,
        role: &str,
    ) -> Vec<serde_json::Value> {
        // Check cache first
        if let Some(cached) = self.computed_nodes.read().get(role) {
            if let Some(arr) = cached.as_array() {
                return arr.clone();
            }
        }

        // Lazy compute - only materialize if needed
        let html = self.html.full_dom();

        // Extract by role attribute
        let results: Vec<_> = match role {
            "heading" => {
                let sel = Selector::parse("h1,h2,h3,h4,h5,h6").unwrap();
                html.select(&sel)
                    .map(|el| self.element_to_json(el))
                    .collect()
            }
            "link" => {
                let sel = Selector::parse("a[href]").unwrap();
                html.select(&sel)
                    .map(|el| self.element_to_json(el))
                    .collect()
            }
            "button" => {
                let sel = Selector::parse("button,input[type=submit],input[type=button]").unwrap();
                html.select(&sel)
                    .map(|el| self.element_to_json(el))
                    .collect()
            }
            _ => Vec::new(),
        };

        // Cache results
        self.computed_nodes.write().insert(
            role.to_string(),
            serde_json::json!(&results),
        );

        results
    }

    fn element_to_json(
        &self,
        el: ElementRef,
    ) -> serde_json::Value {
        let text: String = el.text().collect();
        let mut attrs = serde_json::Map::new();

        for attr in el.value().attrs() {
            attrs.insert(attr.0.to_string(), serde_json::Value::String(attr.1.to_string()));
        }

        serde_json::json!({
            "tag": el.value().name(),
            "text": text.trim(),
            "attributes": attrs,
        })
    }
}
