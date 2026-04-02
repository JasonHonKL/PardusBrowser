//! CSP source expression types and matching logic.

use url::Url;
use super::directive::HashAlgorithm;

/// A CSP source expression (one token in a directive value).
#[derive(Debug, Clone)]
pub enum CspSource {
    /// `'none'` — nothing is allowed.
    None,
    /// `'self'` — same-origin.
    SelfOrigin,
    /// `'unsafe-inline'` — inline scripts/styles allowed.
    UnsafeInline,
    /// `'unsafe-eval'` — eval()/new Function() allowed.
    UnsafeEval,
    /// `'strict-dynamic'` — trust scripts loaded by trusted scripts.
    StrictDynamic,
    /// `'nonce-<base64>'` — nonce-based allowlist.
    Nonce { value: String },
    /// `'sha256-<base64>'` / `'sha384-<base64>'` / `'sha512-<base64>'`.
    Hash { algorithm: HashAlgorithm, value: String },
    /// Scheme source (e.g., `https:`, `http:`).
    Scheme { scheme: String },
    /// Host source with optional scheme, wildcard, port, and path.
    Host {
        scheme: Option<String>,
        host: String,
        port: Option<String>,
        path: Option<String>,
    },
}

/// Context needed for source matching.
pub struct SourceMatchContext<'a> {
    /// Origin of the page (for `'self'` matching).
    pub page_origin: &'a url::Origin,
    /// Nonce from the element being checked (if any).
    pub element_nonce: Option<&'a str>,
    /// Hash of the content being checked (if any, as base64 string).
    pub content_hash: Option<(&'a HashAlgorithm, &'a str)>,
    /// URL being fetched (for host/scheme matching).
    pub target_url: Option<&'a Url>,
}

impl CspSource {
    /// Parse a single CSP source token into a `CspSource`.
    pub fn parse(token: &str) -> Option<Self> {
        let trimmed = token.trim();
        if trimmed.is_empty() {
            return None;
        }

        // Keyword sources (quoted)
        if trimmed.starts_with('\'') && trimmed.ends_with('\'') {
            let inner = &trimmed[1..trimmed.len() - 1];

            // Check for nonce source inside quotes: 'nonce-<value>'
            if let Some(nonce_val) = inner.strip_prefix("nonce-") {
                return Some(Self::Nonce {
                    value: nonce_val.to_string(),
                });
            }

            // Check for hash source inside quotes: 'sha256-<value>', etc.
            for algo in ["sha256", "sha384", "sha512"] {
                if let Some(hash_val) = inner.strip_prefix(&format!("{}-", algo)) {
                    if let Some(algorithm) = HashAlgorithm::from_name(algo) {
                        return Some(Self::Hash {
                            algorithm,
                            value: hash_val.to_string(),
                        });
                    }
                }
            }

            return match inner {
                "none" => Some(Self::None),
                "self" => Some(Self::SelfOrigin),
                "unsafe-inline" => Some(Self::UnsafeInline),
                "unsafe-eval" => Some(Self::UnsafeEval),
                "strict-dynamic" => Some(Self::StrictDynamic),
                _ => None, // Unknown keyword, skip
            };
        }

        // Nonce source (unquoted fallback)
        if let Some(nonce_val) = trimmed.strip_prefix("nonce-") {
            return Some(Self::Nonce {
                value: nonce_val.to_string(),
            });
        }

        // Hash source
        for algo in ["sha256", "sha384", "sha512"] {
            if let Some(hash_val) = trimmed.strip_prefix(&format!("{}-", algo)) {
                if let Some(algorithm) = HashAlgorithm::from_name(algo) {
                    return Some(Self::Hash {
                        algorithm,
                        value: hash_val.to_string(),
                    });
                }
            }
        }

        // Scheme source (e.g., "https:", "http:", "data:")
        if trimmed.ends_with(':') && !trimmed.contains("://") {
            return Some(Self::Scheme {
                scheme: trimmed[..trimmed.len() - 1].to_lowercase(),
            });
        }

        // Host source: may include scheme, host (with optional *. prefix), port, path
        Self::parse_host_source(trimmed)
    }

