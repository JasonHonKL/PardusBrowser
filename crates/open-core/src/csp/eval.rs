//! CSP policy evaluation.
//!
//! Provides `check_*` methods on `CspPolicySet` that determine whether
//! a given action (resource fetch, script execution, form submission, etc.)
//! is allowed by the CSP policies.

use url::{Origin, Url};

use super::directive::CspDirectiveKind;
use super::parser::CspPolicySet;
use super::source::{compute_hash, CspSource, SourceMatchContext};
use super::violation::{report_violation, CspViolation, Disposition};

use crate::resource::ResourceKind;

/// Result of a CSP check.
#[derive(Debug, Clone)]
pub struct CspCheckResult {
    /// Whether the action is allowed.
    pub allowed: bool,
    /// The directive that was violated (if not allowed).
    pub violated_directive: Option<String>,
    /// Whether this came from a report-only policy.
    pub report_only: bool,
}

impl CspCheckResult {
    fn allow() -> Self {
        Self {
            allowed: true,
            violated_directive: None,
            report_only: false,
        }
    }

    fn deny(directive: &str, report_only: bool) -> Self {
        Self {
            allowed: false,
            violated_directive: Some(directive.to_string()),
            report_only,
        }
    }
}

impl CspPolicySet {
    /// Check whether fetching a subresource of the given kind is allowed.
    pub fn check_resource_fetch(
        &self,
        page_origin: &Origin,
        target_url: &Url,
        resource_kind: ResourceKind,
        nonce: Option<&str>,
        is_inline: bool,
        is_eval: bool,
        content: Option<&[u8]>,
    ) -> CspCheckResult {
        let directive_kind = resource_kind_to_directive(resource_kind);
        self.check_source_list(
            page_origin,
            target_url,
            directive_kind,
            nonce,
            is_inline,
            is_eval,
            content,
        )
    }

    /// Check whether inline script execution is allowed.
    pub fn check_inline_script(
        &self,
        page_origin: &Origin,
        nonce: Option<&str>,
        content: &[u8],
    ) -> CspCheckResult {
        // Use a placeholder URL; inline checks never match host/scheme sources.
        let target_url = Url::parse("about:inline").unwrap();
        let result = self.check_source_list(
            page_origin,
            &target_url,
            CspDirectiveKind::ScriptSrc,
            nonce,
            true,  // is_inline
            false, // is_eval
            Some(content),
        );
        result
    }

    /// Check whether inline style application is allowed.
    pub fn check_inline_style(
        &self,
        page_origin: &Origin,
        nonce: Option<&str>,
        content: &[u8],
    ) -> CspCheckResult {
        let target_url = Url::parse("about:inline").unwrap();
        self.check_source_list(
            page_origin,
            &target_url,
            CspDirectiveKind::StyleSrc,
            nonce,
            true,
            false,
            Some(content),
        )
    }

    /// Check whether eval() / new Function() is allowed.
    pub fn check_eval(&self, page_origin: &Origin, is_script: bool) -> CspCheckResult {
        let directive = if is_script {
            CspDirectiveKind::ScriptSrc
        } else {
            CspDirectiveKind::StyleSrc
        };
        let target_url = Url::parse("about:eval").unwrap();
        self.check_source_list(
            page_origin,
            &target_url,
            directive,
            None,
            false,
            true, // is_eval
            None,
        )
    }

    /// Check whether a form submission to the given URL is allowed.
    pub fn check_form_action(
        &self,
        page_origin: &Origin,
        action_url: &Url,
    ) -> CspCheckResult {
        self.check_source_list(
            page_origin,
            action_url,
            CspDirectiveKind::FormAction,
            None,
            false,
            false,
            None,
        )
    }

    /// Check whether navigation to the given URL is allowed.
    pub fn check_navigation(
        &self,
        page_origin: &Origin,
        target_url: &Url,
    ) -> CspCheckResult {
        self.check_source_list(
            page_origin,
            target_url,
            CspDirectiveKind::NavigateTo,
            None,
            false,
            false,
            None,
        )
    }

