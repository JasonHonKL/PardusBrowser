//! Streaming HTML parser using lol-html
//!
//! Provides efficient parsing for large documents with minimal memory overhead.

use bytes::Bytes;
use std::sync::Arc;
use tracing::{trace, instrument};

use super::LazyDom;
use super::preload_scanner::{PreloadScanner, ResourceHint};

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
    pub fn parse_streaming(&mut self, html: Bytes, _url: &str) -> ParseResult {
        let start = std::time::Instant::now();
        let bytes_len = html.len();
        trace!("starting streaming parse, {} bytes", bytes_len);

        // Extract hints via scanner
        let hints = if self.options.extract_hints {
            self.scanner.scan(&html)
        } else {
            Vec::new()
        };
        let hints_len = hints.len();

        // Build lazy DOM from source
        let dom = if self.options.keep_source {
            Arc::new(LazyDom::from_bytes(html))
        } else {
            Arc::new(LazyDom::empty())
        };
        let element_count = dom.element_count();

        let elapsed = start.elapsed();
        trace!("streaming parse complete in {:?}", elapsed);

        ParseResult {
            dom,
            hints,
            stats: ParseStats {
                bytes_processed: bytes_len,
                elements_seen: element_count,
                text_chunks: 0,
                hints_extracted: hints_len,
                time_micros: elapsed.as_micros() as u64,
            },
            used_streaming: true,
        }
    }

    /// Parse full document - build complete DOM
    #[instrument(skip(self, html), level = "trace")]
    pub fn parse_full(&mut self, html: Bytes, _url: &str) -> ParseResult {
        let start = std::time::Instant::now();
        let bytes_len = html.len();
        trace!("starting full parse, {} bytes", bytes_len);

        // Extract hints via scanner
        let hints = if self.options.extract_hints {
            self.scanner.scan(&html)
        } else {
            Vec::new()
        };
        let hints_len = hints.len();

        // Use scraper/html5ever for full DOM
        let dom = Arc::new(LazyDom::parse_bytes(&html).unwrap_or_default());
        let element_count = dom.element_count();

        let elapsed = start.elapsed();
        trace!("full parse complete in {:?}", elapsed);

        ParseResult {
            dom,
            hints,
            stats: ParseStats {
                bytes_processed: bytes_len,
                elements_seen: element_count,
                text_chunks: 0,
                hints_extracted: hints_len,
                time_micros: elapsed.as_micros() as u64,
            },
            used_streaming: false,
        }
    }
}