    /// Parse a host source expression like `https://*.example.com:8080/path`.
    fn parse_host_source(token: &str) -> Option<Self> {
        let (scheme, rest) = if let Some(pos) = token.find("://") {
            (Some(token[..pos].to_lowercase()), &token[pos + 3..])
        } else {
            (None, token)
        };

        // Split off path
        let (host_port, path) = if let Some(slash_pos) = rest.find('/') {
            (&rest[..slash_pos], Some(rest[slash_pos..].to_string()))
        } else {
            (rest, None)
        };

        // Split off port
        let (host, port) = if let Some(colon_pos) = host_port.rfind(':') {
            // Make sure it's not an IPv6 address bracket
            let potential_port = &host_port[colon_pos + 1..];
            if potential_port.parse::<u16>().is_ok() || potential_port == "*" {
                (&host_port[..colon_pos], Some(potential_port.to_string()))
            } else {
                (host_port, None)
            }
        } else {
            (host_port, None)
        };

        if host.is_empty() {
            return None;
        }

        // Validate host: must be a valid hostname or *.hostname or * (for port wildcard)
        let host_lower = host.to_lowercase();
        if host_lower != "*" && !is_valid_host_pattern(&host_lower) {
            return None;
        }

        Some(Self::Host {
            scheme,
            host: host_lower,
            port,
            path,
        })
    }

    /// Check if this source expression allows the request/content
    /// given the match context.
    pub fn matches(&self, ctx: &SourceMatchContext<'_>) -> bool {
        match self {
            Self::None => false,

            Self::SelfOrigin => {
                if let Some(target) = ctx.target_url {
                    let target_origin = target.origin();
                    if &target_origin == ctx.page_origin {
                        return true;
                    }
                    // Allow wss:// to match https:// origin and ws:// to match http:// origin.
                    // CSP spec treats WebSocket schemes as matching the corresponding HTTP scheme for 'self'.
                    let page_serialized = ctx.page_origin.ascii_serialization();
                    let target_serialized = target_origin.ascii_serialization();
                    // wss://host -> https://host and ws://host -> http://host
                    let normalized_target = target_serialized
                        .strip_prefix("wss://")
                        .map(|rest| format!("https://{}", rest))
                        .unwrap_or_else(|| {
                            target_serialized
                                .strip_prefix("ws://")
                                .map(|rest| format!("http://{}", rest))
                                .unwrap_or(target_serialized.clone())
                        });
                    normalized_target == page_serialized
                } else {
                    false
                }
            }

            // These are flags checked by the evaluator, not source matches.
            // They return true here so the evaluator can detect their presence.
            Self::UnsafeInline | Self::UnsafeEval | Self::StrictDynamic => true,

            Self::Nonce { value } => {
                ctx.element_nonce.map_or(false, |n| n == value)
            }

            Self::Hash { algorithm, value } => {
                ctx.content_hash.map_or(false, |(algo, hash)| {
                    algo == algorithm && hash == value
                })
            }

            Self::Scheme { scheme } => {
                ctx.target_url.map_or(false, |url| {
                    url.scheme().eq_ignore_ascii_case(scheme)
                })
            }

            Self::Host {
                scheme: source_scheme,
                host: pattern,
                port: source_port,
                path: source_path,
            } => {
                let target = match ctx.target_url {
                    Some(t) => t,
                    None => return false,
                };

                // Scheme check
                if let Some(s) = source_scheme {
                    if !target.scheme().eq_ignore_ascii_case(s) {
                        return false;
                    }
                } else {
                    // Without explicit scheme, only match http/https
                    let scheme = target.scheme();
                    if scheme != "http" && scheme != "https" {
                        return false;
                    }
                }

                // Host check
                let target_host = match target.host_str() {
                    Some(h) => h.to_lowercase(),
                    None => return false,
                };

                if !host_matches(pattern, &target_host) {
                    return false;
                }

                // Port check
                if let Some(sp) = source_port {
                    let target_port = target.port_or_known_default().unwrap_or(0);
                    if sp == "*" {
                        // Wildcard port matches everything
                    } else if let Ok(sp_num) = sp.parse::<u16>() {
                        if target_port != sp_num {
                            return false;
                        }
                    } else {
                        return false;
                    }
                }

                // Path check
                if let Some(sp) = source_path {
                    let target_path = target.path();
                    if !path_matches(sp, target_path) {
                        return false;
                    }
                }

                true
            }
        }
    }