    /// Check whether setting base URI to the given URL is allowed.
    pub fn check_base_uri(
        &self,
        page_origin: &Origin,
        base_url: &Url,
    ) -> CspCheckResult {
        self.check_source_list(
            page_origin,
            base_url,
            CspDirectiveKind::BaseUri,
            None,
            false,
            false,
            None,
        )
    }

    /// Check whether a connect (fetch, XHR, WebSocket, SSE) is allowed.
    pub fn check_connect(
        &self,
        page_origin: &Origin,
        target_url: &Url,
    ) -> CspCheckResult {
        self.check_source_list(
            page_origin,
            target_url,
            CspDirectiveKind::ConnectSrc,
            None,
            false,
            false,
            None,
        )
    }
}

// ==================== Internal Logic ====================

impl CspPolicySet {
    /// Core evaluation: check a source list against a policy.
    fn check_source_list(
        &self,
        page_origin: &Origin,
        target_url: &Url,
        directive_kind: CspDirectiveKind,
        nonce: Option<&str>,
        is_inline: bool,
        is_eval: bool,
        content: Option<&[u8]>,
    ) -> CspCheckResult {
        // Check enforce policy first
        if let Some(policy) = &self.enforce {
            let result = Self::evaluate_policy(
                policy,
                page_origin,
                target_url,
                directive_kind,
                nonce,
                is_inline,
                is_eval,
                content,
                false, // report_only
            );
            if !result.allowed {
                return result;
            }
        }

        // Check report-only policy — log violations but don't block
        if let Some(policy) = &self.report_only {
            let result = Self::evaluate_policy(
                policy,
                page_origin,
                target_url,
                directive_kind,
                nonce,
                is_inline,
                is_eval,
                content,
                true, // report_only
            );
            if !result.allowed {
                // Log but allow
                report_violation(&CspViolation {
                    document_uri: page_origin.ascii_serialization(),
                    blocked_uri: target_url.to_string(),
                    effective_directive: directive_kind.name().to_string(),
                    original_policy: policy.raw().to_string(),
                    disposition: Disposition::Report,
                    status_code: 0,
                });
            }
        }

        CspCheckResult::allow()
    }

    /// Evaluate a single policy against the given parameters.
    fn evaluate_policy(
        policy: &super::parser::CspPolicy,
        page_origin: &Origin,
        target_url: &Url,
        directive_kind: CspDirectiveKind,
        nonce: Option<&str>,
        is_inline: bool,
        is_eval: bool,
        content: Option<&[u8]>,
        report_only: bool,
    ) -> CspCheckResult {
        let (sources, effective_kind) = policy.effective_sources_and_kind(directive_kind);

        // No directive and no fallback → allow
        if sources.is_empty() {
            return CspCheckResult::allow();
        }

        // Check for 'none' — blocks everything
        if sources.iter().any(|s| matches!(s, CspSource::None)) {
            // 'none' with other sources is technically a parse error,
            // but we treat 'none' as "block all"
            if sources.len() == 1 {
                return CspCheckResult::deny(effective_kind.name(), report_only);
            }
        }

        // Handle inline check
        if is_inline {
            let has_unsafe_inline = sources.iter().any(|s| matches!(s, CspSource::UnsafeInline));

            // 'unsafe-inline' allows all inline content
            if has_unsafe_inline {
                return CspCheckResult::allow();
            }

            // Check nonce match
            if let Some(n) = nonce {
                let nonce_matches = sources.iter().any(|s| {
                    matches!(s, CspSource::Nonce { value } if value == n)
                });
                if nonce_matches {
                    return CspCheckResult::allow();
                }
            }

            // Check hash match
            if let Some(data) = content {
                use super::directive::HashAlgorithm;
                for algo in [HashAlgorithm::Sha256, HashAlgorithm::Sha384, HashAlgorithm::Sha512] {
                    let hash = compute_hash(&algo, data);
                    let hash_matches = sources.iter().any(|s| {
                        matches!(s, CspSource::Hash { algorithm, value } if algorithm == &algo && value == &hash)
                    });
                    if hash_matches {
                        return CspCheckResult::allow();
                    }
                }
            }

            return CspCheckResult::deny(effective_kind.name(), report_only);
        }

        // Handle eval check
        if is_eval {
            let has_unsafe_eval = sources.iter().any(|s| matches!(s, CspSource::UnsafeEval));
            if !has_unsafe_eval {
                return CspCheckResult::deny(effective_kind.name(), report_only);
            }
            return CspCheckResult::allow();
        }

        // Build match context
        let ctx = SourceMatchContext {
            page_origin,
            element_nonce: nonce,
            content_hash: None,
            target_url: Some(target_url),
        };

        // Check each source for a match
        for source in sources {
            if source.is_flag() {
                continue; // Flags handled above
            }
            if source.matches(&ctx) {
                return CspCheckResult::allow();
            }
        }

        // No source matched → deny
        CspCheckResult::deny(effective_kind.name(), report_only)
    }
}

