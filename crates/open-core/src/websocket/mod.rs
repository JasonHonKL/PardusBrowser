//! WebSocket support for OpenBrowser.
//!
//! Provides native WS/WSS protocol handling with:
//! - Connection management via `WebSocketManager`
//! - CDP event emission for WebSocket lifecycle
//! - Integration with network logging
//! - SSRF protection via `UrlPolicy`

pub mod connection;
pub mod manager;

pub use connection::WebSocketConnection;
pub use manager::{WebSocketConfig, WebSocketManager};