    /// Returns true if this is a flag source (`UnsafeInline`, `UnsafeEval`, `StrictDynamic`).
    pub fn is_flag(&self) -> bool {
        matches!(self, Self::UnsafeInline | Self::UnsafeEval | Self::StrictDynamic)
    }
}

/// Check if a host matches a CSP host pattern.
/// Supports exact match and wildcard prefix (`*.example.com`).
fn host_matches(pattern: &str, actual_host: &str) -> bool {
    let pattern_lower = pattern.to_lowercase();
    let host_lower = actual_host.to_lowercase();

    if pattern_lower == "*" {
        return true;
    }

    if pattern_lower.starts_with("*.") {
        let suffix = &pattern_lower[2..];
        // *.example.com matches example.com OR sub.example.com
        host_lower == suffix || host_lower.ends_with(&format!(".{}", suffix))
    } else {
        host_lower == pattern_lower
    }
}

/// Check if a target path matches a source path pattern.
/// The source path must be a prefix of the target path.
fn path_matches(source_path: &str, target_path: &str) -> bool {
    if source_path.is_empty() {
        return true;
    }
    // Exact match or source_path is a prefix
    target_path == source_path || target_path.starts_with(source_path)
}

/// Validate that a host pattern is syntactically reasonable.
fn is_valid_host_pattern(host: &str) -> bool {
    if host.is_empty() {
        return false;
    }
    // Allow *. prefix
    let check = if host.starts_with("*.") {
        &host[2..]
    } else if host == "*" {
        return true;
    } else {
        host
    };

    if check.is_empty() {
        return true; // just "*" was matched above, "*.something" passes
    }

    // Basic validation: only alphanumeric, dots, hyphens
    check.chars().all(|c| c.is_alphanumeric() || c == '.' || c == '-')
}

/// Compute the base64-encoded hash of content using the specified algorithm.
pub fn compute_hash(algorithm: &HashAlgorithm, content: &[u8]) -> String {
    use sha2::{Sha256, Sha384, Sha512, Digest};
    use base64::Engine;

    let hash = match algorithm {
        HashAlgorithm::Sha256 => Sha256::digest(content).to_vec(),
        HashAlgorithm::Sha384 => Sha384::digest(content).to_vec(),
        HashAlgorithm::Sha512 => Sha512::digest(content).to_vec(),
    };

    base64::engine::general_purpose::STANDARD.encode(&hash)
}

#[cfg(test)]
mod tests {
    use super::*;
    use url::Origin;

    fn make_origin(url_str: &str) -> url::Origin {
        Url::parse(url_str).unwrap().origin()
    }

