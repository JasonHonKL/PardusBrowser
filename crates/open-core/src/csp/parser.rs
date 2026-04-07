//! CSP header parsing.
//!
//! Parses `Content-Security-Policy` and `Content-Security-Policy-Report-Only`
//! header values into structured `CspPolicy` / `CspPolicySet` types.

use std::collections::HashMap;

use super::directive::{CspDirective, CspDirectiveKind};
use super::source::CspSource;

/// A fully parsed CSP policy (from one or more CSP headers).
#[derive(Debug, Clone, Default)]
pub struct CspPolicy {
    /// Directives keyed by kind.
    directives: HashMap<CspDirectiveKind, CspDirective>,
    /// Raw header value for reference in violation reports.
    raw: String,
}

impl CspPolicy {
    /// Parse a single CSP header value.
    ///
    /// Handles multiple directives separated by semicolons, each with
    /// a directive name and optional source list.
    pub fn parse(header_value: &str) -> Self {
        let mut directives = HashMap::new();
        let raw = header_value.to_string();

        for directive_str in header_value.split(';') {
            let trimmed = directive_str.trim();
            if trimmed.is_empty() {
                continue;
            }

            let mut tokens = trimmed.split_whitespace();
            let name = match tokens.next() {
                Some(n) => n,
                None => continue,
            };

            // Handle upgrade-insecure-requests (flag directive, no value)
            if name == "upgrade-insecure-requests" {
                directives.insert(
                    CspDirectiveKind::UpgradeInsecureRequests,
                    CspDirective {
                        kind: CspDirectiveKind::UpgradeInsecureRequests,
                        sources: Vec::new(),
                    },
                );
                continue;
            }

            let kind = match CspDirectiveKind::from_name(name) {
                Some(k) => k,
                None => continue, // Skip unknown directives
            };

            // Parse source tokens
            let sources: Vec<CspSource> = tokens
                .filter_map(|token| CspSource::parse(token))
                .collect();

            // For sandbox directive, parse sandbox tokens as special sources
            if kind == CspDirectiveKind::Sandbox {
                // Sandbox values are flags like allow-scripts, allow-forms, etc.
                // We store them as Scheme sources for simplicity (they're just flags).
                let flags: Vec<CspSource> = directive_str
                    .trim()
                    .split_whitespace()
                    .skip(1) // skip "sandbox"
                    .filter_map(|tok| {
                        // Store sandbox flags as Scheme sources (repurposed as flags)
                        Some(CspSource::Scheme { scheme: tok.to_string() })
                    })
                    .collect();

                directives.insert(
                    kind,
                    CspDirective {
                        kind,
                        sources: flags,
                    },
                );
                continue;
            }

            directives.insert(
                kind,
                CspDirective {
                    kind,
                    sources,
                },
            );
        }

        Self { directives, raw }
    }

    /// Get the effective source list for a directive.
    /// Falls back to `default-src` if the specific directive is absent.
    pub fn effective_sources(&self, kind: CspDirectiveKind) -> &[CspSource] {
        self.effective_sources_and_kind(kind).0
    }

    /// Get the effective source list and the directive kind that provided it.
    /// Returns the actual directive kind (may be the fallback, e.g. `default-src`).
    pub fn effective_sources_and_kind(&self, kind: CspDirectiveKind) -> (&[CspSource], CspDirectiveKind) {
        if let Some(directive) = self.directives.get(&kind) {
            return (&directive.sources, kind);
        }
        // Fallback to default-src for fetch directives
        if let Some(fallback) = kind.fallback() {
            if let Some(directive) = self.directives.get(&fallback) {
                return (&directive.sources, fallback);
            }
        }
        (&[], kind)
    }

    /// Check whether a directive is explicitly present in this policy.
    pub fn has_directive(&self, kind: CspDirectiveKind) -> bool {
        self.directives.contains_key(&kind)
    }

    /// Get the raw header value.
    pub fn raw(&self) -> &str {
        &self.raw
    }

