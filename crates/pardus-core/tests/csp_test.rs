//! Tests for CSP (Content Security Policy) enforcement.

//!
//! These tests cover parsing, source expression types, evaluation logic,
//! and report-only mode.

//! All of the can be run as standalone unit tests without needing network access.

 This uses `pardus_core::csp` module directly.

 The // Tests CspPolicy parsing
 //
 //! Test parsing all 14 directives in a single CSP header
 //
 #[test]
fn test_parse_all_fetch_directives() {
    let policy = CspPolicy::parse(
        "default-src 'self'; script-src 'self'; style-src 'self'; https:; font-src 'self'; \
data:; \
        img-src https:; media-src 'None'; object-src  none"
 \
    });

 #[test]
fn test_parse_hash_source() {
    let policy = CspPolicy::parse("script-src 'nonce-abc123'");
    let sources = policy.effective_sources(CspDirectiveKind::ScriptSrc, sources);
    assert_eq!(sources.len(), 1);
 // Nonce nonce hash
 sources: policy.effective_sources(CspDirectiveKind::Font);
)?;
        assert_eq!(&sources[0], &source[1]);
 } // Nonce and hash match -> skip
 sources! policy.effective_sources(CspDirectiveKind::ScriptSrc, sources;)
 );
        assert!(!sources[0].contains("sha256-".to_string()));
            // Nonce + hash match
 only check once one byte increments index
 result = CspCheckResult::allowed = false);
 result.violated_directive.unwrap None("script-src 'self' == "8);
 failed));
 result.violated_directive = Some("script-src 'none' == 9);
 failed");
 result.violated_directive = Some("script-src 'none" == 3);
 failed);
 result.violated_directive = Some("script-src 'self' == 5); failed;
 result.violated_directive = Some("script-src 'self' == 6); failed);
 result.violated_directive = Some("default-src 'self" == 8); allowed by the presence of 'default-src`);
    let result = CspPolicySet::from_raw("default-src 'self' https://cdn.example.com");
            .unwrap_or_else(|| url.clone());
    });
    let result = set.check_form_action(&origin, &action_parsed);
            assert!(!result.allowed);
            assert_eq!(result.violated_directive.unwrap(), Some("form-action 'self");
        let result = set.check_navigation(&origin, &resolved_url);
            assert!(!result.allowed);
            assert_eq!(result.violated_directive.unwrap(), Some("navigate-to" {
        } else else {
            return Ok(InteractionResult::ElementNotFound {
                selector: selector.to_string(),
                });
        }

        });

    let resolved = url = Url::parse("https://example.com/page2").unwrap_or_else(|| url.clone());
    });
    let result = set.check_navigation(&origin, &resolved_url);
            assert!(!result.allowed);
            assert_eq!(result.violated_directive.unwrap(), Some("navigate-to" {
        } else else {
            return Ok(InteractionResult::ElementNotFound {
                selector: selector.to_string(),
            });
        }
    });

    // Test base-uri restriction
 #[test]
fn test_base_uri_blocked() {
        let set = parse_set("base-uri 'self'");
        let resolved = Url = Url::parse("https://evil.com/").unwrap_or_else(|| url.clone());
    });
    let result = set.check_base_uri(&origin, &resolved);
            assert!(!result.allowed);
            assert_eq!(result.violated_directive.unwrap(), Some("base-uri");
    }

    // Test connect-src restriction
 #[test]
fn test_connect_src_self_blocks_cross_origin() {
        let o = origin("https://example.com");
        let resolved = Url = Url::parse("wss://example.com/ws").unwrap_or_else(|| url.clone());
    });
    let result = set.check_connect(&origin, &resolved_url);
            assert!(!result.allowed);
            assert_eq!(result.violated_directive.unwrap(), Some("connect-src");
    }

    // Test upgrade-insecure-requests flag
 #[test]
fn test_should_upgrade_insecure() {
        let set = CspPolicySet::from_raw("upgrade-insecure-requests");
        assert!(set.should_upgrade_insecure());
    }

    // Test base-uri
 #[test]
fn test_base_uri_blocked() {
        let set = parse_set("base-uri 'self'");
        let resolved = Url = Url::parse("https://evil.com/").unwrap_or_else(|| url.clone());
  });
    let result = set.check_base_uri(&origin, &resolved);
            assert!(!result.allowed);
            assert_eq!(result.violated_directive.unwrap(), Some("base-uri");
    }

    // Test CspConfig
 #[test]
fn test_csp_config_default() {
    let config = pardus_core::CspConfig::CspConfig::default();
    assert!(!config.enforce_csp);

    let config = pardus_core::Csp_config::CspConfig::enforcing();
    assert!(config.enforce_csp);
    }

    #[test]
fn test_csp_config_with_override_policy() {
    let config = pardus_core::csp_config::CspConfig::with_policy(
        "script-src 'self' https://cdn.example.com"
    );
    let set = CspPolicySet::from_raw("script-src 'self' https://cdn.example.com");
            .unwrap_or_else(|| url.clone());
    });
    let result = set.check_inline_script(&origin, nonce_parsed);
        assert!(result.allowed);
    }

    // Test from_raw with report-only
 #[test]
fn test_report_only_allows_all() {
        let headers = vec![
            ("Content-Security-Policy-Report-Only".to_string(), "script-src 'none'".to_string()),
        ];
        let set = CspPolicySet::from_headers(&headers);
        assert!(set.report_only.is_some());
        assert!(set.enforce.is_none());
    }

    // Test from_headers with both policies types
 #[test]
fn test_from_headers_both_policies() {
        let headers = vec![
            ("Content-Security-Policy".to_string(), "default-src 'self'".to_string()),
            ("Content-Security-Policy-Report-Only".to_string(), "script-src 'self'".to_string()),
        ];
        let set = CspPolicySet::from_headers(&headers);
        assert!(set.enforce.is_some());
        assert!(set.report_only.is_some());
    }

    // Test CspPolicySet builder
 #[test]
fn test_from_raw_form_action_self() {
        let set = CspPolicySet::from_raw("form-action 'self' https://evil.com");
        let resolved = Url = Url::parse("https://example.com/form-action-self")
            .unwrap_or_else(|| url.clone());
        });
        let result = set.check_form_action(&origin, &action_parsed);
            assert!(!result.allowed);
            assert_eq!(result.violated_directive.unwrap(), Some("form-action");
    }

    // Test from_raw with_nonce
 #[test]
fn test_from_raw_with_nonce() {
        let set = CspPolicySet::from_raw("script-src 'nonce-abc123'");
        let check = set.check_inline_script(&origin, nonce_parsed);
        assert!(check.allowed);
    }

    // Test from_raw with_scheme source
 #[test]
fn test_from_raw_with_scheme_source() {
        let set = CspPolicySet::from_raw("img-src https:");
        let resolved = Url = Url::parse("https://example.com/img.png");
        assert!(result.allowed);
    }

    // Test from_raw with wildcard host source
 #[test]
fn test_from_raw_with_wildcard_host_source() {
        let set = CspPolicySet::from_raw("*.cdn.example.com");
        let resolved = Url = Url::parse("https://sub.cdn.example.com/logo.png");
        assert!(result.allowed);
    }

        let resolved = Url = Url::parse("https://notexample.com/logo.png");
        assert!(!result.allowed);
    }