    fn make_ctx<'a>(
        page_origin: &'a Origin,
        target_url: Option<&'a Url>,
    ) -> SourceMatchContext<'a> {
        SourceMatchContext {
            page_origin,
            element_nonce: None,
            content_hash: None,
            target_url,
        }
    }

    // ==================== Parse Tests ====================

    #[test]
    fn test_parse_keyword_sources() {
        assert!(matches!(CspSource::parse("'none'"), Some(CspSource::None)));
        assert!(matches!(CspSource::parse("'self'"), Some(CspSource::SelfOrigin)));
        assert!(matches!(CspSource::parse("'unsafe-inline'"), Some(CspSource::UnsafeInline)));
        assert!(matches!(CspSource::parse("'unsafe-eval'"), Some(CspSource::UnsafeEval)));
        assert!(matches!(CspSource::parse("'strict-dynamic'"), Some(CspSource::StrictDynamic)));
    }

    #[test]
    fn test_parse_unknown_keyword() {
        assert!(CspSource::parse("'unknown-keyword'").is_none());
    }

    #[test]
    fn test_parse_nonce() {
        match CspSource::parse("nonce-abc123") {
            Some(CspSource::Nonce { value }) => assert_eq!(value, "abc123"),
            other => panic!("Expected Nonce, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_hash() {
        match CspSource::parse("sha256-RFWPLDbv2BY+f9DYCZlZ2Rt6S3JuEjBjhMtHIv7sTmE=") {
            Some(CspSource::Hash { algorithm: HashAlgorithm::Sha256, value }) => {
                assert_eq!(value, "RFWPLDbv2BY+f9DYCZlZ2Rt6S3JuEjBjhMtHIv7sTmE=");
            }
            other => panic!("Expected Hash, got {:?}", other),
        }

        match CspSource::parse("sha384-abcdef=") {
            Some(CspSource::Hash { algorithm: HashAlgorithm::Sha384, .. }) => {}
            other => panic!("Expected Sha384 Hash, got {:?}", other),
        }

        match CspSource::parse("sha512-xyz123==") {
            Some(CspSource::Hash { algorithm: HashAlgorithm::Sha512, .. }) => {}
            other => panic!("Expected Sha512 Hash, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_scheme_source() {
        match CspSource::parse("https:") {
            Some(CspSource::Scheme { scheme }) => assert_eq!(scheme, "https"),
            other => panic!("Expected Scheme, got {:?}", other),
        }
        match CspSource::parse("data:") {
            Some(CspSource::Scheme { scheme }) => assert_eq!(scheme, "data"),
            other => panic!("Expected Scheme, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_host_source() {
        match CspSource::parse("example.com") {
            Some(CspSource::Host { scheme: None, host, port: None, path: None }) => {
                assert_eq!(host, "example.com");
            }
            other => panic!("Expected Host, got {:?}", other),
        }

        match CspSource::parse("https://example.com") {
            Some(CspSource::Host { scheme, host, .. }) => {
                assert_eq!(scheme.as_deref(), Some("https"));
                assert_eq!(host, "example.com");
            }
            other => panic!("Expected Host, got {:?}", other),
        }

        match CspSource::parse("*.example.com") {
            Some(CspSource::Host { host, .. }) => {
                assert_eq!(host, "*.example.com");
            }
            other => panic!("Expected Host, got {:?}", other),
        }

        match CspSource::parse("example.com:8080") {
            Some(CspSource::Host { port, .. }) => {
                assert_eq!(port.as_deref(), Some("8080"));
            }
            other => panic!("Expected Host, got {:?}", other),
        }

        match CspSource::parse("example.com/path") {
            Some(CspSource::Host { path, .. }) => {
                assert_eq!(path.as_deref(), Some("/path"));
            }
            other => panic!("Expected Host, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_empty() {
        assert!(CspSource::parse("").is_none());
        assert!(CspSource::parse("  ").is_none());
    }

    // ==================== Matching Tests ====================

    #[test]
    fn test_self_origin_match() {
        let origin = make_origin("https://example.com/page");
        let target = Url::parse("https://example.com/script.js").unwrap();
        let ctx = make_ctx(&origin, Some(&target));
        assert!(CspSource::SelfOrigin.matches(&ctx));
    }

    #[test]
    fn test_self_origin_no_match() {
        let origin = make_origin("https://example.com/page");
        let target = Url::parse("https://evil.com/script.js").unwrap();
        let ctx = make_ctx(&origin, Some(&target));
        assert!(!CspSource::SelfOrigin.matches(&ctx));
    }

    #[test]
    fn test_scheme_match() {
        let origin = make_origin("https://example.com");
        let target = Url::parse("https://cdn.example.com/script.js").unwrap();
        let ctx = make_ctx(&origin, Some(&target));
        let source = CspSource::Scheme { scheme: "https".to_string() };
        assert!(source.matches(&ctx));
    }

    #[test]
    fn test_host_exact_match() {
        let origin = make_origin("https://example.com");
        let target = Url::parse("https://cdn.example.com/script.js").unwrap();
        let ctx = make_ctx(&origin, Some(&target));
        let source = CspSource::Host {
            scheme: None,
            host: "cdn.example.com".to_string(),
            port: None,
            path: None,
        };
        assert!(source.matches(&ctx));
    }

    #[test]
    fn test_host_wildcard_match() {
        let origin = make_origin("https://example.com");
        let target = Url::parse("https://cdn.example.com/script.js").unwrap();
        let ctx = make_ctx(&origin, Some(&target));
        let source = CspSource::Host {
            scheme: None,
            host: "*.example.com".to_string(),
            port: None,
            path: None,
        };
        assert!(source.matches(&ctx));

        // Wildcard also matches the base domain itself
        let target2 = Url::parse("https://example.com/script.js").unwrap();
        let ctx2 = make_ctx(&origin, Some(&target2));
        assert!(source.matches(&ctx2));
    }

    #[test]
    fn test_host_wildcard_no_match() {
        let origin = make_origin("https://example.com");
        let target = Url::parse("https://evil.com/script.js").unwrap();
        let ctx = make_ctx(&origin, Some(&target));
        let source = CspSource::Host {
            scheme: None,
            host: "*.example.com".to_string(),
            port: None,
            path: None,
        };
        assert!(!source.matches(&ctx));
    }

    #[test]
    fn test_host_with_scheme() {
        let origin = make_origin("https://example.com");
        let target_ok = Url::parse("https://cdn.example.com/script.js").unwrap();
        let target_bad = Url::parse("http://cdn.example.com/script.js").unwrap();

        let source = CspSource::Host {
            scheme: Some("https".to_string()),
            host: "cdn.example.com".to_string(),
            port: None,
            path: None,
        };

        assert!(source.matches(&make_ctx(&origin, Some(&target_ok))));
        assert!(!source.matches(&make_ctx(&origin, Some(&target_bad))));
    }

    #[test]
    fn test_nonce_match() {
        let origin = make_origin("https://example.com");
        let target = Url::parse("https://example.com").unwrap();
        let ctx = SourceMatchContext {
            page_origin: &origin,
            element_nonce: Some("abc123"),
            content_hash: None,
            target_url: Some(&target),
        };
        let source = CspSource::Nonce { value: "abc123".to_string() };
        assert!(source.matches(&ctx));

        let ctx_bad = SourceMatchContext {
            page_origin: &origin,
            element_nonce: Some("wrong"),
            content_hash: None,
            target_url: Some(&target),
        };
        assert!(!source.matches(&ctx_bad));
    }

    #[test]
    fn test_hash_match() {
        let origin = make_origin("https://example.com");
        let target = Url::parse("https://example.com").unwrap();
        let hash_val = "RFWPLDbv2BY+f9DYCZlZ2Rt6S3JuEjBjhMtHIv7sTmE=";
        let ctx = SourceMatchContext {
            page_origin: &origin,
            element_nonce: None,
            content_hash: Some((&HashAlgorithm::Sha256, hash_val)),
            target_url: Some(&target),
        };
        let source = CspSource::Hash {
            algorithm: HashAlgorithm::Sha256,
            value: hash_val.to_string(),
        };
        assert!(source.matches(&ctx));
    }

    // ==================== compute_hash Tests ====================

    #[test]
    fn test_compute_hash_sha256() {
        let content = b"alert(1)";
        let hash = compute_hash(&HashAlgorithm::Sha256, content);
        // Should be valid base64
        use base64::Engine;
        assert!(base64::engine::general_purpose::STANDARD.decode(&hash).is_ok());
        // Should be consistent
        assert_eq!(hash, compute_hash(&HashAlgorithm::Sha256, content));
    }

    #[test]
    fn test_compute_hash_different_algorithms() {
        let content = b"test";
        let h256 = compute_hash(&HashAlgorithm::Sha256, content);
        let h384 = compute_hash(&HashAlgorithm::Sha384, content);
        let h512 = compute_hash(&HashAlgorithm::Sha512, content);
        // All different lengths
        assert!(h256.len() < h384.len());
        assert!(h384.len() < h512.len());
    }

    // ==================== Host Matching Helpers ====================

    #[test]
    fn test_host_matches_exact() {
        assert!(host_matches("example.com", "example.com"));
        assert!(!host_matches("example.com", "evil.com"));
    }

    #[test]
    fn test_host_matches_wildcard() {
        assert!(host_matches("*.example.com", "cdn.example.com"));
        assert!(host_matches("*.example.com", "deep.cdn.example.com"));
        assert!(host_matches("*.example.com", "example.com"));
        assert!(!host_matches("*.example.com", "notexample.com"));
    }

    #[test]
    fn test_host_matches_star() {
        assert!(host_matches("*", "anything.com"));
    }

    #[test]
    fn test_host_matches_case_insensitive() {
        assert!(host_matches("Example.COM", "example.com"));
        assert!(host_matches("*.Example.COM", "cdn.example.com"));
    }
}
