//! Server-Sent Events (SSE) support.
//!
//! Provides SSE protocol parsing, async client, and connection management.
//! The `EventSource` JS Web API integration (ops in `js/sse.rs`) is gated
//! behind the `js` feature, but the parser, client, and manager are always
//! available for use from the browser API.

pub mod client;
pub mod manager;
pub mod parser;

pub use client::{SseConnectionHandle, SSE_CLOSED, SSE_CONNECTING, SSE_OPEN};
pub use manager::SseManager;
pub use parser::{SseEvent, SseParser};
