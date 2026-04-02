//! Built-in interceptor implementations.

use std::collections::HashMap;

use async_trait::async_trait;

use super::rules::InterceptorRule;
use super::{
    InterceptAction, Interceptor, InterceptorPhase, MockResponse, ModifiedRequest,
    RequestContext,
};

// ---------------------------------------------------------------------------
// BlockingInterceptor
// ---------------------------------------------------------------------------

/// Blocks all requests matching a rule.
pub struct BlockingInterceptor {
    rule: InterceptorRule,
}

impl BlockingInterceptor {
    pub fn new(rule: InterceptorRule) -> Self {
        Self { rule }
    }
}

#[async_trait]
impl Interceptor for BlockingInterceptor {
    fn phase(&self) -> InterceptorPhase {
        InterceptorPhase::BeforeRequest
    }

    fn matches(&self, ctx: &RequestContext) -> bool {
        self.rule.matches(ctx)
    }

    async fn intercept_request(&self, _ctx: &mut RequestContext) -> InterceptAction {
        InterceptAction::Block
    }
}

// ---------------------------------------------------------------------------
// RedirectInterceptor
// ---------------------------------------------------------------------------

/// Redirects matching requests to a different URL.
pub struct RedirectInterceptor {
    rule: InterceptorRule,
    target_url: String,
}

impl RedirectInterceptor {
    pub fn new(rule: InterceptorRule, target_url: String) -> Self {
        Self { rule, target_url }
    }
}

#[async_trait]
impl Interceptor for RedirectInterceptor {
    fn phase(&self) -> InterceptorPhase {
        InterceptorPhase::BeforeRequest
    }

    fn matches(&self, ctx: &RequestContext) -> bool {
        self.rule.matches(ctx)
    }

    async fn intercept_request(&self, _ctx: &mut RequestContext) -> InterceptAction {
        InterceptAction::Redirect(self.target_url.clone())
    }
}

// ---------------------------------------------------------------------------
// HeaderModifierInterceptor
// ---------------------------------------------------------------------------

/// Adds or removes headers on matching requests.
pub struct HeaderModifierInterceptor {
    rule: Option<InterceptorRule>,
    headers_to_add: HashMap<String, String>,
    headers_to_remove: Vec<String>,
}

impl HeaderModifierInterceptor {
    /// Add/replace headers on all matching requests.
    pub fn new(
        rule: Option<InterceptorRule>,
        headers_to_add: HashMap<String, String>,
    ) -> Self {
        Self {
            rule,
            headers_to_add,
            headers_to_remove: Vec::new(),
        }
    }

    /// Remove headers from matching requests.
    pub fn with_removal(mut self, headers: Vec<String>) -> Self {
        self.headers_to_remove = headers;
        self
    }
}

#[async_trait]
impl Interceptor for HeaderModifierInterceptor {
    fn phase(&self) -> InterceptorPhase {
        InterceptorPhase::BeforeRequest
    }

    fn matches(&self, ctx: &RequestContext) -> bool {
        match &self.rule {
            Some(rule) => rule.matches(ctx),
            None => true, // applies to all requests when no rule
        }
    }

    async fn intercept_request(&self, _ctx: &mut RequestContext) -> InterceptAction {
        InterceptAction::Modify(ModifiedRequest {
            url: None,
            headers: self.headers_to_add.clone(),
            remove_headers: self.headers_to_remove.clone(),
            body: None,
        })
    }
}

// ---------------------------------------------------------------------------
// MockResponseInterceptor
// ---------------------------------------------------------------------------

/// Returns a synthetic response for matching requests without making the HTTP call.
pub struct MockResponseInterceptor {
    rule: InterceptorRule,
    status: u16,
    headers: HashMap<String, String>,
    body: Vec<u8>,
}

impl MockResponseInterceptor {
    pub fn new(
        rule: InterceptorRule,
        status: u16,
        headers: HashMap<String, String>,
        body: Vec<u8>,
    ) -> Self {
        Self {
            rule,
            status,
            headers,
            body,
        }
    }