    /// Returns true if the policy has no directives.
    pub fn is_empty(&self) -> bool {
        self.directives.is_empty()
    }
}

/// Holds both enforce and report-only policies extracted from response headers.
#[derive(Debug, Clone, Default)]
pub struct CspPolicySet {
    /// The enforcement policy (from `Content-Security-Policy` header).
    pub enforce: Option<CspPolicy>,
    /// The report-only policy (from `Content-Security-Policy-Report-Only` header).
    pub report_only: Option<CspPolicy>,
}

impl CspPolicySet {
    /// Parse CSP policies from a list of response headers.
    ///
    /// Looks for `content-security-policy` and `content-security-policy-report-only`
    /// headers (case-insensitive).
    pub fn from_headers(headers: &[(String, String)]) -> Self {
        let mut enforce_policies: Vec<CspPolicy> = Vec::new();
        let mut report_only_policies: Vec<CspPolicy> = Vec::new();

        for (name, value) in headers {
            let name_lower = name.to_lowercase();
            if name_lower == "content-security-policy" {
                enforce_policies.push(CspPolicy::parse(value));
            } else if name_lower == "content-security-policy-report-only" {
                report_only_policies.push(CspPolicy::parse(value));
            }
        }

        // Multiple CSP headers are combined: each must be satisfied independently.
        // For simplicity, we merge directives (union of source lists).
        // In a full implementation, each header would be checked independently.
        let enforce = if enforce_policies.is_empty() {
            None
        } else if enforce_policies.len() == 1 {
            Some(enforce_policies.into_iter().next().unwrap())
        } else {
            Some(Self::merge_policies(enforce_policies))
        };

        let report_only = if report_only_policies.is_empty() {
            None
        } else if report_only_policies.len() == 1 {
            Some(report_only_policies.into_iter().next().unwrap())
        } else {
            Some(Self::merge_policies(report_only_policies))
        };

        Self { enforce, report_only }
    }

    /// Create a policy set from a raw CSP header string (for override policy).
    pub fn from_raw(header_value: &str) -> Self {
        Self {
            enforce: Some(CspPolicy::parse(header_value)),
            report_only: None,
        }
    }

    /// Returns true if no policies are present.
    pub fn is_empty(&self) -> bool {
        self.enforce.is_none() && self.report_only.is_none()
    }

    /// Check if `upgrade-insecure-requests` is active.
    pub fn should_upgrade_insecure(&self) -> bool {
        self.enforce
            .as_ref()
            .map_or(false, |p| p.has_directive(CspDirectiveKind::UpgradeInsecureRequests))
    }

