//! HTTP/2 push support for subresources.
//!
//! Implements two mechanisms for resource pre-fetching:
//!
//! 1. **Client-side push simulation** — Proactively fetches critical subresources
//!    as soon as the HTML `<head>` is available, before full parsing completes.
//!    Uses the `EarlyScanner` to extract resource hints and `PushCache` to
//!    deduplicate with later explicit fetches.
//!
//! 2. **Low-level PUSH_PROMISE reception** (feature-gated `h2-push`) — Intercepts
//!    HTTP/2 PUSH_PROMISE frames from servers that still support push, buffering
//!    pushed stream data into `PushCache`.

pub mod push_cache;
pub mod early_scanner;
#[cfg(feature = "h2-push")]
pub mod h2_push;

pub use push_cache::{PushCache, PushEntry, PushCacheStats};
pub use early_scanner::EarlyScanner;
#[cfg(feature = "h2-push")]
pub use h2_push::H2PushReceiver;