    /// Convenience: mock with a text body.
    pub fn text(rule: InterceptorRule, status: u16, body: &str) -> Self {
        let mut headers = HashMap::new();
        headers.insert("content-type".to_string(), "text/html; charset=utf-8".to_string());
        Self::new(rule, status, headers, body.as_bytes().to_vec())
    }

    /// Convenience: mock with a JSON body.
    pub fn json(rule: InterceptorRule, status: u16, body: &str) -> Self {
        let mut headers = HashMap::new();
        headers.insert(
            "content-type".to_string(),
            "application/json; charset=utf-8".to_string(),
        );
        Self::new(rule, status, headers, body.as_bytes().to_vec())
    }
}

#[async_trait]
impl Interceptor for MockResponseInterceptor {
    fn phase(&self) -> InterceptorPhase {
        InterceptorPhase::BeforeRequest
    }

    fn matches(&self, ctx: &RequestContext) -> bool {
        self.rule.matches(ctx)
    }

    async fn intercept_request(&self, _ctx: &mut RequestContext) -> InterceptAction {
        InterceptAction::Mock(MockResponse {
            status: self.status,
            headers: self.headers.clone(),
            body: self.body.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::intercept::rules::InterceptorRule;
    use pardus_debug::{Initiator, ResourceType};

    fn test_ctx(url: &str) -> RequestContext {
        RequestContext {
            url: url.to_string(),
            method: "GET".to_string(),
            headers: HashMap::new(),
            body: None,
            resource_type: ResourceType::Document,
            initiator: Initiator::Navigation,
            is_navigation: true,
        }
    }

    #[tokio::test]
    async fn test_blocking_interceptor() {
        let interceptor = BlockingInterceptor::new(InterceptorRule::UrlGlob("*/ads/*".to_string()));
        let mut ctx = test_ctx("https://example.com/ads/banner.png");
        assert!(interceptor.matches(&ctx));
        let action = interceptor.intercept_request(&mut ctx).await;
        assert!(matches!(action, InterceptAction::Block));
    }

    #[tokio::test]
    async fn test_blocking_no_match() {
        let interceptor = BlockingInterceptor::new(InterceptorRule::UrlGlob("*/ads/*".to_string()));
        let ctx = test_ctx("https://example.com/page");
        assert!(!interceptor.matches(&ctx));
    }

    #[tokio::test]
    async fn test_redirect_interceptor() {
        let interceptor = RedirectInterceptor::new(
            InterceptorRule::UrlGlob("*/api/*".to_string()),
            "http://localhost:3000/api/".to_string(),
        );
        let mut ctx = test_ctx("https://example.com/api/data");
        let action = interceptor.intercept_request(&mut ctx).await;
        match action {
            InterceptAction::Redirect(url) => assert_eq!(url, "http://localhost:3000/api/"),
            _ => panic!("expected Redirect"),
        }
    }

    #[tokio::test]
    async fn test_header_modifier() {
        let interceptor = HeaderModifierInterceptor::new(
            None,
            HashMap::from([("Authorization".to_string(), "Bearer token".to_string())]),
        );
        let mut ctx = test_ctx("https://example.com/page");
        assert!(interceptor.matches(&ctx)); // no rule = matches all
        let action = interceptor.intercept_request(&mut ctx).await;
        match action {
            InterceptAction::Modify(mods) => {
                assert_eq!(mods.headers.get("Authorization").unwrap(), "Bearer token");
            }
            _ => panic!("expected Modify"),
        }
    }

    #[tokio::test]
    async fn test_mock_response() {
        let interceptor = MockResponseInterceptor::text(
            InterceptorRule::UrlGlob("*/api/data*".to_string()),
            200,
            "{\"mocked\": true}",
        );
        let mut ctx = test_ctx("https://example.com/api/data");
        let action = interceptor.intercept_request(&mut ctx).await;
        match action {
            InterceptAction::Mock(mock) => {
                assert_eq!(mock.status, 200);
                assert_eq!(String::from_utf8(mock.body).unwrap(), "{\"mocked\": true}");
            }
            _ => panic!("expected Mock"),
        }
    }
}