    /// Merge multiple policies by combining their directives.
    fn merge_policies(policies: Vec<CspPolicy>) -> CspPolicy {
        let mut merged_directives: HashMap<CspDirectiveKind, CspDirective> = HashMap::new();
        let raw_parts: Vec<&str> = Vec::new();

        for policy in &policies {
            for (kind, directive) in &policy.directives {
                merged_directives
                    .entry(*kind)
                    .and_modify(|existing: &mut CspDirective| {
                        // For flag directives, just keep one
                        if !kind.has_source_list() {
                            return;
                        }
                        // Merge source lists
                        existing.sources.extend(directive.sources.clone());
                    })
                    .or_insert_with(|| directive.clone());
            }
        }

        CspPolicy {
            directives: merged_directives,
            raw: raw_parts.join(", "),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== Basic Parsing Tests ====================

    #[test]
    fn test_parse_empty() {
        let policy = CspPolicy::parse("");
        assert!(policy.is_empty());
    }

    #[test]
    fn test_parse_whitespace_only() {
        let policy = CspPolicy::parse("   ;  ;  ");
        assert!(policy.is_empty());
    }

    #[test]
    fn test_parse_single_directive() {
        let policy = CspPolicy::parse("default-src 'self'");
        assert!(policy.has_directive(CspDirectiveKind::DefaultSrc));
        let sources = policy.effective_sources(CspDirectiveKind::DefaultSrc);
        assert_eq!(sources.len(), 1);
        assert!(matches!(&sources[0], CspSource::SelfOrigin));
    }

    #[test]
    fn test_parse_multiple_directives() {
        let policy = CspPolicy::parse(
            "default-src 'self'; script-src 'self' 'unsafe-inline'; style-src 'self'"
        );
        assert!(policy.has_directive(CspDirectiveKind::DefaultSrc));
        assert!(policy.has_directive(CspDirectiveKind::ScriptSrc));
        assert!(policy.has_directive(CspDirectiveKind::StyleSrc));

        let script_sources = policy.effective_sources(CspDirectiveKind::ScriptSrc);
        assert_eq!(script_sources.len(), 2);
    }

    #[test]
    fn test_parse_all_fetch_directives() {
        let policy = CspPolicy::parse(
            "default-src 'self'; script-src 'self'; style-src 'self'; \
             img-src 'self'; connect-src 'self'; font-src 'self'; \
             frame-src 'self'; media-src 'self'; object-src 'none'"
        );
        for kind in [
            CspDirectiveKind::DefaultSrc,
            CspDirectiveKind::ScriptSrc,
            CspDirectiveKind::StyleSrc,
            CspDirectiveKind::ImgSrc,
            CspDirectiveKind::ConnectSrc,
            CspDirectiveKind::FontSrc,
            CspDirectiveKind::FrameSrc,
            CspDirectiveKind::MediaSrc,
            CspDirectiveKind::ObjectSrc,
        ] {
            assert!(policy.has_directive(kind), "Missing: {:?}", kind);
        }
    }

    #[test]
    fn test_parse_unknown_directive_ignored() {
        let policy = CspPolicy::parse("unknown-src 'self'; script-src 'self'");
        assert!(!policy.has_directive(CspDirectiveKind::from_name("unknown-src").unwrap_or(CspDirectiveKind::DefaultSrc)));
        assert!(policy.has_directive(CspDirectiveKind::ScriptSrc));
    }

    // ==================== Source Expression Parsing ====================

    #[test]
    fn test_parse_nonce_source() {
        let policy = CspPolicy::parse("script-src 'nonce-abc123'");
        let sources = policy.effective_sources(CspDirectiveKind::ScriptSrc);
        assert_eq!(sources.len(), 1);
        assert!(matches!(&sources[0], CspSource::Nonce { value } if value == "abc123"));
    }

    #[test]
    fn test_parse_hash_source() {
        let policy = CspPolicy::parse("script-src 'sha256-RFWPLDbv2BY+f9DYCZlZ2Rt6S3JuEjBjhMtHIv7sTmE='");
        let sources = policy.effective_sources(CspDirectiveKind::ScriptSrc);
        assert_eq!(sources.len(), 1);
    }

    #[test]
    fn test_parse_host_sources() {
        let policy = CspPolicy::parse("script-src example.com *.cdn.com https://secure.com");
        let sources = policy.effective_sources(CspDirectiveKind::ScriptSrc);
        assert_eq!(sources.len(), 3);
    }

    #[test]
    fn test_parse_scheme_sources() {
        let policy = CspPolicy::parse("img-src https: data:");
        let sources = policy.effective_sources(CspDirectiveKind::ImgSrc);
        assert_eq!(sources.len(), 2);
    }

    // ==================== Flag Directives ====================

    #[test]
    fn test_parse_upgrade_insecure_requests() {
        let policy = CspPolicy::parse("upgrade-insecure-requests");
        assert!(policy.has_directive(CspDirectiveKind::UpgradeInsecureRequests));
    }

    #[test]
    fn test_parse_sandbox_with_flags() {
        let policy = CspPolicy::parse("sandbox allow-scripts allow-forms");
        assert!(policy.has_directive(CspDirectiveKind::Sandbox));
    }

    // ==================== Fallback Chain ====================

    #[test]
    fn test_fallback_to_default_src() {
        let policy = CspPolicy::parse("default-src 'self' https://cdn.com");
        // No script-src, should fall back to default-src
        let sources = policy.effective_sources(CspDirectiveKind::ScriptSrc);
        assert_eq!(sources.len(), 2); // 'self' + https://cdn.com
    }

    #[test]
    fn test_specific_overrides_default() {
        let policy = CspPolicy::parse("default-src 'self'; script-src 'none'");
        // script-src is present, so it's used instead of default-src
        let sources = policy.effective_sources(CspDirectiveKind::ScriptSrc);
        assert_eq!(sources.len(), 1);
        assert!(matches!(&sources[0], CspSource::None));
    }

    #[test]
    fn test_no_fallback_for_form_action() {
        let policy = CspPolicy::parse("default-src 'self'");
        // form-action has no fallback, so empty sources means "allow all"
        let sources = policy.effective_sources(CspDirectiveKind::FormAction);
        assert!(sources.is_empty());
    }

    // ==================== from_headers ====================

    #[test]
    fn test_from_headers_no_csp() {
        let headers = vec![
            ("content-type".to_string(), "text/html".to_string()),
            ("cache-control".to_string(), "no-cache".to_string()),
        ];
        let set = CspPolicySet::from_headers(&headers);
        assert!(set.is_empty());
    }

    #[test]
    fn test_from_headers_enforce_only() {
        let headers = vec![
            ("Content-Security-Policy".to_string(), "default-src 'self'".to_string()),
        ];
        let set = CspPolicySet::from_headers(&headers);
        assert!(set.enforce.is_some());
        assert!(set.report_only.is_none());
    }

    #[test]
    fn test_from_headers_both() {
        let headers = vec![
            ("Content-Security-Policy".to_string(), "default-src 'self'".to_string()),
            ("Content-Security-Policy-Report-Only".to_string(), "script-src 'none'".to_string()),
        ];
        let set = CspPolicySet::from_headers(&headers);
        assert!(set.enforce.is_some());
        assert!(set.report_only.is_some());
    }

    #[test]
    fn test_from_headers_case_insensitive() {
        let headers = vec![
            ("content-security-policy".to_string(), "default-src 'self'".to_string()),
        ];
        let set = CspPolicySet::from_headers(&headers);
        assert!(set.enforce.is_some());
    }

    #[test]
    fn test_from_raw() {
        let set = CspPolicySet::from_raw("default-src 'self'; script-src 'none'");
        assert!(set.enforce.is_some());
        assert!(set.report_only.is_none());
    }

    #[test]
    fn test_should_upgrade_insecure() {
        let set = CspPolicySet::from_raw("upgrade-insecure-requests");
        assert!(set.should_upgrade_insecure());

        let set = CspPolicySet::from_raw("default-src 'self'");
        assert!(!set.should_upgrade_insecure());
    }

    // ==================== Complex Policy ====================

    #[test]
    fn test_real_world_policy() {
        let policy = CspPolicy::parse(
            "default-src 'self'; \
             script-src 'self' 'nonce-abc123' https://cdn.example.com; \
             style-src 'self' 'unsafe-inline'; \
             img-src 'self' data: https:; \
             font-src 'self' https://fonts.gstatic.com; \
             connect-src 'self' https://api.example.com; \
             frame-ancestors 'none'; \
             base-uri 'self'; \
             form-action 'self'"
        );
        assert!(policy.has_directive(CspDirectiveKind::DefaultSrc));
        assert!(policy.has_directive(CspDirectiveKind::ScriptSrc));
        assert!(policy.has_directive(CspDirectiveKind::StyleSrc));
        assert!(policy.has_directive(CspDirectiveKind::ImgSrc));
        assert!(policy.has_directive(CspDirectiveKind::FontSrc));
        assert!(policy.has_directive(CspDirectiveKind::ConnectSrc));
        assert!(policy.has_directive(CspDirectiveKind::BaseUri));
        assert!(policy.has_directive(CspDirectiveKind::FormAction));

        // frame-ancestors is not in our enum, so it's ignored
        assert!(!policy.has_directive(CspDirectiveKind::FrameSrc));
    }
}