/// Maps `ResourceKind` to the corresponding `CspDirectiveKind`.
fn resource_kind_to_directive(kind: ResourceKind) -> CspDirectiveKind {
    match kind {
        ResourceKind::Stylesheet => CspDirectiveKind::StyleSrc,
        ResourceKind::Script => CspDirectiveKind::ScriptSrc,
        ResourceKind::Image => CspDirectiveKind::ImgSrc,
        ResourceKind::Font => CspDirectiveKind::FontSrc,
        ResourceKind::Media => CspDirectiveKind::MediaSrc,
        ResourceKind::Document => CspDirectiveKind::DefaultSrc,
        ResourceKind::Other => CspDirectiveKind::DefaultSrc,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn origin(url: &str) -> Origin {
        Url::parse(url).unwrap().origin()
    }

    fn parse_set(raw: &str) -> CspPolicySet {
        CspPolicySet::from_raw(raw)
    }

    // ==================== Resource Fetch Tests ====================

    #[test]
    fn test_default_src_self_allows_same_origin() {
        let set = parse_set("default-src 'self'");
        let o = origin("https://example.com");
        let url = Url::parse("https://example.com/script.js").unwrap();
        let result = set.check_resource_fetch(&o, &url, ResourceKind::Script, None, false, false, None);
        assert!(result.allowed);
    }

    #[test]
    fn test_default_src_self_blocks_cross_origin() {
        let set = parse_set("default-src 'self'");
        let o = origin("https://example.com");
        let url = Url::parse("https://evil.com/script.js").unwrap();
        let result = set.check_resource_fetch(&o, &url, ResourceKind::Script, None, false, false, None);
        assert!(!result.allowed);
        assert_eq!(result.violated_directive.as_deref(), Some("default-src"));
    }

    #[test]
    fn test_script_src_overrides_default() {
        let set = parse_set("default-src 'self'; script-src https://cdn.com");
        let o = origin("https://example.com");

        // script-src allows cdn.com
        let cdn = Url::parse("https://cdn.com/app.js").unwrap();
        assert!(set.check_resource_fetch(&o, &cdn, ResourceKind::Script, None, false, false, None).allowed);

        // but blocks same-origin (only cdn.com allowed for scripts)
        let self_url = Url::parse("https://example.com/app.js").unwrap();
        assert!(!set.check_resource_fetch(&o, &self_url, ResourceKind::Script, None, false, false, None).allowed);

        // non-script resources still use default-src
        let css = Url::parse("https://example.com/style.css").unwrap();
        assert!(set.check_resource_fetch(&o, &css, ResourceKind::Stylesheet, None, false, false, None).allowed);
    }

    #[test]
    fn test_no_policy_allows_all() {
        let set = CspPolicySet::default();
        let o = origin("https://example.com");
        let url = Url::parse("https://evil.com/script.js").unwrap();
        assert!(set.check_resource_fetch(&o, &url, ResourceKind::Script, None, false, false, None).allowed);
    }

    // ==================== Inline Script Tests ====================

    #[test]
    fn test_inline_script_blocked_by_default() {
        let set = parse_set("script-src 'self'");
        let o = origin("https://example.com");
        let result = set.check_inline_script(&o, None, b"alert(1)");
        assert!(!result.allowed);
    }

    #[test]
    fn test_inline_script_allowed_with_unsafe_inline() {
        let set = parse_set("script-src 'self' 'unsafe-inline'");
        let o = origin("https://example.com");
        let result = set.check_inline_script(&o, None, b"alert(1)");
        assert!(result.allowed);
    }

    #[test]
    fn test_inline_script_allowed_with_nonce() {
        let set = parse_set("script-src 'nonce-abc123'");
        let o = origin("https://example.com");
        let result = set.check_inline_script(&o, Some("abc123"), b"alert(1)");
        assert!(result.allowed);
    }

    #[test]
    fn test_inline_script_blocked_wrong_nonce() {
        let set = parse_set("script-src 'nonce-abc123'");
        let o = origin("https://example.com");
        let result = set.check_inline_script(&o, Some("wrong"), b"alert(1)");
        assert!(!result.allowed);
    }

    #[test]
    fn test_inline_script_allowed_with_hash() {
        let content = b"alert(1)";
        let hash = compute_hash(&super::super::directive::HashAlgorithm::Sha256, content);
        let set = CspPolicySet::from_raw(&format!("script-src 'sha256-{}'", hash));
        let o = origin("https://example.com");
        let result = set.check_inline_script(&o, None, content);
        assert!(result.allowed);
    }

    #[test]
    fn test_script_src_none_blocks_all() {
        let set = parse_set("script-src 'none'");
        let o = origin("https://example.com");
        // Inline blocked
        assert!(!set.check_inline_script(&o, None, b"alert(1)").allowed);
        // External blocked
        let url = Url::parse("https://example.com/app.js").unwrap();
        assert!(!set.check_resource_fetch(&o, &url, ResourceKind::Script, None, false, false, None).allowed);
    }

    // ==================== Eval Tests ====================

    #[test]
    fn test_eval_blocked_by_default() {
        let set = parse_set("script-src 'self'");
        let o = origin("https://example.com");
        assert!(!set.check_eval(&o, true).allowed);
    }

    #[test]
    fn test_eval_allowed_with_unsafe_eval() {
        let set = parse_set("script-src 'self' 'unsafe-eval'");
        let o = origin("https://example.com");
        assert!(set.check_eval(&o, true).allowed);
    }

    // ==================== Form Action Tests ====================

    #[test]
    fn test_form_action_self_blocks_external() {
        let set = parse_set("form-action 'self'");
        let o = origin("https://example.com");

        // Same-origin allowed
        let self_url = Url::parse("https://example.com/submit").unwrap();
        assert!(set.check_form_action(&o, &self_url).allowed);

        // Cross-origin blocked
        let ext_url = Url::parse("https://evil.com/steal").unwrap();
        assert!(!set.check_form_action(&o, &ext_url).allowed);
    }

    #[test]
    fn test_form_action_no_directive_allows_all() {
        let set = parse_set("default-src 'self'");
        let o = origin("https://example.com");
        // form-action has no fallback to default-src
        let ext_url = Url::parse("https://evil.com/steal").unwrap();
        assert!(set.check_form_action(&o, &ext_url).allowed);
    }

    // ==================== Navigation Tests ====================

    #[test]
    fn test_navigate_to_restriction() {
        let set = parse_set("navigate-to 'self'");
        let o = origin("https://example.com");

        let self_url = Url::parse("https://example.com/page2").unwrap();
        assert!(set.check_navigation(&o, &self_url).allowed);

        let ext_url = Url::parse("https://evil.com").unwrap();
        assert!(!set.check_navigation(&o, &ext_url).allowed);
    }

    // ==================== Base URI Tests ====================

    #[test]
    fn test_base_uri_restriction() {
        let set = parse_set("base-uri 'self'");
        let o = origin("https://example.com");

        let self_base = Url::parse("https://example.com/").unwrap();
        assert!(set.check_base_uri(&o, &self_base).allowed);

        let ext_base = Url::parse("https://evil.com/").unwrap();
        assert!(!set.check_base_uri(&o, &ext_base).allowed);
    }

    // ==================== Connect (WebSocket/SSE) Tests ====================

    #[test]
    fn test_connect_src_restriction() {
        let set = parse_set("connect-src 'self' https://api.example.com");
        let o = origin("https://example.com");

        // Same-origin allowed
        let ws_url = Url::parse("wss://example.com/ws").unwrap();
        assert!(set.check_connect(&o, &ws_url).allowed);

        // Allowed API
        let api_url = Url::parse("https://api.example.com/data").unwrap();
        assert!(set.check_connect(&o, &api_url).allowed);

        // Blocked
        let ext_url = Url::parse("wss://evil.com/ws").unwrap();
        assert!(!set.check_connect(&o, &ext_url).allowed);
    }

    // ==================== Report-Only Tests ====================

    #[test]
    fn test_report_only_does_not_block() {
        let headers = vec![
            ("Content-Security-Policy-Report-Only".to_string(), "script-src 'none'".to_string()),
        ];
        let set = CspPolicySet::from_headers(&headers);
        let o = origin("https://example.com");

        // Should still be allowed (report-only)
        let url = Url::parse("https://example.com/app.js").unwrap();
        let result = set.check_resource_fetch(&o, &url, ResourceKind::Script, None, false, false, None);
        assert!(result.allowed);
    }

    // ==================== Host Pattern Tests ====================

    #[test]
    fn test_wildcard_host_source() {
        let set = parse_set("script-src *.example.com");
        let o = origin("https://example.com");

        let cdn = Url::parse("https://cdn.example.com/app.js").unwrap();
        assert!(set.check_resource_fetch(&o, &cdn, ResourceKind::Script, None, false, false, None).allowed);

        let deep = Url::parse("https://a.b.c.example.com/app.js").unwrap();
        assert!(set.check_resource_fetch(&o, &deep, ResourceKind::Script, None, false, false, None).allowed);

        let other = Url::parse("https://notexample.com/app.js").unwrap();
        assert!(!set.check_resource_fetch(&o, &other, ResourceKind::Script, None, false, false, None).allowed);
    }

    #[test]
    fn test_scheme_source() {
        let set = parse_set("img-src https: data:");
        let o = origin("https://example.com");

        let https_url = Url::parse("https://any.com/img.png").unwrap();
        assert!(set.check_resource_fetch(&o, &https_url, ResourceKind::Image, None, false, false, None).allowed);

        let data_url = Url::parse("data:image/png;base64,abc").unwrap();
        assert!(set.check_resource_fetch(&o, &data_url, ResourceKind::Image, None, false, false, None).allowed);

        let http_url = Url::parse("http://any.com/img.png").unwrap();
        assert!(!set.check_resource_fetch(&o, &http_url, ResourceKind::Image, None, false, false, None).allowed);
    }

    // ==================== Resource Kind Mapping Tests ====================

    #[test]
    fn test_resource_kind_mapping() {
        assert_eq!(resource_kind_to_directive(ResourceKind::Script), CspDirectiveKind::ScriptSrc);
        assert_eq!(resource_kind_to_directive(ResourceKind::Stylesheet), CspDirectiveKind::StyleSrc);
        assert_eq!(resource_kind_to_directive(ResourceKind::Image), CspDirectiveKind::ImgSrc);
        assert_eq!(resource_kind_to_directive(ResourceKind::Font), CspDirectiveKind::FontSrc);
        assert_eq!(resource_kind_to_directive(ResourceKind::Media), CspDirectiveKind::MediaSrc);
        assert_eq!(resource_kind_to_directive(ResourceKind::Document), CspDirectiveKind::DefaultSrc);
        assert_eq!(resource_kind_to_directive(ResourceKind::Other), CspDirectiveKind::DefaultSrc);
    }

    // ==================== Style Tests ====================

    #[test]
    fn test_inline_style_blocked() {
        let set = parse_set("style-src 'self'");
        let o = origin("https://example.com");
        assert!(!set.check_inline_style(&o, None, b"body{color:red}").allowed);
    }

    #[test]
    fn test_inline_style_allowed_with_unsafe_inline() {
        let set = parse_set("style-src 'self' 'unsafe-inline'");
        let o = origin("https://example.com");
        assert!(set.check_inline_style(&o, None, b"body{color:red}").allowed);
    }
}
