//! CSP violation reporting.
//!
//! Logs CSP violations via `tracing::warn!`. Does not send network reports.
//! The `CspViolation` struct is `Serialize` so it can be emitted as JSON
//! for downstream processing.

use serde::Serialize;

/// Whether the violation came from an enforce or report-only policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Disposition {
    Enforce,
    Report,
}

/// A CSP violation report.
#[derive(Debug, Clone, Serialize)]
pub struct CspViolation {
    /// The URL of the document where the violation occurred.
    pub document_uri: String,
    /// The blocked resource URL.
    pub blocked_uri: String,
    /// The directive that was violated.
    pub effective_directive: String,
    /// The original policy string.
    pub original_policy: String,
    /// Whether this is an enforce or report-only violation.
    pub disposition: Disposition,
    /// HTTP status code of the page.
    pub status_code: u16,
}

/// Log a CSP violation using `tracing::warn!`.
pub fn report_violation(violation: &CspViolation) {
    tracing::warn!(
        target: "csp::violation",
        "CSP violation: directive={}, blocked={}, document={}, disposition={:?}",
        violation.effective_directive,
        violation.blocked_uri,
        violation.document_uri,
        violation.disposition,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_violation_serialize() {
        let v = CspViolation {
            document_uri: "https://example.com".to_string(),
            blocked_uri: "https://evil.com/script.js".to_string(),
            effective_directive: "script-src".to_string(),
            original_policy: "script-src 'self'".to_string(),
            disposition: Disposition::Enforce,
            status_code: 200,
        };
        let json = serde_json::to_string(&v).unwrap();
        assert!(json.contains("script-src"));
        assert!(json.contains("enforce"));
    }
}
