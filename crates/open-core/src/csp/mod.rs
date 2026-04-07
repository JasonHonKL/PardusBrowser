//! Content Security Policy (CSP) enforcement.
//!
//! Parses CSP headers from HTTP responses and enforces directives when
//! loading subresources, executing scripts, submitting forms, and navigating.
//!
//! # Usage
//!
//! CSP is **opt-in** via `BrowserConfig::csp.enforce_csp`. When enabled,
//! the browser parses `Content-Security-Policy` headers from responses
//! and stores the policy on each `Page`.
//!
//! # Supported Directives
//!
//! - `default-src`, `script-src`, `style-src`, `img-src`, `connect-src`,
//!   `font-src`, `frame-src`, `media-src`, `object-src`
//! - `base-uri`, `form-action`, `navigate-to`
//! - `sandbox`, `upgrade-insecure-requests`
//!
//! # Supported Source Expressions
//!
//! - `'none'`, `'self'`, `'unsafe-inline'`, `'unsafe-eval'`, `'strict-dynamic'`
//! - `'nonce-<base64>'`, `'sha256/sha384/sha512-<base64>'`
//! - Scheme (`https:`), Host (`example.com`, `*.example.com`)

pub mod directive;
pub mod eval;
pub mod parser;
pub mod source;
pub mod violation;

pub use directive::{CspDirective, CspDirectiveKind, HashAlgorithm};
pub use eval::CspCheckResult;
pub use parser::{CspPolicy, CspPolicySet};
pub use source::{CspSource, SourceMatchContext, compute_hash};
pub use violation::{CspViolation, Disposition, report_violation};
