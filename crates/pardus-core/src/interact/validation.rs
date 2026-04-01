//! Shared validation and lookup helpers for interaction operations.
//!
//! Eliminates duplication between the HTML-only path (actions.rs)
//! and the JS-enabled path (js_interact.rs).

use scraper::{Html, Selector};

use super::actions::InteractionResult;
use super::element::ElementHandle;

/// Validate that an element handle is not disabled.
pub fn validate_not_disabled(handle: &ElementHandle) -> Option<InteractionResult> {
    if handle.is_disabled {
        Some(InteractionResult::ElementNotFound {
            selector: handle.selector.clone(),
            reason: "element is disabled".to_string(),
        })
    } else {
        None
    }
}

/// Validate that an element has a specific action.
pub fn validate_action(handle: &ElementHandle, expected: &str) -> Option<InteractionResult> {
    match &handle.action {
        Some(action) if action == expected => None,
        Some(action) => Some(InteractionResult::ElementNotFound {
            selector: handle.selector.clone(),
            reason: format!("element action is '{}', expected '{}'", action, expected),
        }),
        None => Some(InteractionResult::ElementNotFound {
            selector: handle.selector.clone(),
            reason: "element has no action".to_string(),
        }),
    }
}

/// Validate that an element is fillable (action is "fill" or "select").
pub fn validate_fillable(handle: &ElementHandle) -> Option<InteractionResult> {
    match &handle.action {
        Some(action) if action == "fill" || action == "select" => None,
        Some(action) => Some(InteractionResult::ElementNotFound {
            selector: handle.selector.clone(),
            reason: format!("element action is '{}', not fillable", action),
        }),
        None => Some(InteractionResult::ElementNotFound {
            selector: handle.selector.clone(),
            reason: "element has no action".to_string(),
        }),
    }
}

/// Check that a selector matches an element in the given HTML string.
/// Returns `ElementNotFound` result if no match, or `None` if found.
pub fn check_selector_exists(html: &str, selector: &str) -> Option<InteractionResult> {
    if let Ok(sel) = Selector::parse(selector) {
        let doc = Html::parse_document(html);
        if doc.select(&sel).next().is_none() {
            return Some(InteractionResult::ElementNotFound {
                selector: selector.to_string(),
                reason: "no element matches selector".to_string(),
            });
        }
    }
    None
}

/// Resolve a relative href against a base URL.
pub fn resolve_href(base_url: &str, href: &str) -> String {
    url::Url::parse(base_url)
        .and_then(|base| base.join(href))
        .map(|u| u.to_string())
        .unwrap_or_else(|_| href.to_string())
}

/// Build a JS execution failure result.
pub fn js_execution_failed(selector: &str) -> InteractionResult {
    InteractionResult::ElementNotFound {
        selector: selector.to_string(),
        reason: "JS execution failed".to_string(),
    }
}

/// Build a JS timeout result.
pub fn js_interaction_timeout(selector: &str) -> InteractionResult {
    InteractionResult::ElementNotFound {
        selector: selector.to_string(),
        reason: "JS interaction timed out".to_string(),
    }
}
