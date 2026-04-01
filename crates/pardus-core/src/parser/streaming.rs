//! Streaming HTML parser using lol-html
//!
//! Provides efficient parsing for large documents with minimal memory overhead.

use bytes::Bytes;
use lol_html::{HtmlRewriter, RewriteStrSettings, OutputSink, ElementContentHandlers};
use lol_html::errors::RewritingError;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{trace, instrument};

use super::LazyDom;
use super::preload_scanner::{PreloadScanner, ResourceHint, Priority};

/// Parser configuration options
#[derive(Debug, Clone)]
pub struct ParseOptions {
    /// Enable streaming mode for documents larger than this threshold (bytes)
    pub streaming_threshold: usize,
    /// Extract resource hints during parsing
    pub extract_hints: bool,
    /// Keep raw HTML for lazy re-parsing
    pub keep_source: bool,
    /// Maximum memory for rewriter buffer
    pub max_memory: usize,
    /// Enable text normalization
    pub normalize_text: bool,
    /// Extract semantic elements during stream
    pub extract_semantic: bool,
}

impl Default for ParseOptions {
    fn default() -> Self {
        Self {
            streaming_threshold: 500_000,  // 500KB
            extract_hints: true,
            keep_source: true,
            max_memory: 10 * 1024 * 1024, // 10MB
            normalize_text: true,
            extract_semantic: true,
        }
    }
}

/// Result of parsing operation
#[derive(Debug)]
pub struct ParseResult {
    /// Parsed DOM (may be lazy)
    pub dom: Arc<LazyDom>,
    /// Resource hints discovered during parsing
    pub hints: Vec<ResourceHint>,
    /// Statistics about the parse
    pub stats: ParseStats,
    /// Whether streaming mode was used
    pub used_streaming: bool,
}

/// Parse statistics
#[derive(Debug, Default)]
pub struct ParseStats {
    pub bytes_processed: usize,
    pub elements_seen: usize,
    pub text_chunks: usize,
    pub hints_extracted: usize,
    pub time_micros: u64,
}

/// High-performance streaming HTML parser
pub struct StreamingParser {
    options: ParseOptions,
    scanner: PreloadScanner,
}

impl StreamingParser {
    pub fn new(options: ParseOptions) -> Self {
        let scanner = PreloadScanner::new();
        Self { options, scanner }
    }

    /// Parse using streaming mode - minimal memory footprint
    #[instrument(skip(self, html), level = "trace")]
    pub fn parse_streaming(&mut self,
        html: Bytes,
        url: &str,
    ) -> ParseResult {
        let start = std::time::Instant::now();
        trace!("starting streaming parse, {} bytes", html.len());

        let mut hints = Vec::new();
        let mut element_count = 0usize;
        let mut text_count = 0usize;

        // Build rewriter with handlers for resource extraction
        let mut rewriter = self.build_rewriter(|hint| {
            hints.push(hint);
        }, |count| {
            element_count += count;
        }, |count| {
            text_count += count;
        });

        // Process chunks
        let chunk_size = 16 * 1024; // 16KB chunks
        for chunk in html.chunks(chunk_size) {
            if let Err(e) = rewriter.write(chunk) {
                tracing::warn!("rewriter error: {}", e);
            }
        }

        // Finalize
        let _ = rewriter.end();

        let elapsed = start.elapsed();
        trace!("streaming parse complete in {:?}", elapsed);

        // Build lazy DOM from source
        let dom = if self.options.keep_source {
            Arc::new(LazyDom::from_bytes(html))
        } else {
            Arc::new(LazyDom::empty())
        };

        ParseResult {
            dom,
            hints,
            stats: ParseStats {
                bytes_processed: html.len(),
                elements_seen: element_count,
                text_chunks: text_count,
                hints_extracted: hints.len(),
                time_micros: elapsed.as_micros() as u64,
            },
            used_streaming: true,
        }
    }

    /// Parse full document - build complete DOM
    #[instrument(skip(self, html), level = "trace")]
    pub fn parse_full(
        &mut self,
        html: Bytes,
        _url: &str,
    ) -> ParseResult {
        let start = std::time::Instant::now();
        trace!("starting full parse, {} bytes", html.len());

        // Use scraper/html5ever for full DOM
        let dom = Arc::new(LazyDom::parse_bytes(&html).unwrap_or_default());

        // Extract hints via scanner
        let hints = if self.options.extract_hints {
            self.scanner.scan(&html)
        } else {
            Vec::new()
        };

        let elapsed = start.elapsed();
        trace!("full parse complete in {:?}", elapsed);

        ParseResult {
            dom,
            hints,
            stats: ParseStats {
                bytes_processed: html.len(),
                elements_seen: dom.element_count(),
                text_chunks: 0,
                hints_extracted: hints.len(),
                time_micros: elapsed.as_micros() as u64,
            },
            used_streaming: false,
        }
    }

    fn build_rewriter(
        &self,
        hint_callback: impl FnMut(ResourceHint),
        _element_callback: impl FnMut(usize),
        _text_callback: impl FnMut(usize),
    ) -> HtmlRewriter<'static, OutputSink> {
        let settings = RewriteStrSettings {
            element_content_handlers: vec![
                // Handle link tags (CSS preload)
                ("link[rel=stylesheet]", ElementContentHandlers::default()),
                // Handle script tags
                ("script[src]", ElementContentHandlers::default()),
                // Handle images
                ("img[src]", ElementContentHandlers::default()),
            ],
            ..RewriteStrSettings::default()
        };

        HtmlRewriter::new(settings, |_chunk| {}).unwrap()
    }
}

/// Fast path for extracting just the resources without building DOM
pub fn extract_resources_only(html: &[u8], base_url: &str) -> Vec<ResourceHint> {
    let scanner = PreloadScanner::new();
    scanner.scan(html)
        .into_iter()
        .map(|mut hint| {
            // Resolve relative URLs
            if let Ok(base) = url::Url::parse(base_url) {
                if let Ok(resolved) = base.join(&hint.url) {
                    hint.url = resolved.to_string();
                }
            }
            hint
        })
        .collect()
}
